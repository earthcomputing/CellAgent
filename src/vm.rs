use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use cellagent::{VmToCa, VmFromCa};
use container::{Container, Service};
use name::{ContainerID, VMID};

pub type VmToContainerMsg = String;
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
pub type ContainerToVmMsg = String;
pub type ContainerToVm = mpsc::Sender<ContainerToVmMsg>;
pub type VmFromContainer = mpsc::Receiver<ContainerToVmMsg>;
pub type ContainerVmError = mpsc::SendError<ContainerToVmMsg>;
#[derive(Debug, Clone)]
pub struct VirtualMachine {
	id: VMID,
	containers: HashMap<Service, VmToContainer>,
}
impl VirtualMachine {
	pub fn new(id: VMID) -> VirtualMachine {
		VirtualMachine { id: id, containers: HashMap::new() }
	}
	pub fn initialize(&mut self, services: &mut Vec<Service>,
			vm_to_ca: VmToCa, vm_from_ca: VmFromCa) -> Result<()> {
		self.listen_ca(vm_from_ca).chain_err(|| ErrorKind::VirtualMachineError)?;
		while services.len() > 0 {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let service = services.pop().unwrap();
			let container_id = ContainerID::new(&format!("Container:{}+{}", self.id, self.containers.len() + 1)).chain_err(|| ErrorKind::VirtualMachineError)?;
			let container = Container::new(container_id.clone(), service);
			container.initialize(container_to_vm, container_from_vm);
			self.containers.insert(service, vm_to_container);
			self.listen_container(container_id, vm_from_container, vm_to_ca.clone())?;
		}
		Ok(())
	}
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<()> {
		println!("VM {}: listening to Ca", self.id);
		Ok(())
	}	
	fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer, 
			vm_to_ca: VmToCa) -> Result<()> {
		
		Ok(())	
	}	
}
error_chain! {
	errors { VirtualMachineError
		
	}
}