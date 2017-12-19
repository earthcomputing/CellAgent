use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::collections::{HashMap, HashSet};
use std::time;

use serde_json;

use blueprint::{Blueprint};
use config::{BASE_TREE_NAME, CONTROL_TREE_NAME, SEPARATOR, CellNo, DatacenterNo, Edge, PortNo, TableIndex};
use datacenter::{Datacenter};
use message::{Message, MsgPayload, MsgType, ManifestMsg, TreeNameMsgPayload};
use message_types::{NocToPort, NocPortError, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use name::{Name, TreeID};
use packet::{PacketAssembler, PacketAssemblers};
use service::NocMaster;
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::{S, write_err};

#[derive(Debug, Clone)]
pub struct Noc {
	control_tree: AllowedTree,
	base_tree: AllowedTree,
    allowed_trees: HashSet<AllowedTree>,
	noc_to_outside: NocToOutside,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(control_tree_name: &str, base_tree_name: &str, noc_to_outside: NocToOutside) -> Result<Noc, Error> {
        let mut allowed_trees = HashSet::new();
        allowed_trees.insert(AllowedTree::new(control_tree_name));
        allowed_trees.insert(AllowedTree::new(base_tree_name));
		Ok(Noc { packet_assemblers: PacketAssemblers::new(), allowed_trees: allowed_trees,
				 control_tree: AllowedTree::new(control_tree_name),
                 base_tree: AllowedTree::new(base_tree_name),
				 noc_to_outside: noc_to_outside })
	}
	pub fn initialize(&self, blueprint: &Blueprint, noc_from_outside: NocFromOutside) ->
            Result<Vec<JoinHandle<()>>, Error> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, mut join_handles) = self.build_datacenter(blueprint).context(NocError::Chain { func_name: "initialize", comment: ""})?;
		dc.connect_to_noc(port_to_noc, port_from_noc).context(NocError::Chain { func_name: "initialize", comment: ""})?;
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
		println!("---> Change line noc.rs:57 to print datacenter"); //println!("{}", dc);
		Ok(join_handles)
	}
	fn build_datacenter(&self, blueprint: &Blueprint) -> Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
		let mut dc = Datacenter::new();
		let join_handles = dc.initialize(blueprint).context(NocError::Chain { func_name: "build_datacenter", comment: ""})?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort) -> Result<(), Error> {
		loop {
			let (index, packet) = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets).context(NocError::Chain { func_name: "listen_port", comment: ""})?;
                //println!("Noc got msg {}", msg);
				match msg.get_header().get_msg_type() {
					MsgType::TreeName => {
						let (tree_id, index, allowed_trees) = msg.process_noc(&self)?;
						self.control(&tree_id, index, &allowed_trees, &noc_to_port).context(NocError::Chain { func_name: "listen_port", comment: ""})?;
					}
					_ => write_err("Noc: listen_port: {}", NocError::MsgType { func_name: S("listen_port"), msg_type: msg.get_header().get_msg_type() }.into())
				}
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	// Sets up the NOC Master and NOC Agent services on up trees
	fn control(&mut self, tree_id: &TreeID, other_index: TableIndex, allowed_trees: &Vec<AllowedTree>, noc_to_port: &NocToPort) -> Result<(), Error> {
		// Create an up tree on the border cell for the NOC Master
		for allowed_tree in allowed_trees { self.allowed_trees.insert(allowed_tree.to_owned()); }
		//println!("Noc allowed trees {:?}", allowed_trees);
        match self.allowed_trees.get(&self.base_tree) {
            Some(deployment_tree) => {
                //println!("Noc: me {} base {} deployment {}", tree_id, self.base_tree, deployment_tree);
				let ref manifest = NocMaster::make_manifest(deployment_tree).context(NocError::Chain { func_name: "control", comment: ""})?;
                let msg = ManifestMsg::new(manifest);
                let packets = msg.to_packets(tree_id).context(NocError::Chain { func_name: "control", comment: ""})?;
                for packet in packets { noc_to_port.send((other_index, packet)).context(NocError::Chain { func_name: "control", comment: ""})?; }
                Ok(())
            },
            None => Err(NocError::AllowedTree { func_name: S("control"), tree_name: self.base_tree.get_name().clone() }.into())
        }
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<(), Error> {
		loop {
			let input = &noc_from_outside.recv()?;
			println!("Noc: {}", input);
			let manifest = serde_json::from_str::<Manifest>(input).context(NocError::Chain { func_name: "listen_outside", comment: ""})?;
			println!("Noc: {}", manifest);
		}
	}
}
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum NocError {
	#[fail(display = "NocError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: &'static str },
    #[fail(display = "NocError::AllowedTree {}: {} is not an allowed tree", func_name, tree_name)]
    AllowedTree { func_name: String, tree_name: String },
    #[fail(display = "NocError::MsgType {}: {} is not a valid message type for the NOC", func_name, msg_type)]
    MsgType { func_name: String, msg_type: MsgType },
    #[fail(display = "NocError::Tree {}: {} is not a valid index in the NOC's list of tree names", func_name, index)]
    Tree { func_name: String, index: usize }
}
