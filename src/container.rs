use std::fmt;
use std::collections::HashMap;

use failure::Error;

use message_types::{ContainerToVm, ContainerFromVm};
use name::{ContainerID, TreeID, UptreeID};

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Container {
	id: ContainerID,
	service: Service,
}
impl Container {
	pub fn new(id: ContainerID, service: Service) -> Container {
		Container { id: id, service: service }
	}
	pub fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
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
	fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
		let f = "initialize";
		self.listen_vm(container_from_vm)
	}
	fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
		let f = "listen_vm";
		let master = self.clone();
		loop {
			let msg = container_from_vm.recv()?;
			println!("Container {}: got msg {}", master.container_id, msg);
		}
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
struct NocAgent {
	container_id: ContainerID
}
impl NocAgent {
	fn new(container_id: &ContainerID) -> NocAgent { NocAgent { container_id: container_id.clone() } }
	fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
			container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
		Ok(())
	}
}
// Errors
/*
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
	}
	errors { 
	}
}
*/