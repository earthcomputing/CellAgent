use std::collections::{HashMap, HashSet};
use std::sync::mpsc::channel;

use failure::{Error, Fail, ResultExt};

use container::{Container};
use message_types::{VmToCa, VmFromCa, VmToContainerMsg, VmToContainer, ContainerFromVm,
	ContainerToVmMsg, ContainerToVm, VmFromContainer, ContainerVmError};
use name::{ContainerID, TreeID, UptreeID, VmID};
use uptree_spec::{AllowedTree, ContainerSpec};
use utility::write_err;

#[derive(Debug, Clone)]
pub struct VirtualMachine {
	id: VmID,
    vm_to_ca: VmToCa,
    allowed_trees: Vec<AllowedTree>,
    containers: HashMap<ContainerID, Vec<VmToContainer>>
}
impl VirtualMachine {
	pub fn new(id: &VmID, vm_to_ca: VmToCa, allowed_trees_ref: &Vec<AllowedTree>) -> VirtualMachine {
		VirtualMachine { id: id.clone(), vm_to_ca: vm_to_ca, allowed_trees:allowed_trees_ref.clone(),
            containers: HashMap::new() }
	}
	pub fn initialize(&mut self, up_tree_id: &TreeID, vm_from_ca: VmFromCa, vm_trees: &HashSet<AllowedTree>,
			container_specs: &Vec<ContainerSpec>) -> Result<(), Error> {
		self.listen_ca(vm_from_ca)?;
        let mut senders = Vec::new();
		for container_spec in container_specs {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let name = format!("Container:{}+{}", self.id, container_specs.len() + 1);
			let container_id = ContainerID::new(&name)?;
            let service_name = container_spec.get_image();
			let container = Container::new(&container_id, service_name.as_str(), &self.allowed_trees,
                 container_to_vm)?;
			container.initialize(up_tree_id, container_from_vm)?;
            senders.push(vm_to_container);
            // Next line must be inside loop or vm_to_container goes out of scope in listen_container
            self.containers.insert(container_id.clone(), senders.clone());
			self.listen_container(container_id, vm_from_container, self.vm_to_ca.clone())?;
		}
		Ok(())
	}
	pub fn get_id(&self) -> &VmID { &self.id }	
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<(), Error> {
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
			let _ = vm.listen_container_loop(container_id, vm_from_container, vm_to_ca).map_err(|e| write_err("vm", e));
			Ok(())
		});
		Ok(())
	}	
	fn listen_ca_loop(&self, vm_from_ca: VmFromCa) -> Result<(), Error> {
		loop {
			let msg = vm_from_ca.recv().context("listen_ca_loop")?;
            println!("VM {} got msg from ca: {}", self.id, msg);
            self.vm_to_ca.send(msg)?;
		}
	}
	fn listen_container_loop(&self, container_id: ContainerID, vm_from_container: VmFromContainer, vm_to_ca: VmToCa) -> Result<(), Error> {
		loop {
			let msg = vm_from_container.recv().context("listen_container_loop")?;
            println!("VM {} got msg from container: {}", self.id, msg);
			vm_to_ca.send(msg)?;
		}
	}
}
#[derive(Debug, Fail)]
pub enum VmError {
    #[fail(display = "VM {}: {} is not an allowed tree for VM {}", func_name, tree, vm_id)]
    AllowedTree { func_name: &'static str, tree: AllowedTree, vm_id: VmID }
}
