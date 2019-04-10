use std::{thread,
          thread::{JoinHandle},
          sync::mpsc::channel,
          collections::{HashMap, HashSet}};

use crate::app_message::{AppMsgType, AppMsgDirection, AppTreeNameMsg};
use crate::app_message_formats::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromApplication, NocToApplication};
use crate::blueprint::{Blueprint};
use crate::config::{CONTINUE_ON_ERROR, RACE_SLEEP, SCHEMA_VERSION, TRACE_OPTIONS,
                    ByteArray, CellConfig, get_geometry};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use crate::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use crate::utility::{S, TraceHeader, TraceHeaderParams, TraceType, sleep, write_err};

const NOC_MASTER_DEPLOY_TREE_NAME:  &str = "NocMasterDeploy";
const NOC_AGENT_DEPLOY_TREE_NAME:   &str = "NocAgentDeploy";
const NOC_CONTROL_TREE_NAME:        &str = "NocMasterAgent";
const NOC_LISTEN_TREE_NAME:         &str = "NocAgentMaster";

#[derive(Debug, Clone)]
pub struct Noc {
    allowed_trees: HashSet<AllowedTree>,
    noc_to_application: NocToApplication,
}
impl Noc {
    pub fn get_name(&self) -> &str { "NOC" }
    pub fn new(noc_to_application: NocToApplication) -> Result<Noc, Error> {
        Ok(Noc { allowed_trees: HashSet::new(), noc_to_application })
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
        //::utility::sleep(1);
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
            let cmd = noc_from_port.recv().context(NocError::Chain { func_name: "listen_port", comment: S("")})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.noc {
                    let trace_params = &TraceHeaderParams { module: "src/noc.rs", line_no: line!(), function: _f, format: "noc_from_port" };
                    let trace = json!({ "id": self.get_name(), "cmd": cmd });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let (_is_ait, _allowed_tree, msg_type, _direction, bytes) = cmd;
            match msg_type {
                AppMsgType::TreeName => {
                    let serialized = ::std::str::from_utf8(&bytes)?;
                    let msg = serde_json::from_str::<AppTreeNameMsg>(&serialized).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    let base_tree_name = msg.get_tree_name();
                    self.allowed_trees.insert(AllowedTree::new(base_tree_name));
                    // If this is the first tree, set up NocMaster and NocAgent
                    if self.allowed_trees.len() == 1 {
                        self.create_noc(base_tree_name, &noc_to_port).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    }
                }
                _ => write_err("Noc: listen_port: {}", &NocError::MsgType { func_name: "listen_port", msg_type }.into())
            }
        }
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
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                    let trace = json!({ "id": self.get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            println!("Noc: {}", input);
            let manifest = serde_json::from_str::<Manifest>(input).context(NocError::Chain { func_name: "listen_application", comment: S("")})?;
            println!("Noc: {}", manifest);
        }
    }
    // Sets up the NOC Master and NOC Agent services on up trees
    fn create_noc(&mut self, base_tree_name: &str, noc_to_port: &NocToPort) -> Result<(), Error> {
        // TODO: Avoids race condition of deployment with Discover, remove to debug
        if RACE_SLEEP > 0 {
            println!("---> Sleeping to let discover finish");
            sleep(RACE_SLEEP);
        }
        let is_ait = false;
        // Stack the trees needed to deploy the master and agent and for them to talk master->agent and agent->master
        let noc_master_deploy_tree = AllowedTree::new(NOC_MASTER_DEPLOY_TREE_NAME);
        Noc::noc_master_deploy_tree(&noc_master_deploy_tree, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master deploy")})?;
        let noc_agent_deploy_tree = AllowedTree::new(NOC_AGENT_DEPLOY_TREE_NAME);
        Noc::noc_agent_deploy_tree(&noc_agent_deploy_tree, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent deploy")})?;
        let noc_master_agent = AllowedTree::new(NOC_CONTROL_TREE_NAME);
        let noc_agent_master = AllowedTree::new(NOC_LISTEN_TREE_NAME);
        Noc::noc_master_agent_tree(&noc_master_agent, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree")})?;
        Noc::noc_agent_master_tree(&noc_agent_master, base_tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent tree")})?;
        let allowed_trees = vec![&noc_master_agent, &noc_agent_master];
        // Sleep to allow tree stacking to finish
        // TODO: Sleep to let stack tree msgs finish before sending application msgs; should be removed
        if RACE_SLEEP > 0 {
            println!("---> Sleeping to let tree stacking finish");
            sleep(RACE_SLEEP);
        }
        // Deploy NocMaster
        let up_tree = UpTreeSpec::new("NocMaster", vec![0]).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster") })?;
        let service = ContainerSpec::new("NocMaster", "NocMaster", vec![], &allowed_trees).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster") })?;
        let vm_spec = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                 &allowed_trees, vec![&service], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
        let manifest = Manifest::new("NocMaster", CellConfig::Large, &noc_master_deploy_tree, &allowed_trees,
                  vec![&vm_spec], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
        let manifest_ser = serde_json::to_string(&manifest).context(NocError::Chain { func_name: "create_noc", comment: S("") })?;
        let mut params = HashMap::new();
        let deployment_tree_name = noc_master_deploy_tree.get_name();
        params.insert(S("deploy_tree_name"), deployment_tree_name.clone());
        params.insert( S("manifest"), manifest_ser);
        let manifest_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
        println!("Noc: deploy {} on tree {}", manifest.get_id(), noc_master_deploy_tree);
        let bytes = ByteArray(manifest_msg.into_bytes());
        noc_to_port.send((is_ait, noc_master_deploy_tree.clone(), AppMsgType::Manifest, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
        // Deploy NocAgent
        let up_tree = UpTreeSpec::new("NocAgent", vec![0]).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent") })?;
        let service = ContainerSpec::new("NocAgent", "NocAgent", vec![], &allowed_trees).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent") })?;
        let vm_spec = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                                  &allowed_trees, vec![&service], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        let manifest = Manifest::new("NocAgent", CellConfig::Large, &noc_agent_deploy_tree, &allowed_trees,
                                     vec![&vm_spec], vec![&up_tree]).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        let manifest_ser = serde_json::to_string(&manifest).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent") })?;
        let mut params = HashMap::new();
        let deployment_tree_name = noc_agent_deploy_tree.get_name();
        params.insert(S("deploy_tree_name"), deployment_tree_name.clone());
        params.insert( S("manifest"), manifest_ser);
        let manifest_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        println!("Noc: deploy {} on tree {}", manifest.get_id(), noc_agent_deploy_tree);
        let bytes = ByteArray(manifest_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_deploy_tree.clone(), AppMsgType::Manifest, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        Ok(())
    }
    // Because of packet forwarding, this tree gets stacked on all cells even though only one of them can receive the deployment message
    fn noc_master_deploy_tree(noc_master_deploy_tree: &AllowedTree, tree_name: &str, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        // Tree for deploying the NocMaster, which only runs on the border cell connected to this instance of Noc
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(noc_master_deploy_tree.get_name()));
        params.insert(S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops == 0"));
        eqns.insert(GvmEqn::Xtnd("false"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_MASTER_DEPLOY_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());

        noc_to_port.send((is_ait, noc_master_deploy_tree.clone(), AppMsgType::StackTree, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("")})?;
        Ok(())
    }
    // For the reasons given in the comments to the following two functions, the agent does not run
    // on the same cell as the master
    fn noc_agent_deploy_tree(noc_agent_deploy_tree: &AllowedTree, tree_name: &str, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        // Stack a tree for deploying the NocAgents, which run on all cells, including the one running the NocMaster
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_AGENT_DEPLOY_TREE_NAME));
        params.insert(S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops > 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_AGENT_DEPLOY_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_deploy_tree.clone(), AppMsgType::StackTree, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("")})?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can receive messages from the master
    fn noc_master_agent_tree(noc_master_agent: &AllowedTree, tree_name: &str, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_CONTROL_TREE_NAME));
        params.insert( S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops > 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_CONTROL_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_master_agent.clone(), AppMsgType::StackTree, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can send messages to the master
    fn noc_agent_master_tree(noc_agent_master: &AllowedTree, tree_name: &str, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_LISTEN_TREE_NAME));
        params.insert( S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops > 0"));
        eqns.insert(GvmEqn::Recv("hops == 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_LISTEN_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_master.clone(), AppMsgType::StackTree, AppMsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        Ok(())
    }
}
// Errors
use failure::{Error, ResultExt};
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
