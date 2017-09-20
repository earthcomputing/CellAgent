use std::fmt;
use std::collections::HashMap;

use message_types::{ContainerToVm, ContainerFromVm};
use name::{ContainerID, TreeID, UpTraphID};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Container {
	id: ContainerID,
	service: Service,
}
impl Container {
	pub fn new(id: ContainerID, service: Service) -> Container {
		Container { id: id, service: service }
	}
	pub fn initialize(&self, up_tree_id: &UpTraphID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()> {
		match self.service {
			Service::NocMaster => {
				let master = NocMaster::new(&self.id);
				master.initialize(up_tree_id, tree_ids, container_to_vm, container_from_vm)
			}
			Service::NocAgent  => {
				let agent = NocAgent::new(&self.id);
				agent.initialize(up_tree_id, tree_ids, container_to_vm, container_from_vm)
			}
		}
	}
}
impl fmt::Display for Container {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Container {}: Service {}", self.id, self.service)
	}
}
#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
pub enum Service {
	NocMaster,
	NocAgent
}
impl fmt::Display for Service {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Service::NocMaster => write!(f, "NOC Master"),
			Service::NocAgent  => write!(f, "NOC Agent")
		}
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
struct NocMaster {
	container_id: ContainerID,
	service: Service
}
impl NocMaster {
	fn new(container_id: &ContainerID) -> NocMaster { 
		NocMaster { container_id: container_id.clone(), service: Service::NocMaster } 
	}
//	fn get_container_id(&self) -> &ContainerID { &self.container_id }
//	fn get_service(&self) -> Service { self.service }
	fn initialize(&self, up_tree_id: &UpTraphID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()> {
		let f = "initialize";
		self.listen_vm(container_from_vm)
	}
	fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<()> {
		let f = "listen_vm";
		let master = self.clone();
		loop {
			let msg = container_from_vm.recv()?;
			println!("Container {}: got msg {}", master.container_id, msg);
		}
	}
//	fn write_err(&self, e: Error) {
//		use ::std::io::Write;
//		let stderr = &mut ::std::io::stderr();
//		let _ = writeln!(stderr, "Container {} error: {}", self.container_id, e);
//		for e in e.iter().skip(1) {
//			let _ = writeln!(stderr, "Caused by: {}", e);
//		}
//		if let Some(backtrace) = e.backtrace() {
//			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
//		}
//	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
struct NocAgent {
	container_id: ContainerID
}
impl NocAgent {
	fn new(container_id: &ContainerID) -> NocAgent { NocAgent { container_id: container_id.clone() } }
	fn initialize(&self, up_tree_id: &UpTraphID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()> {
		Ok(())
	}
}
// Errors
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
	}
	errors { 
	}
}