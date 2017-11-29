use std::collections::HashMap;
use std::sync::mpsc::channel;

use failure::{Error, Fail, ResultExt};

use container::{Container, Service};
use message_types::{VmToCa, VmFromCa, VmToContainerMsg, VmToContainer, ContainerFromVm,
	ContainerToVmMsg, ContainerToVm, VmFromContainer, ContainerVmError};
use name::{ContainerID, TreeID, UptreeID, VmID};
use uptree_spec::{AllowedTree, ContainerSpec};
use utility::write_err;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VirtualMachine {
	id: VmID,
	containers: Vec<ContainerSpec>,
}
impl VirtualMachine {
	pub fn new(id: VmID) -> VirtualMachine {
		VirtualMachine { id: id, containers: Vec::new() }
	}
	pub fn initialize(&mut self, up_tree_id: &TreeID, tree_map: &Vec<&str>,
			containers: &Vec<ContainerSpec>) -> Result<(), Error> {
		//self.listen_ca(vm_from_ca)?;
/*		
		while services.len() > 0 {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let service = services.pop().unwrap();
			let name = format!("Container:{}+{}", self.id, self.containers.len() + 1);
			let container_id = ContainerID::new(&name)?;
			let container = Container::new(container_id.clone(), service);
			container.initialize(up_tree_id, tree_ids, container_to_vm, container_from_vm)?;
			//self.containers.insert(up_tree_id.clone(), vec![vm_to_container]);
			self.listen_container(container_id, vm_from_container, vm_to_ca.clone())?;
		}
*/		
		Ok(())
	}
	pub fn get_id(&self) -> &VmID { &self.id }	
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<(), Error, > {
		println!("VM {}: listening to Ca", self.id);
		let vm = self.clone();
		::std::thread::spawn( move || -> Result<(), Error> {
			let _ = vm.listen_ca_loop(vm_from_ca).map_err(|e| write_err("vm", e));
			Ok(())
		});
		Ok(())
	}	
	fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer, 
			vm_to_ca: VmToCa) -> Result<(), Error> {
		println!("VM {}: listening to container {}", self.id, container_id);
		let vm = self.clone();
		::std::thread::spawn( move || -> Result<(), Error> {
			let msg = vm.listen_container_loop(vm_from_container, vm_to_ca).map_err(|e| write_err("vm", e));
			Ok(())
		});		
		Ok(())	
	}	
	fn listen_ca_loop(&self, vm_from_ca: VmFromCa) -> Result<(), Error> {
		loop {
			let msg = vm_from_ca.recv()?;
			
		}
	}
	fn listen_container_loop(&self, vm_from_container: VmFromContainer, vm_to_ca: VmToCa) -> Result<(), Error> {
		loop {
			let msg = vm_from_container.recv()?;
			vm_to_ca.send(msg)?;
		}
	}
}
