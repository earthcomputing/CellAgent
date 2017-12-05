use std::fmt;
use std::collections::HashMap;

use failure::{Error, ResultExt};

use name::{ContainerID, TreeID, UptreeID};
use message::Message;
use message_types::{ContainerToVm, ContainerFromVm};
use uptree_spec::AllowedTree;
use utility::{S, write_err};

const NOCMASTER: &'static str ="NocMaster";
const NOCAGENT: &'static str = "NocAgent";

#[derive(Debug, Clone)]
pub enum Service {
    NocMaster { service: NocMaster },
    NocAgent { service: NocAgent }
}
impl Service {
    pub fn new(container_id: &ContainerID, service_name: &str, allowed_trees: &Vec<AllowedTree>,
            container_to_vm: ContainerToVm) -> Result<Service, Error> {
        match service_name {
            NOCMASTER => Ok(Service::NocMaster { service: NocMaster::new(container_id.clone(), NOCMASTER, container_to_vm, allowed_trees) }),
            NOCAGENT => Ok(Service::NocAgent { service: NocAgent::new(container_id.clone(), NOCAGENT, container_to_vm, allowed_trees) }),
            _ => Err(ServiceError::NoSuchService { func_name: "create_service", service_name: S(service_name) }.into())
        }
    }
    pub fn initialize(&self, up_tree_id: &TreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        match self {
            &Service::NocMaster { ref service } => service.initialize(up_tree_id, container_from_vm),
            &Service::NocAgent  { ref service } => service.initialize(up_tree_id, container_from_vm)
        }
    }
}
impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.clone() {
            Service::NocMaster { service } => write!(f, "{}", service),
            Service::NocAgent  { service } => write!(f, "{}", service),
        }
    }
}
#[derive(Debug, Clone)]
pub struct NocMaster {
    container_id: ContainerID,
    name: String,
    container_to_vm: ContainerToVm,
    allowed_trees: Vec<AllowedTree>
}
impl NocMaster {
    pub fn new(container_id: ContainerID, name: &str, container_to_vm: ContainerToVm,
               allowed_trees: &Vec<AllowedTree>) -> NocMaster {
        NocMaster { container_id: container_id, name: S(name), container_to_vm: container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    //	fn get_container_id(&self) -> &ContainerID { &self.container_id }
//	fn get_service(&self) -> Service { self.service }
    pub fn initialize(&self, up_tree_id: &TreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        self.container_to_vm.send(S("Message from NocMaster"))?;
        self.listen_vm(container_from_vm)
    }
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let master = self.clone();
        println!("Service {} on {}: listening to VM", self.name, self.container_id);
        let vm = self.clone();
        ::std::thread::spawn(move || -> Result<(), Error> {
            let _ = master.listen_vm_loop(container_from_vm).map_err(|e| write_err("service", e));
            Ok(())
        });
        Ok(())
    }
    fn listen_vm_loop(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        println!("NocMaster on container {} waiting for msg from VM", self.container_id);
        loop {
            let msg = container_from_vm.recv().context("NocMaster container_from_vm")?;
            println!("NocMaster on container {} got msg {}", self.container_id, msg);
        }
    }
}
impl fmt::Display for NocMaster {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} running in {}", self.name, self.container_id)
    }
}
#[derive(Debug, Clone)]
pub struct NocAgent {
    container_id: ContainerID,
    name: String,
    container_to_vm: ContainerToVm,
    allowed_trees: Vec<AllowedTree>
}
impl NocAgent {
    pub fn new(container_id: ContainerID, name: &str, container_to_vm: ContainerToVm,
            allowed_trees: &Vec<AllowedTree>) -> NocAgent {
        NocAgent { container_id: container_id, name: S(name), container_to_vm: container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    pub fn initialize(&self, up_tree_id: &TreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        Ok(())
    }
}
impl fmt::Display for NocAgent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} running in {}", self.name, self.container_id)
    }
}
#[derive(Debug, Fail)]
pub enum ServiceError {
    #[fail(display = "Service {}: No image for service named {}", func_name, service_name)]
    NoSuchService { func_name: &'static str, service_name: String }
}

