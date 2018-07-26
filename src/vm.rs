use std::collections::{HashSet};
use std::sync::mpsc::channel;

use container::{Container};
use message_types::{VmToCa, VmFromCa, VmToContainer, ContainerFromVm,
	ContainerToVm, VmFromContainer};
use name::{Name, ContainerID, UptreeID, VmID};
use uptree_spec::{AllowedTree, ContainerSpec};
use utility::{S, write_err};

#[derive(Debug, Clone)]
pub struct VirtualMachine {
	id: VmID,
    vm_to_ca: VmToCa,
    allowed_trees: Vec<AllowedTree>,
    vm_to_containers: Vec<VmToContainer>,
}
impl VirtualMachine {
	pub fn new(id: &VmID, vm_to_ca: VmToCa, allowed_trees_ref: &Vec<AllowedTree>) -> VirtualMachine {
        //println!("Create VM {}", id);
		VirtualMachine { id: id.clone(), vm_to_ca, allowed_trees:allowed_trees_ref.clone(),
            vm_to_containers: Vec::new() }
	}
	pub fn initialize(&mut self, up_tree_name: &String, vm_from_ca: VmFromCa, _: &HashSet<AllowedTree>,
			container_specs: &Vec<ContainerSpec>) -> Result<(), Error> {
		//println!("VM {} initializing", self.id);
		let up_tree_id = UptreeID::new(up_tree_name).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " up tree id"})?;
		for container_spec in container_specs {
			let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
			let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
			let name = format!("Container:{}+{}", self.id, container_specs.len() + 1);
			let container_id = ContainerID::new(&name).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name())})?;
            let service_name = container_spec.get_image();
			let container = Container::new(&container_id, service_name.as_str(), &self.allowed_trees,
                 container_to_vm).context(VmError::Chain { func_name: "initialize", comment: S("")})?;
			container.initialize(&up_tree_id, container_from_vm).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name())})?;
            self.vm_to_containers.push(vm_to_container);
            // Next line must be inside loop or vm_to_container goes out of scope in listen_container
			self.listen_container(container_id, vm_from_container, self.vm_to_ca.clone()).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " listen_container"})?;
		}
        //println!("VM {}: {} containers", self.id, self.vm_to_containers.len());
        self.listen_ca(vm_from_ca).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " listen_ca"})?;
		Ok(())
	}
	//pub fn get_id(&self) -> &VmID { &self.id }
	fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<(), Error> {
		//println!("VM {}: listening to Ca", self.id);
		let vm = self.clone();
		::std::thread::spawn( move || -> Result<(), Error> {
			let _ = vm.listen_ca_loop(&vm_from_ca).map_err(|e| write_err("vm", e));
			//let _ = vm.listen_ca(vm_from_ca);
			Ok(())
		});
		Ok(())
	}	
	fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer,
			vm_to_ca: VmToCa) -> Result<(), Error> {
		//println!("VM {}: listening to container {}", self.id, container_id);
		let vm = self.clone();
		::std::thread::spawn( move || {
			let _ = vm.listen_container_loop(&container_id, &vm_from_container, &vm_to_ca).map_err(|e| write_err("vm", e));
            //let _ = vm.listen_container(container_id, vm_from_container, vm_to_ca);
		});
		Ok(())
	}	
	fn listen_ca_loop(&self, vm_from_ca: &VmFromCa) -> Result<(), Error> {
		loop {
            let msg = vm_from_ca.recv().context("listen_ca_loop").context(VmError::Chain { func_name: "listen_ca_loop", comment: S(self.id.get_name()) })?;
            //println!("VM {} send to {} containers msg from ca: {}", self.id,  self.vm_to_containers.len(), msg);
            for vm_to_container in &self.vm_to_containers {
                vm_to_container.send(msg.clone()).context(VmError::Chain { func_name: "listen_ca_loop", comment: S("send to container") })?;
            }
		}
	}
	fn listen_container_loop(&self, _: &ContainerID, vm_from_container: &VmFromContainer, vm_to_ca: &VmToCa) -> Result<(), Error> {
		let is_ait = false;
		loop {
			let (is_ait, allowed_tree, msg_type, direction, msg) = vm_from_container.recv().context("listen_container_loop").context(VmError::Chain { func_name: "listen_container_loop", comment: S(self.id.get_name()) + " recv from container"})?;
            //println!("VM {} got from container {} msg {} {} {} {}", self.id, container_id, msg.0, msg.1, msg.2, msg.3);
			vm_to_ca.send((is_ait, allowed_tree, msg_type, direction, msg)).context(VmError::Chain { func_name: "listen_container_loop", comment: S(self.id.get_name()) + " send to ca"})?;
		}
	}
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum VmError {
    #[fail(display = "VmError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "VmError::AllowedTree {}: {} is not an allowed tree for VM {}", func_name, tree, vm_id)]
//    AllowedTree { func_name: &'static str, tree: AllowedTree, vm_id: VmID }
}
