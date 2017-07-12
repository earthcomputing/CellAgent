use std::collections::HashMap;
use std::sync::mpsc::channel;
use container::{Container, Service};
use message_types::{VmToCa, VmFromCa, VmToContainerMsg, VmToContainer, ContainerFromVm,
	ContainerToVmMsg, ContainerToVm, VmFromContainer, ContainerVmError};
use name::{ContainerID, TreeID, UpTreeID, VmID};

#[derive(Debug, Clone)]
pub struct VirtualMachine {
	id: VmID,
	containers: HashMap<Service, VmToContainer>,
}
impl VirtualMachine {
	pub fn new(id: VmID) -> VirtualMachine {
		VirtualMachine { id: id, containers: HashMap::new() }
	}
	pub fn initialize(&mut self, services: &mut Vec<Service>,
			up_tree_id: &UpTreeID, tree_ids: &Vec<TreeID>, 
			vm_to_ca: VmToCa, vm_from_ca: VmFromCa) -> Result<()> {
		self.listen_ca(vm_from_ca).chain_err(|| ErrorKind::VirtualMachineError)?;
		while services.len() > 0 {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let service = services.pop().unwrap();
			let container_id = ContainerID::new(&format!("Container:{}+{}", self.id, self.containers.len() + 1)).chain_err(|| ErrorKind::VirtualMachineError)?;
			let container = Container::new(container_id.clone(), service);
			container.initialize(up_tree_id, tree_ids, container_to_vm, container_from_vm)?;
			self.containers.insert(service, vm_to_container);
			self.listen_container(container_id, vm_from_container, &vm_to_ca)?;
		}
		Ok(())
	}
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<()> {
		println!("VM {}: listening to Ca", self.id);
		Ok(())
	}	
	fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer, 
			vm_to_ca: &VmToCa) -> Result<()> {
		
		Ok(())	
	}	
}
error_chain! {
	links {
		Container(::container::Error, ::container::ErrorKind);
	}
	errors { VirtualMachineError
		
	}
}