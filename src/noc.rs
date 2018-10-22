use std::thread::{JoinHandle, spawn};
use std::sync::mpsc::channel;
use std::collections::{HashMap, HashSet};

use serde_json;

use blueprint::{Blueprint};
use config::{NCELLS, SCHEMA_VERSION, ByteArray, get_geometry};
use dal;
use datacenter::{Datacenter};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use message::{MsgDirection, TcpMsgType, TreeNameMsg};
use message_types::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::{S, TraceHeader, TraceHeaderParams, TraceType, write_err};

const NOC_MASTER_DEPLOY_TREE_NAME:  &'static str = "NocMasterDeploy";
const NOC_AGENT_DEPLOY_TREE_NAME:   &'static str = "NocAgentDeploy";
const NOC_CONTROL_TREE_NAME: &'static str = "NocMasterAgent";
const NOC_LISTEN_TREE_NAME:  &'static str = "NocAgentMaster";

#[derive(Debug, Clone)]
pub struct Noc {
    allowed_trees: HashSet<AllowedTree>,
    noc_to_outside: NocToOutside,
}
impl Noc {
    pub fn new(noc_to_outside: NocToOutside) -> Result<Noc, Error> {
        Ok(Noc { allowed_trees: HashSet::new(), noc_to_outside })
    }
    pub fn initialize(&mut self, blueprint: &Blueprint, noc_from_outside: NocFromOutside,
                      trace_header: &mut TraceHeader) ->
            Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
        let f = "initialize";
        let (rows, cols, geometry) = get_geometry();
        {
            // For reasons I can't understand, the trace record doesn't show up when generated from main.
            let ref trace_params = TraceHeaderParams { module: "src/main.rs", line_no: line!(), function: "MAIN", format: "trace_schema" };
            let trace = json!({ "schema_version": SCHEMA_VERSION, "ncells": NCELLS, "rows": rows, "cols": cols });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params,&trace, f);
        }
        let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
        let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
        let (mut dc, mut join_handles) = self.build_datacenter(blueprint, trace_header).context(NocError::Chain { func_name: "initialize", comment: S("")})?;
        dc.connect_to_noc(port_to_noc, port_from_noc).context(NocError::Chain { func_name: "initialize", comment: S("")})?;
        let join_outside = self.listen_outside(noc_from_outside, noc_to_port.clone())?;
        join_handles.push(join_outside);
        let join_port = self.listen_port(noc_to_port, noc_from_port, trace_header)?;
        join_handles.push(join_port);
        ::utility::sleep(1);
        Ok((dc, join_handles))
    }
    fn build_datacenter(&self, blueprint: &Blueprint, trace_header: &mut TraceHeader) -> Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
        let mut dc = Datacenter::new();
        let join_handles = dc.initialize(blueprint, trace_header).context(NocError::Chain { func_name: "build_datacenter", comment: S("")})?;
        Ok((dc, join_handles))
    }
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
    fn listen_port(&mut self, noc_to_port: NocToPort, noc_from_port: NocFromPort,
            outer_trace_header: &mut TraceHeader) -> Result<JoinHandle<()>, Error> {
        let f = "listen_port";
        let mut noc = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        let join_port = spawn( move ||  {
            let ref mut inner_trace_header= outer_trace_header_clone.fork_trace();
            let _ = noc.listen_port_loop(&noc_to_port, &noc_from_port, inner_trace_header).map_err(|e| write_err("port", e));
            let _ = noc.listen_port(noc_to_port, noc_from_port, inner_trace_header);
        });
        Ok(join_port)
    }
    fn listen_port_loop(&mut self, noc_to_port: &NocToPort, noc_from_port: &NocFromPort,
            trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "listen_port_loop";
        loop {
            let (is_ait, allowed_tree, msg_type, direction, bytes) = noc_from_port.recv().context(NocError::Chain { func_name: "listen_port", comment: S("")})?;
            let serialized = ::std::str::from_utf8(&bytes)?;
            match msg_type {
                TcpMsgType::TreeName => {
                    let msg = serde_json::from_str::<TreeNameMsg>(&serialized).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    let tree_name = msg.get_tree_name();
                    {
                        let ref trace_params = TraceHeaderParams { module: "src/main.rs", line_no: line!(), function: f, format: "noc_from_ca" };
                        let trace = json!({ "msg_type": msg_type, "tree_name": tree_name });
                        let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params,&trace, f);
                    }
                    self.allowed_trees.insert(AllowedTree::new(tree_name));
                    // If this is the first tree, set up NocMaster and NocAgent
                    if self.allowed_trees.len() == 1 {
                        self.create_noc(tree_name, &noc_to_port).context(NocError::Chain { func_name: "listen_port", comment: S("") })?;
                    }
                }
                _ => write_err("Noc: listen_port: {}", NocError::MsgType { func_name: "listen_port", msg_type }.into())
            }
        }
    }
    fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<JoinHandle<()>,Error> {
        let mut noc = self.clone();
        let join_outside = spawn( move || {
            let _ = noc.listen_outside_loop(&noc_from_outside, &noc_to_port).map_err(|e| write_err("outside", e));
        });
        Ok(join_outside)
    }
    fn listen_outside_loop(&mut self, noc_from_outside: &NocFromOutside, _: &NocToPort) -> Result<(), Error> {
        loop {
            let input = &noc_from_outside.recv()?;
            println!("Noc: {}", input);
            let manifest = serde_json::from_str::<Manifest>(input).context(NocError::Chain { func_name: "listen_outside", comment: S("")})?;
            println!("Noc: {}", manifest);
        }
    }
    // Sets up the NOC Master and NOC Agent services on up trees
    fn create_noc(&mut self, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
        // TODO: Avoids race condition of deployment with Discover, remove to debug
        ::utility::sleep(4);
        let is_ait = false;
        // Stack the trees needed to deploy the master and agent and for them to talk master->agent and agent->master
        let noc_master_deploy_tree = AllowedTree::new(NOC_MASTER_DEPLOY_TREE_NAME);
        Noc::noc_master_deploy_tree(&noc_master_deploy_tree, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master deploy")})?;
        let noc_agent_deploy_tree = AllowedTree::new(NOC_AGENT_DEPLOY_TREE_NAME);
        Noc::noc_agent_deploy_tree(&noc_agent_deploy_tree, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent deploy")})?;
        let noc_master_agent = AllowedTree::new(NOC_CONTROL_TREE_NAME);
        let noc_agent_master = AllowedTree::new(NOC_LISTEN_TREE_NAME);
        Noc::noc_master_agent_tree(&noc_master_agent, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc master tree")})?;
        Noc::noc_agent_master_tree(&noc_agent_master, tree_name, noc_to_port).context(NocError::Chain { func_name: "create_noc", comment: S("noc agent tree")})?;
        let allowed_trees = vec![&noc_master_agent, &noc_agent_master];
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
        noc_to_port.send((is_ait, noc_master_deploy_tree.clone(), TcpMsgType::Manifest, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "create_noc", comment: S("NocMaster")})?;
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
        println!("Noc: deploy {} on tree {}", manifest.get_id(), noc_master_deploy_tree);
        let bytes = ByteArray(manifest_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_deploy_tree.clone(), TcpMsgType::Manifest, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "create_noc", comment: S("NocAgent")})?;
        Ok(())
    }
    // Because of packet forwarding, this tree gets stacked on all cells even though only one of them can receive the deployment message
    fn noc_master_deploy_tree(noc_master_deploy_tree: &AllowedTree, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
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
        let gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_MASTER_DEPLOY_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());

        noc_to_port.send((is_ait, noc_master_deploy_tree.clone(), TcpMsgType::StackTree, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_deploy_tree", comment: S("")})?;
        Ok(())
    }
    // For the reasons given in the comments to the following two functions, the agent does not run
    // on the same cell as the master
    fn noc_agent_deploy_tree(noc_agent_deploy_tree: &AllowedTree, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
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
        let gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_AGENT_DEPLOY_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_deploy_tree.clone(), TcpMsgType::StackTree, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_agent_deploy_tree", comment: S("")})?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can receive messages from the master
    fn noc_master_agent_tree(noc_master_agent: &AllowedTree, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_CONTROL_TREE_NAME));
        params.insert( S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops == 0"));
        eqns.insert(GvmEqn::Recv("hops > 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_CONTROL_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_master_agent.clone(), TcpMsgType::StackTree, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        Ok(())
    }
    // I need a more comprehensive GVM to express the fact that the agent running on the same cell as the master
    // can send messages to the master
    fn noc_agent_master_tree(noc_agent_master: &AllowedTree, tree_name: &String, noc_to_port: &NocToPort) -> Result<(), Error> {
        let is_ait = false;
        let mut params = HashMap::new();
        params.insert(S("new_tree_name"), S(NOC_LISTEN_TREE_NAME));
        params.insert( S("parent_tree_name"), S(tree_name));
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Send("hops > 0"));
        eqns.insert(GvmEqn::Recv("hops == 0"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("true"));
        let gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
        let gvm_eqn_ser = serde_json::to_string(&gvm_eqn).context(NocError::Chain { func_name: "noc_master_tree", comment: S("gvm")})?;;
        params.insert(S("gvm_eqn"), gvm_eqn_ser);
        let stack_tree_msg = serde_json::to_string(&params).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
        println!("Noc: stack {} on tree {}", NOC_LISTEN_TREE_NAME, tree_name);
        let bytes = ByteArray(stack_tree_msg.into_bytes());
        noc_to_port.send((is_ait, noc_agent_master.clone(), TcpMsgType::StackTree, MsgDirection::Leafward, bytes)).context(NocError::Chain { func_name: "noc_master_tree", comment: S("")})?;
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
    #[fail(display = "NocError::Kafka {}: {} ", func_name, error)]
    Kafka { func_name: &'static str, error: String },
    #[fail(display = "NocError::MsgType {}: {} is not a valid message type for the NOC", func_name, msg_type)]
    MsgType { func_name: &'static str, msg_type: TcpMsgType },
//    #[fail(display = "NocError::Tree {}: {} is not a valid index in the NOC's list of tree names", func_name, index)]
//    Tree { func_name: &'static str, index: usize }
}
