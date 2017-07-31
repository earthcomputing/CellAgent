use std::collections::HashMap;
use std::sync::mpsc::channel;

use container::{Container, Service};
use message_types::{VmToCa, VmFromCa, VmToContainerMsg, VmToContainer, ContainerFromVm,
	ContainerToVmMsg, ContainerToVm, VmFromContainer, ContainerVmError};
use name::{ContainerID, TreeID, UpTraphID, VmID};

#[derive(Debug, Clone)]
pub struct VirtualMachine {
	id: VmID,
	containers: HashMap<UpTraphID, Vec<VmToContainer>>,
}
impl VirtualMachine {
	pub fn new(id: VmID) -> VirtualMachine {
		VirtualMachine { id: id, containers: HashMap::new() }
	}
	pub fn initialize(&mut self, services: &mut Vec<Service>,
			up_tree_id: &UpTraphID, tree_ids: &HashMap<&str,TreeID>, 
			vm_to_ca: &VmToCa, vm_from_ca: VmFromCa) -> Result<()> {
		self.listen_ca(vm_from_ca).chain_err(|| ErrorKind::VmError)?;
		while services.len() > 0 {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let service = services.pop().unwrap();
			let container_id = ContainerID::new(&format!("Container:{}+{}", self.id, self.containers.len() + 1)).chain_err(|| ErrorKind::VmError)?;
			let container = Container::new(container_id.clone(), service);
			container.initialize(up_tree_id, tree_ids, container_to_vm, container_from_vm)?;
			self.containers.insert(up_tree_id.clone(), vec![vm_to_container]);
			self.listen_container(container_id, vm_from_container, vm_to_ca.clone())?;
		}
		Ok(())
	}
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<()> {
		println!("VM {}: listening to Ca", self.id);
		let vm = self.clone();
		::std::thread::spawn( move || -> Result<()> {
			let _ = vm.listen_ca_loop(vm_from_ca).chain_err(|| ErrorKind::VmError).map_err(|e| vm.write_err(e));
			Ok(())
		});
		Ok(())
	}	
	fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer, 
			vm_to_ca: VmToCa) -> Result<()> {
		println!("VM {}: listening to container {}", self.id, container_id);
		let vm = self.clone();
		::std::thread::spawn( move || -> Result<()> {
			let msg = vm.listen_container_loop(vm_from_container, vm_to_ca).chain_err(|| ErrorKind::VmError).map_err(|e| vm.write_err(e));
			Ok(())
		});		
		Ok(())	
	}	
	fn listen_ca_loop(&self, vm_from_ca: VmFromCa) -> Result<()> {
		loop {
			let msg = vm_from_ca.recv().chain_err(|| ErrorKind::VmError)?;
			
		}
	}
	fn listen_container_loop(&self, vm_from_container: VmFromContainer, vm_to_ca: VmToCa) -> Result<()> {
		loop {
			let msg = vm_from_container.recv().chain_err(|| ErrorKind::VmError)?;
			vm_to_ca.send(msg).chain_err(|| ErrorKind::VmError)?;
		}
	}
	fn write_err(&self, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "VM {}: {}", self.id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
	}
}
error_chain! {
	foreign_links {
		Receive(::std::sync::mpsc::RecvError);
	}
	links {
		Container(::container::Error, ::container::ErrorKind);
	}
	errors { VmError
		
	}
}