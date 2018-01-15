use std::fmt;
use std::collections::{HashMap, HashSet};

use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use nalcell::CellConfig;
use name::{ContainerID, Name, TreeID, UptreeID};
use message::{Message, StackTreeMsg};
use message_types::{ContainerToVm, ContainerFromVm};
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
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
        println!("NocMaster started in container {}", container_id);
        NocMaster { container_id: container_id, name: S(name), container_to_vm: container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    pub fn make_gvm() -> GvmEquation {
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("false"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        GvmEquation::new(eqns, Vec::new())
    }
    pub fn make_manifest(deployment_tree: &AllowedTree) -> Result<Manifest, Error> {
        let new_tree_id = TreeID::new(NOCMASTER)?;
        let ref allowed_tree = AllowedTree::new(new_tree_id.get_name());
        let allowed_trees = vec![allowed_tree];
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("false"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength,"hops")]);
        let vm_uptree = UpTreeSpec::new("NocMasterTreeVm", vec![0]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCMASTER)})?;
        let container_uptree = UpTreeSpec::new("NocMasterTreeContainer", vec![0]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCMASTER) })?;
        let noc_container = ContainerSpec::new("NocMaster", "NocMaster", vec![], &allowed_trees).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCMASTER)})?;
        let noc_vm = VmSpec::new("NocVM", "Ubuntu", CellConfig::Large,
                                 &allowed_trees, vec![&noc_container], vec![&container_uptree]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCMASTER)})?;
        Ok(Manifest::new(NOCMASTER, CellConfig::Large, deployment_tree,
               &allowed_trees, vec![&noc_vm], vec![&vm_uptree]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCMASTER)})?)
        //println!("NOC Master Manifest {}", manifest);

    }
    //	fn get_container_id(&self) -> &ContainerID { &self.container_id }
//	fn get_service(&self) -> Service { self.service }
    pub fn initialize(&self, up_tree_id: &TreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let new_tree_id = up_tree_id.add_component("NocAgent").context(ServiceError::Chain { func_name: "initialize", comment: S("")})?;
        new_tree_id.append2file().context(ServiceError::Chain { func_name: "initialize", comment: S("")})?;
        self.noc_agents(&new_tree_id, up_tree_id).context(ServiceError::Chain { func_name: "initialize", comment: S("")})?;
        self.container_to_vm.send((S("NocMaster"), S("Message from NocMaster"))).context(ServiceError::Chain { func_name: "initialize", comment: S("send to vm")})?;
        self.listen_vm(container_from_vm)
    }
    fn noc_agents(&self, new_tree_id: &TreeID, up_tree_id: &TreeID) -> Result<(), Error> {
        /*
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("false"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(eqns, Vec::new());
        let vm_uptree = UpTreeSpec::new("NocAgentTreeVm", vec![0])?;
        let container_uptree = UpTreeSpec::new("NocAgentTreeContainer", vec![0])?;
        let ref new_tree = AllowedTree::new(new_tree.get_name());
        let noc_container = ContainerSpec::new("NocAgent", "NocAgent", vec![], vec![new_tree])?;
        let noc_vm = VmSpec::new("NocVM", "Ubuntu", CellConfig::Large,
              vec![new_tree], vec![&noc_container], vec![&container_uptree])?;
        let ref manifest = Manifest::new("NocAgent", CellConfig::Large,"Base",
              vec![new_tree_id.get_name()], vec![&noc_vm], vec![&vm_uptree], gvm_eqn)?;
        let msg = StackTreeMsg::new(new_tree_id, up_tree_id.get_name(), &manifest);
        */
        Ok(())
    }
    fn listen_vm(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        let master = self.clone();
        //println!("Service {} on {}: listening to VM", self.name, self.container_id);
        let vm = self.clone();
        ::std::thread::spawn(move || -> Result<(), Error> {
            let _ = master.listen_vm_loop(container_from_vm).map_err(|e| write_err("service", e));
            Ok(())
        });
        Ok(())
    }
    fn listen_vm_loop(&self, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        //println!("NocMaster on container {} waiting for msg from VM", self.container_id);
        loop {
            let (tree, msg) = container_from_vm.recv().context("NocMaster container_from_vm").context(ServiceError::Chain { func_name: "listen_vm_loop", comment: S("recv from vm")})?;
            println!("NocMaster on container {} got msg {} {}", self.container_id, tree, msg);
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
        println!("NocAgent started in container {}", container_id);
        NocAgent { container_id: container_id, name: S(name), container_to_vm: container_to_vm,
            allowed_trees: allowed_trees.clone() }
    }
    pub fn make_manifest(deployment_tree: &AllowedTree) -> Result<Manifest, Error> {
        let new_tree_id = TreeID::new(NOCAGENT)?;
        let ref allowed_tree = AllowedTree::new(new_tree_id.get_name());
        let allowed_trees = vec![allowed_tree];
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("false"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength,"hops")]);
        let vm_uptree = UpTreeSpec::new("NocAgentTreeVm", vec![0]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCAGENT)})?;
        let container_uptree = UpTreeSpec::new("NocAgentTreeContainer", vec![0]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCAGENT)})?;
        let noc_container = ContainerSpec::new("NocAgent", "NocAgent", vec![], &allowed_trees).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCAGENT)})?;
        let noc_vm = VmSpec::new("NocVM", "Ubuntu", CellConfig::Large,
                                 &allowed_trees, vec![&noc_container], vec![&container_uptree]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCAGENT)})?;
        let manifest = Manifest::new(NOCAGENT, CellConfig::Large, deployment_tree,
                         &allowed_trees, vec![&noc_vm], vec![&vm_uptree]).context(ServiceError::Chain { func_name: "make_manifest", comment: S(NOCAGENT)})?;
        Ok(manifest)
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
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum ServiceError {
    #[fail(display = "ServiceError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "ServiceError::NoSuchService {}: No image for service named {}", func_name, service_name)]
    NoSuchService { func_name: &'static str, service_name: String }
}

