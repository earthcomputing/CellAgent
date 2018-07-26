use std::fmt;

use config::ByteArray;
use name::{ContainerID, UptreeID};
use message::{MsgDirection, TcpMsgType};
use message_types::{ContainerToVm, ContainerFromVm};
use uptree_spec::{AllowedTree};
use utility::{S, write_err};

pub const NOCMASTER: &'static str ="NocMaster";
pub const NOCAGENT: &'static str = "NocAgent";

#[derive(Debug, Clone)]
pub enum Service {
    NocMaster { service: NocMaster },
    NocAgent { service: NocAgent }
}
impl Service {
    pub fn new(container_id: &ContainerID, service_name: &str, allowed_trees: &Vec<AllowedTree>,
            container_to_vm: ContainerToVm) -> Result<Service, ServiceError> {
        match service_name {
            NOCMASTER => Ok(Service::NocMaster { service: NocMaster::new(container_id.clone(), NOCMASTER, container_to_vm, allowed_trees) }),
            NOCAGENT => Ok(Service::NocAgent { service: NocAgent::new(container_id.clone(), NOCAGENT, container_to_vm, allowed_trees) }),
            _ => Err(ServiceError::NoSuchService { func_name: "create_service", service_name: S(service_name) })
        }
    }
    pub fn initialize(&self, up_tree_id: &UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
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
        //println!("NocMaster started in container {}", container_id);
        NocMaster { container_id, name: S(name), container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    //fn get_container_id(&self) -> &ContainerID { &self.container_id }
    pub fn initialize(&self, _: &UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let f = "initialize";
        println!("Service {} running NocMaster", self.container_id);
        self.listen_vm(container_from_vm)?;
        let msg = S("Hello From Master");
        println!("Service {} sending {}", self.container_id, msg);
        let bytes = ByteArray(msg.into_bytes());
        let is_ait = false;
        self.container_to_vm.send((is_ait, AllowedTree::new("NocMasterAgent"), TcpMsgType::Application, MsgDirection::Leafward, bytes))?;
        Ok(())
    }
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let master = self.clone();
        //println!("Service {} on {}: listening to VM", self.name, self.container_id);
        ::std::thread::spawn(move || -> Result<(), Error> {
            let _ = master.listen_vm_loop(&container_from_vm).map_err(|e| write_err("service", e));
            let _ = master.listen_vm(container_from_vm);
            Ok(())
        });
        Ok(())
    }
    fn listen_vm_loop(&self, container_from_vm: &ContainerFromVm) -> Result<(), Error> {
        let f = "listen_vm_loop";
        loop {
            let (is_ait, msg) = container_from_vm.recv().context("NocMaster container_from_vm").context(ServiceError::Chain { func_name: "listen_vm_loop", comment: S("NocMaster from vm")})?;
            println!("NocMaster on container {} got msg {}", self.container_id, ::std::str::from_utf8(&msg)?);
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
        //println!("NocAgent started in container {}", container_id);
        NocAgent { container_id, name: S(name), container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    pub fn initialize(&self, _up_tree_id: &UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        //let f = "initialize";
        //self.container_to_vm.send((S("NocAgent"), S("Message from NocAgent"))).context(ServiceError::Chain { func_name: f, comment: S("NocAgent") })?;
        println!("Service {} running NocAgent", self.container_id);
        self.listen_vm(container_from_vm)
    }
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let agent = self.clone();
        //println!("Service {} on {}: listening to VM", self.name, self.container_id);
        ::std::thread::spawn(move || -> Result<(), Error> {
            let _ = agent.listen_vm_loop(&container_from_vm).map_err(|e| write_err("service", e));
            let _ = agent.listen_vm(container_from_vm);
            Ok(())
        });
        Ok(())
    }
    fn listen_vm_loop(&self, container_from_vm: &ContainerFromVm) -> Result<(), Error> {
        let f = "listen_vm_loop";
        loop {
            let (is_ait, msg) = container_from_vm.recv().context(ServiceError::Chain { func_name: f, comment: S("Agent recv from vm") })?;
            println!("NocAgent on container {} got msg {}", self.container_id, ::std::str::from_utf8(&msg)?);
            let msg = format!("Reply from {}", self.container_id);
            //println!("Service {} sending {}", self.container_id, msg);
            let bytes = ByteArray(msg.into_bytes());
            let is_ait = false;
            self.container_to_vm.send((is_ait, AllowedTree::new("NocAgentMaster"), TcpMsgType::Application, MsgDirection::Rootward, bytes))?;
        }
    }
}
impl fmt::Display for NocAgent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} running in {}", self.name, self.container_id)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum ServiceError {
    #[fail(display = "ServiceError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "ServiceError::NoSuchService {}: No image for service named {}", func_name, service_name)]
    NoSuchService { func_name: &'static str, service_name: String }
}

