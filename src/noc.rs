use std::{thread,
          thread::{JoinHandle},
          sync::mpsc::channel,
          collections::{HashSet}};

use crate::app_message::{AppMsgType, AppMessage, AppMsgDirection,
                         AppDeleteTreeMsg, AppInterapplicationMsg, AppQueryMsg,
                         AppManifestMsg, AppStackTreeMsg, AppTreeNameMsg};
use crate::app_message_formats::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromApplication, NocToApplication};
use crate::blueprint::{Blueprint};
use crate::config::{CONTINUE_ON_ERROR, SCHEMA_VERSION, TRACE_OPTIONS};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use crate::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use crate::utility::{ByteArray, CellConfig, S, TraceHeader, TraceHeaderParams, TraceType,
                     get_geometry, vec_from_hashset, write_err};

const NOC_MASTER_DEPLOY_TREE_NAME: &str = "NocMasterDeploy";
const NOC_AGENT_DEPLOY_TREE_NAME:  &str = "NocAgentDeploy";
pub const NOC_CONTROL_TREE_NAME:   &str = "NocMasterAgent";
pub const NOC_LISTEN_TREE_NAME:    &str = "NocAgentMaster";

#[derive(Debug, Clone)]
pub struct Noc {
    base_tree: Option<AllowedTree>,
    allowed_trees: HashSet<AllowedTree>,
    noc_to_application: NocToApplication,
}
impl Noc {
    pub fn get_name(&self) -> &str { "NOC" }
    pub fn new(noc_to_application: NocToApplication) -> Result<Noc, Error> {
        Ok(Noc { base_tree: None, allowed_trees: HashSet::new(), noc_to_application })
    }
    pub fn initialize(&mut self, blueprint: &Blueprint, noc_from_application: NocFromApplication)
            -> Result<(PortToNoc, PortFromNoc), Error> {
        let _f = "initialize";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                // For reasons I can't understand, the trace record doesn't show up when generated from main.
                let (rows, cols, _geometry) = get_geometry(blueprint.get_ncells());
                let trace_params = &TraceHeaderParams { module: "src/main.rs", line_no: line!(), function: "MAIN", format: "trace_schema" };
                let trace = json!({ "schema_version": SCHEMA_VERSION, "ncells": blueprint.get_ncells(), "rows": rows, "cols": cols });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
        let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
        self.listen_application(noc_from_application, noc_to_port.clone())?;
        self.listen_port(noc_to_port, noc_from_port)?;
        Ok((port_to_noc, port_from_noc))
    }
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}

    // SPAWN THREAD (listen_port_loop)
    fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort)
            -> Result<JoinHandle<()>, Error> {
        let _f = "listen_port";
        let mut noc = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_port_loop", self.get_name()); // NOC NOC
        let join_port = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = noc.listen_port_loop(&noc_to_port, &noc_from_port).map_err(|e| write_err("port", &e));
            if CONTINUE_ON_ERROR { let _ = noc.listen_port(noc_to_port, noc_from_port); }
        });
        Ok(join_port?)
    }

    // WORKER (NocToPort)
    fn listen_port_loop(&mut self, noc_to_port: &NocToPort, noc_from_port: &NocFromPort)
            -> Result<(), Error> {
        let _f = "listen_port_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let bytes = noc_from_port.recv().context(NocError::Chain { func_name: _f, comment: S("")})?;
            let serialized = bytes.to_string()?;
            let app_msg: Box<dyn AppMessage> = serde_json::from_str(&serialized).context(NocError::Chain { func_name: _f, comment: S("") })?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                    let trace_params = &TraceHeaderParams { module: "src/noc.rs", line_no: line!(), function: _f, format: "noc_from_port" };
                    let trace = json!({ "id": self.get_name(), "app_msg": app_msg });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            app_msg.process_noc(self, noc_to_port)?;
        }
    }
    pub fn app_process_delete_tree(&self, _msg: &AppDeleteTreeMsg, _noc_to_port: &NocToPort) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn app_process_interapplication(&self, _msg: &AppInterapplicationMsg, _noc_to_port: &NocToPort) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn app_process_query(&self, _msg: &AppQueryMsg, _noc_to_port: &NocToPort) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn app_process_manifest(&self, _msg: &AppManifestMsg, _noc_to_port: &NocToPort) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn app_process_stack_tree(&self, _msg: &AppStackTreeMsg, _noc_to_port: &NocToPort) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn app_process_tree_name(&mut self, msg: &AppTreeNameMsg, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "app_process_tree_name";
        let tree_name = msg.get_tree_name();
        self.allowed_trees.insert(tree_name.clone());
        if self.allowed_trees.len() == 1 {
            self.base_tree = Some(tree_name.clone());
            self.stack_trees(tree_name, &noc_to_port).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
        }
        if self.allowed_trees.len() == 4 {
            self.deploy_agent(&AllowedTree::new(NOC_AGENT_DEPLOY_TREE_NAME), noc_to_port)?;
            self.deploy_master(&AllowedTree::new(NOC_MASTER_DEPLOY_TREE_NAME), noc_to_port)?;
        }
        Ok(())
    }
    // SPAWN THREAD (listen_application_loop)
    fn listen_application(&mut self, noc_from_application: NocFromApplication, noc_to_port: NocToPort)
            -> Result<JoinHandle<()>,Error> {
        let mut noc = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_application_loop", self.get_name()); // NOC NOC
        let join_application = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = noc.listen_application_loop(&noc_from_application, &noc_to_port).map_err(|e| write_err("application", &e));
            if CONTINUE_ON_ERROR { }
        });
        Ok(join_application?)
    }

    // WORKER (NocFromApplication)
    fn listen_application_loop(&mut self, noc_from_application: &NocFromApplication, _: &NocToPort)
            -> Result<(), Error> {
        let _f = "listen_application_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let input = &noc_from_application.recv()?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "noc_from_application" };
                    let trace = json!({ "id": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()), "input": input });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            println!("Noc: {}", input);
            let manifest = serde_json::from_str::<Manifest>(input).context(NocError::Chain { func_name: "listen_application", comment: S("")})?;
            println!("Noc: {}", manifest);
        }
    }
    fn deploy_master(&self, master_deploy: &AllowedTree, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "deploy_master";
        
        let mut allowed_trees = self.allowed_trees.clone();
        allowed_trees.remove(&self.base_tree.clone().expect("Base tree must be defined by now"));
        let allowed_trees = vec_from_hashset(&allowed_trees);
        let up_tree = UpTreeSpec::new("NocMaster", vec![0]).context(NocError::Chain { func_name: _f, comment: S("NocMaster") })?;
        let service = ContainerSpec::new("NocMaster", "NocMaster", vec![], &allowed_trees).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster") })?;
        let vm_spec = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                                  &allowed_trees, vec![&service], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
        let manifest = Manifest::new("NocMaster", CellConfig::Large, &master_deploy, &allowed_trees,
                                     vec![&vm_spec], vec![&up_tree]).context(NocError::Chain { func_name: _f, comment: S("NocMaster")})?;
        let allowed_trees= manifest.get_allowed_trees();
        let deploy_msg = AppManifestMsg::new("Noc", false,
                                             &master_deploy, &manifest,
                                             &allowed_trees);
        println!("Noc: deploy {} on tree {}", manifest.get_id(), master_deploy);
        self.send_msg(&deploy_msg, noc_to_port)?;
        Ok(())
    }
    fn deploy_agent(&self, agent_deploy: &AllowedTree, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "deploy_agent";
        let allowed_trees = vec![AllowedTree::new(NOC_CONTROL_TREE_NAME),
                                                      AllowedTree::new(NOC_LISTEN_TREE_NAME)];
        let up_tree = UpTreeSpec::new("NocAgent", vec![0]).context(NocError::Chain { func_name: _f, comment: S("NocAgent") })?;
        let service = ContainerSpec::new("NocAgent", "NocAgent", vec![], &allowed_trees).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent") })?;
        let vm_spec = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                                  &allowed_trees, vec![&service], vec![&up_tree]).context(NocError::Chain { func_name: _f, comment: S("NocAgent")})?;
        let manifest = Manifest::new("NocAgent", CellConfig::Large, agent_deploy, &allowed_trees,
                                     vec![&vm_spec], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        let deploy_msg = AppManifestMsg::new("Noc", false,
                                             &agent_deploy, &manifest,
                                             &allowed_trees);
        println!("Noc: deploy {} on tree {}", manifest.get_id(), agent_deploy);
        self.send_msg(&deploy_msg, noc_to_port)?;
        Ok(())
    }
    fn stack_trees(&mut self, base_tree_name: &AllowedTree, noc_to_port: &NocToPort) -> Result<usize, Error> {
        let noc_master_agent = AllowedTree::new(NOC_CONTROL_TREE_NAME);
        let noc_agent_master = AllowedTree::new(NOC_LISTEN_TREE_NAME);
        let noc_master_deploy= AllowedTree::new(NOC_MASTER_DEPLOY_TREE_NAME);
        let noc_agent_deploy = AllowedTree::new(NOC_AGENT_DEPLOY_TREE_NAME);
        self.noc_master_agent_tree(&noc_master_agent, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree")})?;
        self.noc_agent_master_tree(&noc_agent_master, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent tree")})?;
        self.noc_agent_deploy_tree(&noc_agent_deploy, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent tree")})?;
        self.noc_master_deploy_tree(&noc_master_deploy, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree")})?;
        Ok(4)
    }
    fn noc_master_deploy_tree(&self, noc_master_deploy: &AllowedTree, parent_tree_name: &AllowedTree,
                              noc_to_port: &NocToPort) -> Result<(), Error> {
        // Tree for deploying the NocMaster, which only runs on the border cell connected to this instance of Noc
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops == 0"));
        eqns.insert(GvmEqn::Xtnd("false"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let stack_tree_msg = AppStackTreeMsg::new("Noc",
                      noc_master_deploy, parent_tree_name,
                                                  AppMsgDirection::Leafward, &gvm_eqn);
        println!("Noc: stack {} on tree {}", NOC_MASTER_DEPLOY_TREE_NAME, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
    }
    // For the reasons given in the comments to the following two functions, the agent does not run
    // on the same cell as the master
    fn noc_agent_deploy_tree(&self, noc_agent_deploy: &AllowedTree, parent_tree_name: &AllowedTree, noc_to_port: &NocToPort) -> Result<(), Error> {
        // Stack a tree for deploying the NocAgents, which run on all cells, including the one running the NocMaster
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops > 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let stack_tree_msg = AppStackTreeMsg::new("Noc",
                                                  noc_agent_deploy, parent_tree_name,
                                                  AppMsgDirection::Leafward, &gvm_eqn);
        println!("Noc: stack {} on tree {}", NOC_AGENT_DEPLOY_TREE_NAME, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can receive messages from the master
    fn noc_master_agent_tree(&self, noc_master_agent: &AllowedTree, parent_tree_name: &AllowedTree, noc_to_port: &NocToPort) -> Result<(), Error> {
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops > 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let stack_tree_msg = AppStackTreeMsg::new("Noc",
                   noc_master_agent, parent_tree_name,
                                 AppMsgDirection::Leafward, &gvm_eqn);
        println!("Noc: stack {} on tree {}", NOC_CONTROL_TREE_NAME, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can send messages to the master
    fn noc_agent_master_tree(&self, noc_agent_master: &AllowedTree, parent_tree_name: &AllowedTree, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "noc_agent_master_tree";
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops > 0"));
        eqns.insert(GvmEqn::Recv("hops == 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let stack_tree_msg = AppStackTreeMsg::new("Noc",
                                                  noc_agent_master, parent_tree_name,
                                                  AppMsgDirection::Leafward, &gvm_eqn);
        println!("Noc: stack {} on tree {}", NOC_LISTEN_TREE_NAME, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
    }
    fn send_msg(&self, msg: &dyn AppMessage, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "send_msg";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "noc_to_port" };
                let trace = json!({ "app_msg": msg });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let serialized = serde_json::to_string(msg as &dyn AppMessage)?;
        let bytes = ByteArray::new(&serialized);
        noc_to_port.send(bytes).context(NocError::Chain { func_name: _f, comment: S("") })?;
        Ok(())
    }
}
// Errors
use failure::{Error, ResultExt, Fail};

#[derive(Debug, Fail)]
pub enum NocError {
    #[fail(display = "NocError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "NocError::AllowedTree {}: {} is not an allowed tree", func_name, tree_name)]
//    AllowedTree { func_name: &'static str, tree_name: String },
//    #[fail(display = "NocError::Message {}: Message type {} is malformed {}", func_name, msg_type, message)]
//    Message { func_name: &'static str, msg_type: MsgType, message: String },
    #[fail(display = "NocError::MsgType {}: {} is not a valid message type for the NOC", func_name, msg_type)]
    MsgType { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "NocError::Tree {}: {} is not a valid index in the NOC's list of tree names", func_name, index)]
//    Tree { func_name: &'static str, index: usize }
}
