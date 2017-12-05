use std::fmt;
use std::collections::HashMap;

use failure::Error;

use name::{ContainerID, TreeID, UptreeID};
use message::Message;
use message_types::{ContainerToVm, ContainerFromVm};
use utility::{S, write_err};

const NOCMASTER: &'static str ="NocMaster";
const NOCAGENT: &'static str = "NocAgent";

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
pub enum Service {
    NocMaster { service: NocMaster },
    NocAgent { service: NocAgent }
}
impl Service {
    pub fn create_service(container_id: &ContainerID, service_name: &str) -> Result<Service, Error> {
        match service_name {
            NOCMASTER => Ok(Service::NocMaster { service: NocMaster::new(container_id.clone(), NOCMASTER) }),
            NOCAGENT => Ok(Service::NocAgent { service: NocAgent::new(container_id.clone(), NOCAGENT) }),
            _ => Err(ServiceError::NoSuchService { func_name: "create_service", service_name: S(service_name) }.into())
        }
    }
}
impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.clone() {
            Service::NocMaster { service } => write!(f, "{}", service),
            Service::NocAgent { service } => write!(f, "{}", service),
        }
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
pub struct NocMaster {
    container_id: ContainerID,
    name: String
}
impl NocMaster {
    pub fn new(container_id: ContainerID, name: &str) -> NocMaster {
        NocMaster { container_id: container_id, name: S(name) }
    }
    //	fn get_container_id(&self) -> &ContainerID { &self.container_id }
//	fn get_service(&self) -> Service { self.service }
    pub fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
                  container_to_vm: &ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let f = "initialize";
        self.listen_vm(container_from_vm)
    }
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let f = "listen_vm";
        let master = self.clone();
        loop {
            let msg = container_from_vm.recv()?;
            println!("NocMaster got msg {}", msg);
        }
    }
}
impl fmt::Display for NocMaster {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} running in {}", self.name, self.container_id)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
pub struct NocAgent {
    container_id: ContainerID,
    name: String
}
impl NocAgent {
    pub fn new(container_id: ContainerID, name: &str) -> NocAgent {
        NocAgent { container_id: container_id, name: S(name)} }
    pub fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
                  container_to_vm: &ContainerToVm, container_from_vm: ContainerFromVm) -> Result<(), Error> {
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

