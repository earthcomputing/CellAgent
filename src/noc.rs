use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::collections::{HashMap, HashSet};
use std::time;

use serde_json;

use blueprint::{Blueprint};
use config::{BASE_TREE_NAME, CONTROL_TREE_NAME, SEPARATOR, CellNo, DatacenterNo, Edge, PortNo, TableIndex};
use datacenter::{Datacenter};
use message::{Message, MsgPayload, MsgType, ManifestMsg, TreeNameMsg};
use message_types::{NocToPort, NocPortError, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside, TCP};
use nalcell::CellConfig;
use name::{Name, TreeID};
use packet::{PacketAssembler, PacketAssemblers};
use service::NocMaster;
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::{S, write_err};

const NOC_MASTER_TREE_NAME: &'static str = "NocMaster";

#[derive(Debug, Clone)]
pub struct Noc {
    allowed_trees: HashSet<AllowedTree>,
	noc_to_outside: NocToOutside,
}
impl Noc {
	pub fn new(noc_to_outside: NocToOutside) -> Result<Noc, Error> {
		Ok(Noc { allowed_trees: HashSet::new(), noc_to_outside: noc_to_outside })
	}
	pub fn initialize(&self, blueprint: &Blueprint, noc_from_outside: NocFromOutside) ->
            Result<Vec<JoinHandle<()>>, Error> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, mut join_handles) = self.build_datacenter(blueprint).context(NocError::Chain { func_name: "initialize", comment: S("")})?;
		dc.connect_to_noc(port_to_noc, port_from_noc).context(NocError::Chain { func_name: "initialize", comment: S("")})?;
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
		println!("---> Change line in noc.rs with ---> to print datacenter"); //println!("{}", dc);
		Ok(join_handles)
	}
	fn build_datacenter(&self, blueprint: &Blueprint) -> Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
		let mut dc = Datacenter::new();
		let join_handles = dc.initialize(blueprint).context(NocError::Chain { func_name: "build_datacenter", comment: S("")})?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort) -> Result<(), Error> {
		loop {
			let (msg_type, serialized) = noc_from_port.recv().context(NocError::Chain { func_name: "listen_port", comment: S("")})?;
            match msg_type {
                MsgType::TreeName => {
                    let msg = serde_json::from_str::<TreeNameMsg>(&serialized).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
					let tree_name = msg.get_payload_tree_name().context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    self.allowed_trees.insert(AllowedTree::new(tree_name));
                    // If this is the first tree, set up NocMaster and NocAgent
                    if self.allowed_trees.len() == 1 {
                        self.create_noc(tree_name, &noc_to_port).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    }
                }
                _ => write_err("Noc: listen_port: {}", NocError::MsgType { func_name: "listen_port", msg_type: msg_type }.into())
            }
		}
	}
	// Sets up the NOC Master and NOC Agent services on up trees
	fn create_noc(&mut self, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
        // Stack a tree for the NocMaster
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_MASTER_TREE_NAME));
        params.insert(S("base_tree_name"), S(tree_name));
        let gvm_eqn_ser = serde_json::to_string(&NocMaster::make_gvm()).context(NocError::Chain { func_name: "create_noc", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "create_noc", comment: S("")})?;
        Ok(noc_to_port.send((MsgType::StackTree, stack_tree_msg)).context(NocError::Chain { func_name: "create_noc", comment: S("")})?)
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<(), Error> {
		loop {
			let input = &noc_from_outside.recv()?;
			println!("Noc: {}", input);
			let manifest = serde_json::from_str::<Manifest>(input).context(NocError::Chain { func_name: "listen_outside", comment: S("")})?;
			println!("Noc: {}", manifest);
		}
	}
}
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum NocError {
	#[fail(display = "NocError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
    #[fail(display = "NocError::AllowedTree {}: {} is not an allowed tree", func_name, tree_name)]
    AllowedTree { func_name: &'static str, tree_name: String },
//    #[fail(display = "NocError::Message {}: Message type {} is malformed {}", func_name, msg_type, message)]
//    Message { func_name: &'static str, msg_type: MsgType, message: String },
    #[fail(display = "NocError::MsgType {}: {} is not a valid message type for the NOC", func_name, msg_type)]
    MsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "NocError::Tree {}: {} is not a valid index in the NOC's list of tree names", func_name, index)]
    Tree { func_name: &'static str, index: usize }
}
