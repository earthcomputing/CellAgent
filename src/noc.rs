use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::collections::{HashMap, HashSet};
use std::time;

use serde_json;

use blueprint::{Blueprint};
use config::{SEPARATOR, CellNo, DatacenterNo, Edge, PortNo};
use datacenter::{Datacenter};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use message::{Message, MsgType, ManifestMsg};
use message_types::{NocToPort, NocPortError, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use name::TreeID;
use packet::{PacketAssembler, PacketAssemblers};
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::S;

#[derive(Debug, Clone)]
pub struct Noc {
	tree_id: TreeID, 
	tree_names: Vec<String>,
	noc_to_outside: NocToOutside,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(noc_to_outside: NocToOutside) -> Result<Noc> {
		let tree_id = TreeID::new("CellAgentTree")?;
		Ok(Noc { tree_id: tree_id, tree_names: Vec::new(), packet_assemblers: PacketAssemblers::new(),
				 noc_to_outside: noc_to_outside })
	}
	pub fn initialize(&self, blueprint: &Blueprint, noc_from_outside: NocFromOutside) -> Result<Vec<JoinHandle<()>>> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, mut join_handles) = self.build_datacenter(blueprint)?;
		dc.connect_to_noc(port_to_noc, port_from_noc)?;
		let mut noc = self.clone();
		let noc_to_port_clone = noc_to_port.clone();
		let join_outside = spawn( move || { 
			let _ = noc.listen_outside(noc_from_outside, noc_to_port_clone).map_err(|e| noc.write_err("outside", e));
		});
		join_handles.push(join_outside);
		let mut noc = self.clone();
		let noc_to_port_clone = noc_to_port.clone();
		let join_port = spawn( move || {
			let _ = noc.listen_port(noc_to_port_clone, noc_from_port).map_err(|e| noc.write_err("port", e));	
		});
		join_handles.push(join_port);
		let nap = time::Duration::from_millis(1000);
		sleep(nap);
		println!("{}", dc);
		let noc_to_port_clone = noc_to_port.clone();
		Ok(join_handles)
	}
	fn build_datacenter(&self, blueprint: &Blueprint) 
			-> Result<(Datacenter, Vec<JoinHandle<()>>)> {
		let mut dc = Datacenter::new();
		let join_handles = dc.initialize(blueprint)?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort) -> Result<()> {
		loop {
			let packet = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets)?;
				match msg.get_header().get_msg_type() {
					MsgType::TreeName => {
						self.tree_names.push(msg.get_payload().get_tree_name().clone());
						self.control(&noc_to_port)?;						
					}
					_ => return Err(ErrorKind::MsgType(S("listen_port"), msg.get_header().get_msg_type()).into())
				}
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	// Sets up the NOC Master and NOC Client services on up trees
	fn control(&self, noc_to_port: &NocToPort) -> Result<()> { 
		// Create an up tree on the border cell for the NOC Master
		if let Some(deployment_tree) = self.tree_names.get(0) {
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("false"));
			eqns.insert(GvmEqn::Xtnd("false"));
			eqns.insert(GvmEqn::Save("false"));
			let ref gvm_eqn = GvmEquation::new(eqns, Vec::new());	
			let vm_uptree = UpTreeSpec::new("NocMasterTreeVm", vec![0])?;
			let container_uptree = UpTreeSpec::new("NocMasterTreeContainer", vec![0])?;
			let ref base_tree = AllowedTree::new(deployment_tree);
			let noc_container = ContainerSpec::new("NocMaster", "NocMaster", vec![], vec![base_tree])?;
			let noc_vm = VmSpec::new("NocVM", "Ubuntu", CellConfig::Large, 
				vec![base_tree], vec![&noc_container], vec![&container_uptree])?;
			let ref manifest = Manifest::new("NocMaster", CellConfig::Large, deployment_tree, vec![base_tree], vec![&noc_vm], vec![&vm_uptree], gvm_eqn)?;
			//println!("NOC Master Manifest {}", manifest);
			let msg = ManifestMsg::new(manifest);
			let packets = msg.to_packets(&self.tree_id)?;
			for packet in packets { noc_to_port.send(packet)?; }
		} else {
			return Err(ErrorKind::Tree(S("control"), 0).into());
		}
		Ok(())
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<()> {
		loop {
			let input = &noc_from_outside.recv()?;
			println!("{}", input);
			let manifest = serde_json::from_str::<Manifest>(input);
			println!("{:?}", manifest);
		}
	}
	fn write_err(&self, s: &str, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Noc {} error: {}", s, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
	}
}
// Errors
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
		MsgType(func_name: String, msg_type: MsgType) {
			display("Noc {}: {} is not a valid message type for the NOC", func_name, msg_type)
		}
		Tree(func_name: String, index: usize) {
			display("Noc {}: {} is not a valid index in the NOC's list of tree names", func_name, index)
		}
	}
}
