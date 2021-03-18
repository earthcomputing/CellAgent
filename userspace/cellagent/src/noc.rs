use std::{thread,
          thread::{JoinHandle},
          //sync::mpsc::channel,
          collections::{HashMap, HashSet}};
use crossbeam::crossbeam_channel as mpsc;
use crossbeam::crossbeam_channel::unbounded as channel;

use crate::app_message::{AppMsgType, AppMessage, AppMsgDirection,
                         AppDeleteTreeMsg, AppInterapplicationMsg, AppQueryMsg,
                         AppManifestMsg, AppStackTreeMsg, AppTreeNameMsg};
use crate::app_message_formats::{ApplicationNocMsg, NocToApplicationMsg, PortToNocMsg, NocToPortMsg};
use crate::blueprint::{Blueprint, Cell};
use crate::config::{CONFIG, SCHEMA_VERSION};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::name::{CellID};  // CellID used for trace records
use crate::gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use crate::simulated_border_port::{PortToNoc, PortFromNoc};
use crate::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use crate::utility::{ByteArray, CellNo, CellConfig, PortNo, S, TraceHeader, TraceHeaderParams, TraceType,
                     get_geometry, vec_from_hashset, write_err};

const NOC_MASTER_DEPLOY_TREE_NAME: &str = "NocMasterDeploy";
const NOC_AGENT_DEPLOY_TREE_NAME:  &str = "NocAgentDeploy";
pub const NOC_CONTROL_TREE_NAME:   &str = "NocMasterAgent";
pub const NOC_LISTEN_TREE_NAME:    &str = "NocAgentMaster";

pub type NocToPort = mpsc::Sender<NocToPortMsg>;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;

#[derive(Clone, Debug)]
pub struct DuplexNocPortChannel {
    pub noc_to_port: NocToPort,
    pub noc_from_port: NocFromPort,
}

pub type NocToApplication = mpsc::Sender<NocToApplicationMsg>;
pub type NocFromApplication = mpsc::Receiver<ApplicationNocMsg>;
#[derive(Clone, Debug)]
pub struct DuplexNocApplicationChannel {
    pub noc_to_application: NocToApplication,
    pub noc_from_application: NocFromApplication,
}

#[derive(Debug, Clone)]
pub struct Noc {
    cell_id: CellID,
    base_tree: Option<AllowedTree>,
    allowed_trees: HashSet<AllowedTree>,
    deploy_done: bool,
    duplex_noc_application_channel: DuplexNocApplicationChannel,
    duplex_noc_port_channel_cell_port_map: HashMap::<CellNo, HashMap<PortNo, DuplexNocPortChannel>>,
}
impl Noc {
    pub fn new(duplex_noc_port_channel_cell_port_map: HashMap::<CellNo, HashMap<PortNo, DuplexNocPortChannel>>,
               duplex_noc_application_channel: DuplexNocApplicationChannel) -> Result<Noc, Error> {
        let cell_id = CellID::new("Noc")?;
        Ok(Noc { cell_id, base_tree: None, allowed_trees: HashSet::new(), deploy_done: false, duplex_noc_application_channel, duplex_noc_port_channel_cell_port_map })
    }
    pub fn initialize(&mut self, blueprint: &Blueprint)
            -> Result<(), Error> {
        let _f = "initialize";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                // For reasons I can't understand, the trace record doesn't show up when generated from main.
                let (rows, cols) = get_geometry(blueprint.get_ncells());
                let trace_params = &TraceHeaderParams { module: "src/main.rs", line_no: line!(), function: "MAIN", format: "trace_schema" };
                let trace = json!({ "schema_version": SCHEMA_VERSION, "ncells": blueprint.get_ncells(), "rows": rows, "cols": cols });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.listen_application();
        for border_cell in blueprint.get_border_cells() {
            let cell_no = border_cell.get_cell_no();
            for border_port_no in border_cell.get_border_ports() {
                let duplex_noc_port_channel: DuplexNocPortChannel = self.duplex_noc_port_channel_cell_port_map[&cell_no][border_port_no].clone();
                self.listen_port(duplex_noc_port_channel);
            }
        }
        Ok(())
    }
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
    pub fn get_name(&self) -> &str { "NOC" }

    // SPAWN THREAD (listen_port_loop)
    fn listen_port(&mut self, duplex_noc_port_channel: DuplexNocPortChannel) -> JoinHandle<()> {
        let _f = "listen_port";
        let mut noc = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_port", self.get_name()); // NOC NOC
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = noc.listen_port_loop(&duplex_noc_port_channel).map_err(|e| write_err("Noc: port", &e));
            if CONFIG.continue_on_error { noc.listen_port(duplex_noc_port_channel); }
        }).expect("noc listen port failed")
    }

    // WORKER (NocToPort)
    fn listen_port_loop(&mut self, duplex_noc_port_channel: &DuplexNocPortChannel)
            -> Result<(), Error> {
        let _f = "listen_port_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let bytes = duplex_noc_port_channel.noc_from_port.recv().context(NocError::Chain { func_name: _f, comment: S("")})?;
            let serialized = bytes.stringify()?;
            let app_msg: Box<dyn AppMessage> = serde_json::from_str(&serialized).context(NocError::Chain { func_name: _f, comment: S("") })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                    let trace_params = &TraceHeaderParams { module: "src/noc.rs", line_no: line!(), function: _f, format: "noc_from_port" };
                    let trace = json!({ "cell_id": self.cell_id, "app_msg": app_msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            app_msg.process_noc(self, &duplex_noc_port_channel.noc_to_port)?;
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
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                let trace_params = &TraceHeaderParams { module: "src/noc.rs", line_no: line!(), function: _f, format: "app_process_tree_name_msg" };
                let trace = json!({ "cell_id": self.cell_id, "app_msg": msg });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
    // Handle duplicate notifications
        if self.allowed_trees.get(tree_name).is_some() { return Ok(()); }
        let master_agent = AllowedTree::new(NOC_CONTROL_TREE_NAME);
        let agent_master = AllowedTree::new(NOC_LISTEN_TREE_NAME);
        let master_deploy = AllowedTree::new(NOC_MASTER_DEPLOY_TREE_NAME);
        let agent_deploy = AllowedTree::new(NOC_AGENT_DEPLOY_TREE_NAME);
        let deploy_trees = [&master_agent, &agent_master, &master_deploy, &agent_deploy];
        let three_hop = AllowedTree::new("3hop");
        let two_hop = AllowedTree::new("2hop");
        self.allowed_trees.insert(tree_name.clone());
        match &self.base_tree.clone() {
            None => { // First tree name received is assumed to be the base tree
                self.base_tree = Some(tree_name.to_owned());
                self.noc_master_agent_tree(&master_agent, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree") })?;
                self.noc_agent_master_tree(&agent_master, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent tree") })?;
                self.noc_master_deploy_tree(&master_deploy, tree_name, noc_to_port)?;
                self.noc_agent_deploy_tree(&agent_deploy, tree_name, noc_to_port)?;
                self.small_tree(&three_hop, &tree_name, 3, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree") })?;
            },
            Some(_base) => {
                if self.allowed_trees.contains(&three_hop) && *tree_name == three_hop {
                    self.small_tree(&two_hop, &three_hop, 2, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree") })?;
                }
                if self.has_allowed_trees(&deploy_trees) && !self.deploy_done {
                    self.deploy_done = true;
                    self.deploy_master(&master_deploy, noc_to_port)?;
                    self.deploy_agent(&agent_deploy, noc_to_port)?;
                }
            }
        }
        Ok(())
    }
    fn has_allowed_trees(&self, allowed_trees: &[&AllowedTree]) -> bool {
        allowed_trees.iter().all(|&tree| self.allowed_trees.contains(tree))
    }
    // SPAWN THREAD (listen_application_loop)
    fn listen_application(&mut self) -> JoinHandle<()> { // This should eventually take noc_to_port: NocToPort
        let _f = "listen_application";
        let mut noc = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("{} listen_application_loop", self.get_name()); // NOC NOC
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = noc.listen_application_loop().map_err(|e| write_err("Noc: application", &e));
            if CONFIG.continue_on_error { noc.listen_application(); }
        }).expect("noc application thread failed")
    }

    // WORKER (NocFromApplication)
    fn listen_application_loop(&mut self)
            -> Result<(), Error> { // This should eventually take noc_to_port: &NocToPort
        let _f = "listen_application_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let noc_from_application = self.duplex_noc_application_channel.noc_from_application.clone();
        loop {
            let input = &noc_from_application.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "noc_from_application" };
                    let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()), "input": input });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
        let deploy_msg = AppManifestMsg::new("Noc", false, false,
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
        let deploy_msg = AppManifestMsg::new("Noc", false, false,
                                             &agent_deploy, &manifest,
                                             &allowed_trees);
        println!("Noc: deploy {} on tree {}", manifest.get_id(), agent_deploy);
        self.send_msg(&deploy_msg, noc_to_port)?;
        Ok(())
    }
    fn small_tree(&mut self, new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree,
                  hops: usize, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "small_tree";
        // n-hop tree
        let hops_term = &format!("hops < {}", hops + 1);
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send(hops_term));
        eqns.insert(GvmEqn::Recv(hops_term));
        eqns.insert(GvmEqn::Xtnd(hops_term));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(&eqns, &[GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let stack_tree_msg = AppStackTreeMsg::new("Noc", false, false,
                                                  &new_tree_name, &parent_tree_name,
                                                  AppMsgDirection::Leafward, &gvm_eqn);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.svc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "3hop_to_vm" };
                let trace = json!({ "cell_id": self.cell_id, "NocMaster": self.get_name(), "app_msg": stack_tree_msg });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        println!("Noc: stack {} on tree {}", new_tree_name, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
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
        let stack_tree_msg = AppStackTreeMsg::new("Noc", false, false,
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
        let stack_tree_msg = AppStackTreeMsg::new("Noc", false, false,
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
        let stack_tree_msg = AppStackTreeMsg::new("Noc", false, false, 
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
        let stack_tree_msg = AppStackTreeMsg::new("Noc", false, false,
                                                  noc_agent_master, parent_tree_name,
                                                  AppMsgDirection::Leafward, &gvm_eqn);
        println!("Noc: stack {} on tree {}", NOC_LISTEN_TREE_NAME, parent_tree_name);
        self.send_msg(&stack_tree_msg, noc_to_port)?;
        Ok(())
    }
    fn send_msg(&self, msg: &dyn AppMessage, noc_to_port: &NocToPort) -> Result<(), Error> {
        let _f = "send_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "noc_to_port" };
                let trace = json!({"cell_id": self.cell_id, "app_msg": msg });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
