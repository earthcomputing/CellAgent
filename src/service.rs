use name::{ContainerID, TreeID, UpTraphID};
use message::Message;
use message_types::{ContainerToVm, ContainerFromVm, ContainerVmError};
use utility::S;

pub trait Service {
	fn initialize(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()>;
	fn get_container_id(&self) -> &ContainerID;
	fn run_service(&self) -> Result<()>; 
	fn process_msg(&self, Box<Message>) -> Result<()>;
	fn write_err(&self, e: Error);
	fn listen_loop(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()> {
		let f = "listen_loop";
		loop {
			let msg = container_from_vm.recv()?;
		}
	}
}
#[derive(Debug, Clone)]
pub struct NocMaster {
	container_id: ContainerID,
	up_traph_id: UpTraphID,
	tree_ids: Vec<TreeID>
}
impl NocMaster {
	pub fn new(container_id: ContainerID, up_traph_id: UpTraphID, tree_ids: Vec<TreeID>) -> NocMaster {
		NocMaster { container_id: container_id, up_traph_id: up_traph_id, tree_ids: tree_ids }
	}
}
impl Service for NocMaster {
	fn initialize(&self, container_to_vm: ContainerToVm, container_from_vm: ContainerFromVm) -> Result<()> {
		let service = self.clone();
		::std::thread::spawn( move || {
			let _ = service.listen_loop(container_to_vm, container_from_vm).map_err(|e| service.write_err(e));
		});
		self.run_service()
	}
	fn get_container_id(&self) -> &ContainerID { &self.container_id }
	fn run_service(&self) -> Result<()> {
		println!("Container {} running NocMaster", self.container_id);
		Ok(())
	}
	fn process_msg(&self, boxed_msg: Box<Message>) -> Result<()> {
		println!("Container {} processing message {}", self.container_id, boxed_msg);
		Ok(())
	}
	fn write_err(&self, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Container {} running NocMaster error: {}", self.container_id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
	}
}
// Errors
error_chain!{
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
	}
	errors {  
	}
}