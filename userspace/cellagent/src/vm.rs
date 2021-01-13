use std::{collections::{HashSet},
          //sync::mpsc::channel,
          thread};
use crossbeam::crossbeam_channel::unbounded as channel;

use crate::app_message_formats::{VmToCa, VmFromCa,
                                 VmToContainer, ContainerFromVm,
                                 ContainerToVm, VmFromContainer};
use crate::config::{CONFIG};
use crate::container::{Container};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::name::{Name, ContainerID, UptreeID, VmID};
use crate::uptree_spec::{AllowedTree, ContainerSpec};
use crate::utility::{S, write_err, TraceHeader, TraceHeaderParams, TraceType};

#[derive(Debug, Clone)]
pub struct VirtualMachine {
    id: VmID,
    vm_to_ca: VmToCa,
    allowed_trees: Vec<AllowedTree>,
    vm_to_containers: Vec<VmToContainer>,
}
impl VirtualMachine {
    pub fn new(id: VmID, vm_to_ca: VmToCa, allowed_trees_ref: &[AllowedTree]) -> VirtualMachine {
        //println!("Create VM {}", id);
        VirtualMachine { id: id, vm_to_ca, allowed_trees: allowed_trees_ref.to_owned(),
            vm_to_containers: Vec::new() }
    }
    pub fn initialize(&mut self, up_tree_name: &str, vm_from_ca: VmFromCa, allowed_trees: &HashSet<AllowedTree>,
            container_specs: &[ContainerSpec]) -> Result<(), Error> {
        //println!("VM {} initializing", self.id);
        let up_tree_id = UptreeID::new(up_tree_name).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " up tree id"})?;
        for container_spec in container_specs {
            let (vm_to_container, container_from_vm): (VmToContainer, ContainerFromVm) = channel();
            let (container_to_vm, vm_from_container): (ContainerToVm, VmFromContainer) = channel();
            let name = format!("Container:{}+{}", self.id, container_specs.len() + 1);
            let container_id = ContainerID::new(&name).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name())})?;
            let service_name = container_spec.get_image();
            let container = Container::new(container_id, service_name.as_str(), allowed_trees,
                 container_to_vm).context(VmError::Chain { func_name: "initialize", comment: S("")})?;
            container.initialize(up_tree_id, container_from_vm).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name())})?;
            self.vm_to_containers.push(vm_to_container);
            // Next line must be inside loop or vm_to_container goes out of scope in listen_container
            self.listen_container(container_id, vm_from_container, self.vm_to_ca.clone()).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " listen_container"})?;
        }
        //println!("VM {}: {} containers", self.id, self.vm_to_containers.len());
        self.listen_ca(vm_from_ca).context(VmError::Chain { func_name: "initialize", comment: S(self.id.get_name()) + " listen_ca"})?;
        Ok(())
    }
    pub fn _get_id(&self) -> &VmID { &self.id }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self, vm_from_ca: VmFromCa) -> Result<(), Error> {
        let _f = "listen_ca";
        //println!("VM {}: listening to Ca", self.id);
        let vm = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("VirtualMachine {} listen_ca_loop", self.id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = vm.listen_ca_loop(&vm_from_ca).map_err(|e| write_err("vm", &e));
            if CONFIG.continue_on_error { let _ = vm.listen_ca(vm_from_ca); }
        })?;
        Ok(())
    }

    // SPAWN THREAD (listen_container_loop)
    fn listen_container(&self, container_id: ContainerID, vm_from_container: VmFromContainer,
            vm_to_ca: VmToCa) -> Result<(), Error> {
        let _f = "listen_container";
        //println!("VM {}: listening to container {}", self.id, container_id);
        let vm = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("VirtualMachine {} listen_container_loop", self.id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = vm.listen_container_loop(container_id, &vm_from_container, &vm_to_ca).map_err(|e| write_err("vm", &e));
            if CONFIG.continue_on_error { let _ = vm.listen_container(container_id, vm_from_container, vm_to_ca); }
        })?;
        Ok(())
    }

    // WORKER (VmFromCa)
    fn listen_ca_loop(&self, vm_from_ca: &VmFromCa) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.vm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let bytes = vm_from_ca.recv().context("listen_ca_loop").context(VmError::Chain { func_name: "listen_ca_loop", comment: S(self.id.get_name()) })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.vm {
                    let msg: Box<dyn AppMessage> = serde_json::from_str(&bytes.to_string()?)?;
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "vm_from_ca" };
                    let trace = json!({ "id": self.id, "msg": msg.to_string() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "vm_to_container" };
                    let trace = json!({ "id": self.id, "msg": msg.to_string() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            //println!("VM {} send to {} containers msg from ca: {}", self.id,  self.vm_to_containers.len(), msg);
            for vm_to_container in &self.vm_to_containers {
                vm_to_container.send(bytes.clone()).context(VmError::Chain { func_name: "listen_ca_loop", comment: S("send to container") })?;
            }
        }
    }

    // WORKER (VmFromContainer)
    fn listen_container_loop(&self, _: ContainerID, vm_from_container: &VmFromContainer, vm_to_ca: &VmToCa)
            -> Result<(), Error> {
        let _f = "listen_container_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.vm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let bytes = vm_from_container.recv().context("listen_container_loop").context(VmError::Chain { func_name: "listen_container_loop", comment: S(self.id.get_name()) + " recv from container"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.vm {
                    let msg: Box<dyn AppMessage> = serde_json::from_str(&bytes.to_string()?)?;
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "vm_from_container" };
                    let trace = json!({ "id": self.id, "msg": msg.to_string() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "vm_to_ca" };
                    let trace = json!({ "id": self.id, "msg": msg.to_string() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            //println!("VM {} got from container {} msg {} {} {} {}", self.id, container_id, msg.0, msg.1, msg.2, msg.3);
            vm_to_ca.send(bytes).context(VmError::Chain { func_name: "listen_container_loop", comment: S(self.id.get_name()) + " send to ca"})?;
        }
    }
}
// Errors
use failure::{Error, ResultExt};
use crate::app_message::AppMessage;

#[derive(Debug, Fail)]
pub enum VmError {
    #[fail(display = "VmError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "VmError::AllowedTree {}: {} is not an allowed tree for VM {}", func_name, tree, vm_id)]
//    AllowedTree { func_name: &'static str, tree: AllowedTree, vm_id: VmID }
}
