use std::fmt;
use name::ContainerID;
use vm::{ContainerToVm, ContainerFromVm};

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
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Container {
	id: ContainerID,
	service: Service
}
impl Container {
	pub fn new(id: ContainerID, service: Service) -> Container {
		Container { id: id, service: service }
	}
	pub fn initialize(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) {
		match self.service {
			Service::NocMaster => self.noc_master(container_to_vm, container_from_vm),
			Service::NocAgent  => self.noc_agent(container_to_vm, container_from_vm)
		}
	}
	fn noc_master(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) {
		println!("Container {}: NOC Master started", self.id);
	}
	fn noc_agent(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) {
		println!("Container {}: NOC Agent started", self.id);
	}
}
impl fmt::Display for Container {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Container {}: Service {}", self.id, self.service)
	}
}
