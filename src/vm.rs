use std::collections::HashMap;
use std::sync::mpsc::channel;

use failure::{Error, Fail, ResultExt};

use container::{Container};
use message_types::{VmToCa, VmFromCa, VmToContainerMsg, VmToContainer, ContainerFromVm,
	ContainerToVmMsg, ContainerToVm, VmFromContainer, ContainerVmError};
use name::{ContainerID, TreeID, UptreeID, VmID};
use uptree_spec::{AllowedTree, ContainerSpec};
use utility::write_err;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct VirtualMachine {
	id: VmID,
	container_specs: Vec<ContainerSpec>,
}
impl VirtualMachine {
	pub fn new(id: VmID) -> VirtualMachine {
		VirtualMachine { id: id, container_specs: Vec::new() }
	}
	pub fn initialize(&mut self, up_tree_id: &TreeID, vm_from_ca: VmFromCa, tree_map: &Vec<&str>,
			container_specs: &Vec<ContainerSpec>) -> Result<(), Error> {
		self.listen_ca(vm_from_ca)?;
		for container_spec in container_specs {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let name = format!("Container:{}+{}", self.id, self.container_specs.len() + 1);
			let container_id = ContainerID::new(&name)?;
            let service_name = container_spec.get_image();
            let allowed_trees = container_spec.get_allowed_trees();
            //let up_tree_id = container_spec.get_up_tree_id();
			let container = Container::new(container_id.clone(), allowed_trees,
                 container_to_vm, service_name.as_str());
			//container.initialize(up_tree_id, container_from_vm)?;
			//self.containers.insert(up_tree_id.clone(), vec![vm_to_container]);
			//self.listen_container(container_id, vm_from_container, vm_to_ca.clone())?;
		}

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
