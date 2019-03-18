use std::{fmt, thread};

//use reqwest::Client::*;

use crate::app_message_formats::{ContainerToVm, ContainerFromVm};
use crate::app_message::{AppMsgDirection, AppMsgType};
use crate::config::{ByteArray, CONTINUE_ON_ERROR, TRACE_OPTIONS};
use crate::dal;
use crate::dal::{fork_trace_header, update_trace_header};
use crate::name::{ContainerID, UptreeID};
use crate::uptree_spec::{AllowedTree};
use crate::utility::{S, write_err, TraceHeader, TraceHeaderParams, TraceType};

pub const NOCMASTER: &str ="NocMaster";
pub const NOCAGENT: &str = "NocAgent";

#[derive(Debug, Clone)]
pub enum Service {
    NocMaster { service: NocMaster },
    NocAgent { service: NocAgent }
}
impl Service {
    pub fn new(container_id: ContainerID, service_name: &str, allowed_trees: &[AllowedTree],
            container_to_vm: ContainerToVm) -> Result<Service, ServiceError> {
        match service_name {
            NOCMASTER => Ok(Service::NocMaster { service: NocMaster::new(container_id, NOCMASTER, container_to_vm, allowed_trees) }),
            NOCAGENT => Ok(Service::NocAgent { service: NocAgent::new(container_id, NOCAGENT, container_to_vm, allowed_trees) }),
            _ => Err(ServiceError::NoSuchService { func_name: "create_service", service_name: S(service_name) })
        }
    }
    pub fn initialize(&self, up_tree_id: UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        match *self {
            Service::NocMaster { ref service } => service.initialize(up_tree_id, container_from_vm),
            Service::NocAgent  { ref service } => service.initialize(up_tree_id, container_from_vm)
        }
    }
}
impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    pub fn get_name(&self) -> &str { &self.name }
    //pub fn get_id(&self) -> &ContainerID { &self.container_id }
    pub fn new(container_id: ContainerID, name: &str, container_to_vm: ContainerToVm,
               allowed_trees: &[AllowedTree]) -> NocMaster {
        //println!("NocMaster started in container {}", container_id);
        NocMaster { container_id, name: S(name), container_to_vm,
            allowed_trees: allowed_trees.to_owned() }
    }
    //fn get_container_id(&self) -> &ContainerID { &self.container_id }
    pub fn initialize(&self, _: UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let _f = "initialize";
        println!("Service {} running NocMaster", self.container_id);
        self.listen_vm(container_from_vm)?;
        let msg = S("Hello From Master");
        let bytes = ByteArray(msg.into_bytes());
        let is_ait = false;
        self.container_to_vm.send((is_ait, AllowedTree::new("NocMasterAgent"), AppMsgType::Interapplication, AppMsgDirection::Leafward, bytes))?;
        Ok(())
    }
    // SPAWN THREAD (listen_vm_loop)
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let _f = "listen_vm";
        //println!("Service {} on {}: listening to VM", self.name, self.container_id);
        let master = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_vm_loop", self.get_name()); // NOC NOC
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = master.listen_vm_loop(&container_from_vm).map_err(|e| write_err("service", &e));
            if CONTINUE_ON_ERROR { let _ = master.listen_vm(container_from_vm); }
        })?;
        Ok(())
    }
    // WORKER (ContainerFromVm)
    fn listen_vm_loop(&self, container_from_vm: &ContainerFromVm) -> Result<(), Error> {
        let _f = "listen_vm_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.svc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "NocMaster": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let (_is_ait, msg) = container_from_vm.recv().context("NocMaster container_from_vm").context(ServiceError::Chain { func_name: "listen_vm_loop", comment: S("NocMaster from vm")})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.svc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "NocMaster": self.get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let msg = format!("NocMaster on container {} got msg {}", self.container_id, ::std::str::from_utf8(&msg)?);
            println!("{}", msg);
            let foo = reqwest::Client::new()
                .post("http://localhost:8081/")
                .body(msg)
                .send()
                .and_then(|res| { Ok(()/*println!("Response {:?}", res.status())*/)})
                .map_err(|e| { println!("HTTP {:?}", e) });
        }
    }
}
impl fmt::Display for NocMaster {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    pub fn get_name(&self) -> &str { &self.name }
    pub fn new(container_id: ContainerID, name: &str, container_to_vm: ContainerToVm,
            allowed_trees: &[AllowedTree]) -> NocAgent {
        //println!("NocAgent started in container {}", container_id);
        NocAgent { container_id, name: S(name), container_to_vm,
            allowed_trees: allowed_trees.to_owned() }
    }
    pub fn initialize(&self, _up_tree_id: UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        //let _f = "initialize";
        //self.container_to_vm.send((S("NocAgent"), S("Message from NocAgent"))).context(ServiceError::Chain { func_name: _f, comment: S("NocAgent") })?;
        println!("Service {} running NocAgent", self.container_id);
        self.listen_vm(container_from_vm)
    }
    //pub fn get_id(&self) -> &ContainerID { &self.container_id }
    // SPAWN THREAD (listen_vm_loop)
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let _f = "listen_vm";
        //println!("Service {} on {}: listening to VM", self.name, self.container_id);
        let agent = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_vm_loop", self.get_name()); // NOC NOC
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = agent.listen_vm_loop(&container_from_vm).map_err(|e| write_err("service", &e));
            if CONTINUE_ON_ERROR { let _ = agent.listen_vm(container_from_vm); }
        })?;
        Ok(())
    }

    // WORKER (ContainerFromVm)
    fn listen_vm_loop(&self, container_from_vm: &ContainerFromVm) -> Result<(), Error> {
        let _f = "listen_vm_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.svc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "NocAgent": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let (_is_ait, msg) = container_from_vm.recv().context(ServiceError::Chain { func_name: _f, comment: S("Agent recv from vm") })?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.svc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "NocAgent": self.get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            println!("NocAgent on container {} got msg {}", self.container_id, ::std::str::from_utf8(&msg)?);
            let msg = format!("Reply from {}", self.container_id);
            //println!("Service {} sending {}", self.container_id, msg);
            let bytes = ByteArray(msg.into_bytes());
            let is_ait = false;
            self.container_to_vm.send((is_ait, AllowedTree::new("NocAgentMaster"), AppMsgType::Interapplication, AppMsgDirection::Rootward, bytes))?;
        }
    }
}
impl fmt::Display for NocAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

