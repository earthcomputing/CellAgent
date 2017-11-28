use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::collections::{HashMap, HashSet};
use std::time;
use serde_json;

use blueprint::{Blueprint};
use config::{BASE_TREE_NAME, CONTROL_TREE_NAME, SEPARATOR, CellNo, DatacenterNo, Edge, PortNo};
use datacenter::{Datacenter};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use message::{Message, MsgPayload, MsgType, ManifestMsg, TreeNameMsgPayload};
use message_types::{NocToPort, NocPortError, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use name::TreeID;
use packet::{PacketAssembler, PacketAssemblers};
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::{S, write_err};

#[derive(Debug, Clone)]
pub struct Noc {
	tree_id: TreeID, 
	allowed_trees: HashSet<AllowedTree>,
	control_tree: AllowedTree,
	base_tree: AllowedTree,
	noc_to_outside: NocToOutside,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(noc_to_outside: NocToOutside) -> Result<Noc, Error> {
		let tree_id = TreeID::new("CellAgentTree")?;
		Ok(Noc { tree_id: tree_id, allowed_trees: HashSet::new(), packet_assemblers: PacketAssemblers::new(),
				 control_tree: AllowedTree::new(CONTROL_TREE_NAME), base_tree: AllowedTree::new(BASE_TREE_NAME),
				 noc_to_outside: noc_to_outside })
	}
	pub fn initialize(&self, blueprint: &Blueprint, noc_from_outside: NocFromOutside) ->
            Result<Vec<JoinHandle<()>>, Error> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, mut join_handles) = self.build_datacenter(blueprint)?;
		dc.connect_to_noc(port_to_noc, port_from_noc)?;
		let mut noc = self.clone();
		let noc_to_port_clone = noc_to_port.clone();
		let join_outside = spawn( move || { 
			let _ = noc.listen_outside(noc_from_outside, noc_to_port_clone).map_err(|e| write_err("outside", e));
		});
		join_handles.push(join_outside);
		let mut noc = self.clone();
		let noc_to_port_clone = noc_to_port.clone();
		let join_port = spawn( move || {
			let _ = noc.listen_port(noc_to_port_clone, noc_from_port).map_err(|e| write_err("port", e));
		});
		join_handles.push(join_port);
		let nap = time::Duration::from_millis(1000);
		sleep(nap);
		println!("{}", dc);
		Ok(join_handles)
	}
	fn build_datacenter(&self, blueprint: &Blueprint) -> Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
		let mut dc = Datacenter::new();
		let join_handles = dc.initialize(blueprint)?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort) -> Result<(), Error> {
		loop {
			let packet = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets)?;
				match msg.get_header().get_msg_type() {
					MsgType::TreeName => {
						//println!("Noc got msg {}", msg);
						let allowed_trees = msg.process_noc(&self)?; 
						self.control(&allowed_trees, &noc_to_port)?;						
					}
					_ => return Err(NocError::MsgType { func_name: S("listen_port"), msg_type: msg.get_header().get_msg_type() }.into() )
				}
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	// Sets up the NOC Master and NOC Client services on up trees
	fn control(&mut self, allowed_trees: &Vec<AllowedTree>, noc_to_port: &NocToPort) -> Result<(), Error> {
		// Create an up tree on the border cell for the NOC Master
		for allowed_tree in allowed_trees { self.allowed_trees.insert(allowed_tree.clone()); }
		//println!("Noc allowed trees {:?}", allowed_trees);
		if let Some(deployment_tree) = self.allowed_trees.get(&self.base_tree) {
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("false"));
			eqns.insert(GvmEqn::Xtnd("false"));
			eqns.insert(GvmEqn::Save("false"));
			let ref gvm_eqn = GvmEquation::new(eqns, Vec::new());	
			let vm_uptree = UpTreeSpec::new("NocMasterTreeVm", vec![0])?;
			let container_uptree = UpTreeSpec::new("NocMasterTreeContainer", vec![0])?;
			let ref base_tree = AllowedTree::new(deployment_tree.get_name());
			let noc_container = ContainerSpec::new("NocMaster", "NocMaster", vec![], vec![base_tree])?;
			let noc_vm = VmSpec::new("NocVM", "Ubuntu", CellConfig::Large, 
				vec![base_tree], vec![&noc_container], vec![&container_uptree])?;
			let ref manifest = Manifest::new("NocMaster", CellConfig::Large, deployment_tree.get_name(), vec![base_tree], vec![&noc_vm], vec![&vm_uptree], gvm_eqn)?;
			//println!("NOC Master Manifest {}", manifest);
			let msg = ManifestMsg::new(manifest);
			let packets = msg.to_packets(&self.tree_id)?;
			for packet in packets { noc_to_port.send(packet)?; }
		} else {
			return return Err(NocError::AllowedTree { func_name: S("control"), tree_name: self.base_tree.get_name().clone() }.into());
		}
		Ok(())
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<(), Error> {
		loop {
			let input = &noc_from_outside.recv()?;
			println!("Noc: {}", input);
			let manifest = serde_json::from_str::<Manifest>(input)?;
			println!("Noc: {}", manifest);
		}
	}
}
// Errors
use failure::{Error, Fail};
#[derive(Debug, Fail)]
pub enum NocError {
    #[fail(display = "Noc {}: {} is not an allowed tree", func_name, tree_name)]
    AllowedTree { func_name: String, tree_name: String },
    #[fail(display = "Noc {}: {} is not a valid message type for the NOC", func_name, msg_type)]
    MsgType { func_name: String, msg_type: MsgType },
    #[fail(display = "Noc {}: {} is not a valid index in the NOC's list of tree names", func_name, index)]
    Tree { func_name: String, index: usize }
}
/*
error_chain! {
	foreign_links {
		NocToPort(::message_types::NocPortError);
		Recv(::std::sync::mpsc::RecvError);
		Serialize(::serde_json::Error);
	}
	links {
		Datacenter(::datacenter::Error, ::datacenter::ErrorKind);
		Message(::message::Error, ::message::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		UpTree(::uptree_spec::Error, ::uptree_spec::ErrorKind);
	}
	errors { 
		AllowedTree(func_name: String, tree_name: String) {
			display("Noc {}: {} is not an allowed tree", func_name, tree_name)
		}
		MsgType(func_name: String, msg_type: MsgType) {
			display("Noc {}: {} is not a valid message type for the NOC", func_name, msg_type)
		}
		Tree(func_name: String, index: usize) {
			display("Noc {}: {} is not a valid index in the NOC's list of tree names", func_name, index)
		}
	}
}
*/
