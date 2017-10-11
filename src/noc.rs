use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::collections::HashSet;
use std::time;

use serde_json;

use blueprint::{Blueprint};
use config::{SEPARATOR, CellNo, DatacenterNo, Edge, PortNo};
use datacenter::{Datacenter};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use message::{MsgType};
use message_types::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use name::UpTraphID;
use packet::{PacketAssembler, PacketAssemblers};
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};

#[derive(Debug, Clone)]
pub struct Noc {
	id: UpTraphID,
	noc_to_outside: NocToOutside,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(id: &str, noc_to_outside: NocToOutside) -> Result<Noc> {
		let id = UpTraphID::new(id)?;
		Ok(Noc { id: id, packet_assemblers: PacketAssemblers::new(),
				 noc_to_outside: noc_to_outside })
	}
	pub fn initialize(&self, blueprint: Blueprint, noc_from_outside: NocFromOutside) -> Result<Vec<JoinHandle<()>>> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, mut join_handles) = self.build_datacenter(&self.id, blueprint)?;
		dc.connect_to_noc(port_to_noc, port_from_noc)?;
		let mut noc = self.clone();
		let noc_to_port_clone = noc_to_port.clone();
		let join_outside = spawn( move || { 
			let _ = noc.listen_outside(noc_from_outside, noc_to_port).map_err(|e| noc.write_err("outside", e));
		});
		join_handles.push(join_outside);
		let mut noc = self.clone();
		let join_port = spawn( move || {
			let _ = noc.listen_port(noc_from_port).map_err(|e| noc.write_err("port", e));	
		});
		join_handles.push(join_port);
		let nap = time::Duration::from_millis(1000);
		sleep(nap);
		println!("{}", dc);
		self.control(&mut dc, &noc_to_port_clone)?;
		Ok(join_handles)
	}
	// Sets up the NOC Master and NOC Client services on up trees
	fn control(&self, dc: &mut Datacenter, noc_to_port: &NocToPort) -> Result<()> { 
		// Create an up tree on the border cell for the NOC Master
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("false"));
		eqns.insert(GvmEqn::Save("false"));
		let ref gvm_eqn = GvmEquation::new(eqns, Vec::new());	
		let vm_uptree = UpTreeSpec::new("NocMasterTreeVm", vec![0], gvm_eqn)?;
		let container_uptree = UpTreeSpec::new("NocMasterTreeContainer", vec![0], gvm_eqn)?;
		let ref base_tree = AllowedTree::new("BlackTree", gvm_eqn);
		let ref vm_allowed = AllowedTree::new("NocMasterTreeVm", gvm_eqn);
		let ref container_allowed = AllowedTree::new("NocMasterTreeContainer", gvm_eqn);
		let noc_container = ContainerSpec::new("NocMaster", "NocMaster", vec![base_tree])?;
		let noc_vm = VmSpec::new("NocVM", "Ubuntu", vec![base_tree], vec![&noc_container], vec![&container_uptree])?;
		let up_tree_def = Manifest::new("NocMaster", CellConfig::Large, "NocMaster", vec![base_tree], vec![&noc_vm], vec![&vm_uptree], gvm_eqn)?;
		println!("NOC Master Deployment {}", up_tree_def);
		Ok(())
	}
	fn build_datacenter(&self, id: &UpTraphID, blueprint: Blueprint) 
			-> Result<(Datacenter, Vec<JoinHandle<()>>)> {
		let mut dc = Datacenter::new(id,);
		let join_handles = dc.initialize(blueprint)?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_from_port: NocFromPort) -> Result<()> {
		loop {
			let packet = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets)?;
				println!("Noc received {}", msg);
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<()> {
		loop {
			let input = &noc_from_outside.recv()?;
			println!("{}", input);
			let uptree_spec = serde_json::from_str::<Manifest>(input);
			println!("{:?}", uptree_spec);
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
	}
}
