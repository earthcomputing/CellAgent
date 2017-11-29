use failure::Error;

use name::{ContainerID, TreeID, UptreeID};
use message::Message;
use message_types::{ContainerToVm, ContainerFromVm};
use utility::write_err;

pub trait Service {
	fn initialize(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error>;
	fn get_container_id(&self) -> &ContainerID;
	fn run_service(&self) -> Result<(), Error>;
	fn process_msg(&self, Box<Message>) -> Result<(), Error>;
	fn listen_loop(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
		let f = "listen_loop";
		loop {
			let msg = container_from_vm.recv()?;
		}
	}
}
#[derive(Debug, Clone)]
pub struct NocMaster {
	container_id: ContainerID,
	up_tree_id: UptreeID,
	tree_ids: Vec<TreeID>
}
impl NocMaster {
	pub fn new(container_id: ContainerID, up_tree_id: UptreeID, tree_ids: Vec<TreeID>) -> NocMaster {
		NocMaster { container_id: container_id, up_tree_id: up_tree_id, tree_ids: tree_ids }
	}
}
impl Service for NocMaster {
	fn initialize(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
		let service = self.clone();
		::std::thread::spawn( move || {
			let _ = service.listen_loop(container_to_vm, container_from_vm).map_err(|e| write_err("service", e));
		});
		self.run_service()
	}
	fn get_container_id(&self) -> &ContainerID { &self.container_id }
	fn run_service(&self) -> Result<(), Error> {
		println!("Container {} running NocMaster", self.container_id);
		Ok(())
	}
	fn process_msg(&self, boxed_msg: Box<Message>) -> Result<(), Error> {
		println!("Container {} processing message {}", self.container_id, boxed_msg);
		Ok(())
	}
}
