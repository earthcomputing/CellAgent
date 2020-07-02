use std::{fmt, fmt::Write,
          sync::{Arc, Mutex},
          thread,
          thread::JoinHandle,
          collections::{HashMap, HashSet}};

use bimap::BiMap;
use crossbeam::crossbeam_channel::unbounded as channel;
use serde;
use serde_json;

use crate::app_message::{AppMessage, AppMsgType,
                         AppDeleteTreeMsg, AppInterapplicationMsg, AppManifestMsg,
                         AppQueryMsg, AppStackTreeMsg, AppTreeNameMsg,
                         SenderMsgSeqNo};
use crate::app_message_formats::{CaToPort, PortToCaMsg,
                                 CaToVm, VmFromCa, VmToCa, CaFromVm};
use crate::cmodel::{Cmodel};
use crate::config::{CONFIG, BASE_TREE_NAME, CONNECTED_PORTS_TREE_NAME, CONTROL_TREE_NAME,
                    CellQty, PathLength, PortQty};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{Message, MsgHeader, MsgTreeMap, MsgType,
                        InterapplicationMsg,
                        DeleteTreeMsg,
                        DiscoverMsg, DiscoverDMsg, DiscoverDType,
                        FailoverMsg, FailoverDMsg, FailoverMsgPayload, FailoverResponse,
                        HelloMsg,
                        ManifestMsg,
                        DiscoverAckDMsg, DiscoverAckMsg,
                        StackTreeMsg, StackTreeDMsg,
                        TreeNameMsg};
use crate::ec_message_formats::{CaToCm, CaFromCm, CmToCa, CmFromCa, PeToCm, CmFromPe, CaToCmBytes, CmToCaBytes, PeToPort, PeFromPort };
use crate::gvm_equation::{GvmEquation, GvmEqn};
use crate::name::{Name, CellID, OriginatorID, PortTreeID, TreeID, UptreeID, VmID};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::port_tree::PortTree;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::traph::{PortState, Traph};
use crate::tree::Tree;
use crate::uptree_spec::{AllowedTree, Manifest};
use crate::utility::{BASE_TENANT_MASK, DEFAULT_USER_MASK,
                     ByteArray, CellConfig, CellInfo, CellType, Mask, Path, PortNo,
                     Quench, PortNumber, S,
                     TraceHeader, TraceHeaderParams, TraceType, UtilityError,
                     write_err, new_hashset};
use crate::uuid_ec::Uuid;
use crate::vm::VirtualMachine;

use failure::{Error, ResultExt, Fail};
use crate::app_message_formats::{CaFromPort};

type BorderSenderIDMap = HashMap<PortNumber, OriginatorID>;
type TreeIDNameMap = BiMap<TreeID, AllowedTree>;
pub type PortTreeIDMap = HashMap<Uuid, PortTreeID>;
pub type Traphs = HashMap<Uuid, Traph>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeNameMap = HashMap<OriginatorID, TreeIDNameMap>;
pub type TreeVmMap = HashMap<TreeID, Vec<CaToVm>>;

#[derive(Debug, Clone, Default)]
pub struct CellAgent {
    cell_id: CellID,
    cell_type: CellType,
    config: CellConfig,
    cmodel: Vec<Cmodel>,
    cell_info: CellInfo,
    no_ports: PortQty,
    my_tree_id: TreeID,
    control_tree_id: TreeID,
    connected_tree_id: TreeID,
    my_entry: RoutingTableEntry,
    connected_tree_entry: RoutingTableEntry,
    saved_discover: HashMap<TreeID, DiscoverMsg>,
    saved_discoverd: HashMap<TreeID, DiscoverDMsg>,
    saved_discover_ack_d: HashMap<TreeID, DiscoverAckDMsg>,
    // Next item shared between listen_uptree and listen_cmodel threads
    tree_name_map: Arc<Mutex<TreeNameMap>>,
    traphs: Traphs,
    traphs_mutex: Arc<Mutex<Traphs>>, // Needed so I can print from main() because I have to clone to get self.traphs into the thread
    tree_map: TreeMap, // Base tree for given stacked tree
    border_port_tree_id_map: BorderSenderIDMap, // Find the tree id associated with a border port
    base_tree_map: HashMap<PortTreeID, TreeID>, // Find the black tree associated with any tree, needed for stacking
    tree_id_map: PortTreeIDMap,
    tenant_masks: Vec<Mask>,
    tree_vm_map: TreeVmMap,
    ca_to_vms: HashMap<VmID, CaToVm>,
    ca_to_cm: Vec<CaToCm>,
    ca_to_ports: HashMap<PortNo, CaToPort>,
    vm_id_no: usize,
    up_tree_senders: HashMap<UptreeID, HashMap<String,TreeID>>,
    up_traphs_clist: HashMap<TreeID, TreeID>,
    neighbors: HashMap<PortNo, (CellID, PortNo)>,
    discover_ack_d: HashMap<TreeID, usize>,
    tree_count: usize,
    sent_to_noc: bool,
    discover_sent: HashSet<TreeID>,
    discover_ack_d_sent: HashSet<TreeID>,
    is_border_port_connected: bool,
    discoverd_seen_on_tree: HashMap<TreeID, HashSet<PortNo>>,
    discover_ack_seen_on_tree: HashMap<TreeID, HashSet<PortNo>>,
    parents_seen_on_tree: HashMap<TreeID, usize>,
    discoverd_parent_msg: HashMap<TreeID, (PortNumber, DiscoverDMsg)>,
    discover_breadth_first_msg: HashMap<TreeID, (PortNo, DiscoverMsg)>,
    failover_reply_ports: HashMap<PortTreeID, PortNo>,
    no_packets: Vec<NumberOfPackets>,
    child_ports: HashMap<TreeID, HashSet<PortNo>>,
}
impl CellAgent {
    pub fn new(cell_id: CellID, cell_type: CellType, config: CellConfig, no_ports: PortQty,
               ca_to_ports: HashMap<PortNo, CaToPort>, cm_to_ca: CmToCa, pe_from_ports: PeFromPort, pe_to_ports: HashMap<PortNo, PeToPort>,
               border_port_nos: &HashSet<PortNo>)
               -> Result<(CellAgent, JoinHandle<()>), Error> {
        let _f = "new";
        let tenant_masks = vec![BASE_TENANT_MASK];
        let my_tree_id = TreeID::new(&cell_id.get_name()).context(CellagentError::Chain { func_name: _f, comment: S("my_tree_id") })?;
        let control_tree_id = TreeID::new(&cell_id.get_name())?.
            add_component(CONTROL_TREE_NAME)?;
        let connected_tree_id = TreeID::new(&cell_id.get_name())?
            .add_component(CONNECTED_PORTS_TREE_NAME)?;
        let mut base_tree_map = HashMap::new();
        base_tree_map.insert(my_tree_id.to_port_tree_id_0(), my_tree_id);
        let mut no_packets = Vec::new();
        (1..=(*CONFIG.max_num_phys_ports_per_cell).into())
            .for_each(|_| no_packets.push(NumberOfPackets::new()));
        let my_entry = RoutingTableEntry::default().add_child(PortNumber::default());
        let (ca_to_cm, cm_from_ca): (CaToCm, CmFromCa) = channel();
        let (pe_to_cm, cm_from_pe): (PeToCm, CmFromPe) = channel();
        let (cmodel, _pe_join_handle) = Cmodel::new(cell_id, connected_tree_id, pe_to_cm, cm_to_ca, pe_from_ports, pe_to_ports, border_port_nos);
        let cm_join_handle = cmodel.start(cm_from_ca, cm_from_pe);
        Ok((CellAgent {
            cell_id, my_tree_id, cell_type, config, no_ports,
            control_tree_id, connected_tree_id,
            cmodel: vec![cmodel], ca_to_cm: vec![ca_to_cm], ca_to_ports, my_entry, base_tree_map,
            ..Default::default()
        }, cm_join_handle))
    }

    // SPAWN THREAD (ca.initialize)
    pub fn start(&self, ca_from_cm: CaFromCm, ca_from_ports: CaFromPort) -> JoinHandle<()> {
        let _f = "start_cell";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_start_ca" };
                let trace = json!({ "cell_id": self.get_cell_id() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {}", self.get_cell_id());
        thread::Builder::new().name(thread_name).spawn(move || {
            update_trace_header(child_trace_header);
            let _ = ca.initialize(ca_from_cm, ca_from_ports).map_err(|e| write_err("nalcell", &e));
            if CONFIG.continue_on_error {} // Don't automatically restart cell agent if it crashes
        }).expect("cellagent thread failed")
    }

    // WORKER (CellAgent)
    pub fn initialize(&mut self, ca_from_cm: CaFromCm, ca_from_ports: CaFromPort) -> Result<&mut Self, Error> {
        let _f = "initialize";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        // Set up predefined trees - Must be first two in this order
        let port_number = PortNumber::new0();
        let hops = PathLength(CellQty(0));
        let path = Path::new0();
        let my_tree_id = self.my_tree_id;
        self.tree_id_map.insert(self.control_tree_id.get_uuid(), self.control_tree_id.to_port_tree_id_0());
        self.tree_id_map.insert(self.connected_tree_id.get_uuid(), self.connected_tree_id.to_port_tree_id_0());
        self.tree_id_map.insert(my_tree_id.get_uuid(), my_tree_id.to_port_tree_id_0());
        self.tree_map.insert(self.control_tree_id.get_uuid(), self.control_tree_id.get_uuid());
        self.tree_map.insert(self.connected_tree_id.get_uuid(), self.connected_tree_id.get_uuid());
        self.tree_map.insert(my_tree_id.get_uuid(), my_tree_id.get_uuid());
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        self.update_traph(self.control_tree_id.to_port_tree_id_0(), port_number,
                          PortState::Parent, &gvm_equation,
                          HashSet::new(), hops, path)?;
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("false"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        let connected_tree_entry = self.update_traph(self.connected_tree_id.to_port_tree_id_0(),
                                                     port_number,
                                                     PortState::Parent, &gvm_equation,
                                                     HashSet::new(), hops, path)?;
        self.connected_tree_entry = connected_tree_entry;
        // Create my tree
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(&eqns, &Vec::new());
        self.my_entry = self.update_traph(my_tree_id.to_port_tree_id_0(), port_number,
                                          PortState::Parent, &gvm_eqn,
                                          HashSet::new(), hops, path)?;
        let ca_cm_join_handle: JoinHandle<()> = self.listen_cm(ca_from_cm);
        if self.is_border() {
            self.listen_port(ca_from_ports).join().expect("Error waiting on cellagent ports thread");
        }
        match ca_cm_join_handle.join() { // .context(...)? doesn't compile
            Ok(()) => Ok(self),
            Err(e) => Err(CellagentError::Chain { func_name: _f, comment: format!("{:?}", e) }.into())
        }
    }
    pub fn get_cmodel(&self) -> &Cmodel { &self.cmodel[0] } // Needed in main() to print forwarding tables
    fn get_no_ports(&self) -> PortQty { self.no_ports }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_connected_tree_id(&self) -> TreeID { self.connected_tree_id }
    fn _get_control_tree_id(&self) -> TreeID { self.control_tree_id }
    fn is_border(&self) -> bool { self.cell_type == CellType::Border }
    fn get_no_neighbors(&self) -> usize { self.neighbors.len() }
//    pub fn get_cell_info(&self) -> CellInfo { self.cell_info }
//    pub fn get_tree_name_map(&self) -> &TreeNameMap { &self.tree_name_map }
    fn get_vm_senders(&self, tree_id: TreeID) -> Result<Vec<CaToVm>, Error> {
        let _f = "get_vm_senders";
        self.tree_vm_map
            .get(&tree_id)
            .cloned()
            .ok_or(CellagentError::TreeVmMap { func_name: _f, cell_id: self.cell_id, tree_id }.into())
    }
    fn get_mask(&self, port_tree_id: PortTreeID) -> Result<Mask, Error> {
        let _f = "get_mask";
        Ok(match self.get_traph(port_tree_id) {
            Ok(t) => {
                let entry = t.get_tree_entry(&port_tree_id.get_uuid()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                entry.get_mask()
            },
            Err(_) => Mask::empty().not()
        })
    }
    fn _get_gvm_eqn(&self, port_tree_id: PortTreeID) -> Result<GvmEquation, Error> {
        let _f = "get_gvm_eqn";
        let tree_uuid = port_tree_id.get_uuid();
        let traph = self.get_traph(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let tree = traph.get_tree(&tree_uuid)?;
        let gvm_eqn = tree.get_gvm_eqn().clone();
        Ok(gvm_eqn.clone())
    }
    fn enough_ports(&self, tree_id: TreeID, no_seen_ports: usize, kind: &str) -> bool {
        let _f = "enough_ports";
        let is_done = if self.neighbors.len() >= CONFIG.min_hello {
            no_seen_ports >= CONFIG.min_hello
        } else {
            no_seen_ports >= self.neighbors.len() && self.tree_count >= CONFIG.min_trees
        };
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.enough_ports {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_enough_ports" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "is_done": is_done,
                    "no_neighbors": self.neighbors.len(), "kind": kind,
                    "seen_ports": no_seen_ports, "seen_trees": self.tree_count });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        is_done
    }
    fn discover_ack_d_done(&mut self, tree_id: TreeID) -> bool {
        let _f = "discover_ack_d_done";
        // Can't move to enough_ports() due to borrow checker
        let no_seen_ports = self.discover_ack_seen_on_tree
            .entry(tree_id)
            .or_insert(Default::default())
            .len();
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.enough_ports {
                let default = HashSet::default();
                let seen_neighbors: Vec<PortNo> = self.discover_ack_seen_on_tree.get(&tree_id)
                    .or(Some(&default))
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_discover_ack_done" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "kind": "Prepared",
                    "neighbors_seen_on_tree": self.neighbor_names_from_port_nos(&seen_neighbors) });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.enough_ports(tree_id, no_seen_ports, "DiscoverDAck")
     }
    fn discoverd_done(&mut self, tree_id: TreeID, kind: &str) -> bool { // kind for debugging
        let _f = "discoverd_done";
        // Can't move to enough_ports() due to borrow checker
        let no_seen_ports = self.discoverd_seen_on_tree
            .entry(tree_id)
            .or_insert(Default::default())
            .len();
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.enough_ports {
                let default = HashSet::default();
                let seen_neighbors: Vec<PortNo> = self.discover_ack_seen_on_tree.get(&tree_id)
                    .or(Some(&default))
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_discover_ack_done" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "kind": kind,
                    "neighbors_seen_on_tree": self.neighbor_names_from_port_nos(&seen_neighbors) });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.enough_ports(tree_id, no_seen_ports, kind)
    }
    fn get_saved_discover(&self) -> &HashMap<TreeID, DiscoverMsg> { &self.saved_discover }
    fn get_saved_discoverd(&self) -> &HashMap<TreeID, DiscoverDMsg> { &self.saved_discoverd }
    fn get_saved_discover_ack_d(&self) -> &HashMap<TreeID, DiscoverAckDMsg> { &self.saved_discover_ack_d }
    fn add_saved_discover(&mut self, discover_msg: &DiscoverMsg) {
        let _f = "add_saved_discover";
        let port_tree_id = discover_msg.get_port_tree_id();
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.saved_msgs {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_save_discover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": port_tree_id, "msg": discover_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.saved_discover.insert(port_tree_id.to_tree_id(), discover_msg.clone());
    }
    fn add_saved_discoverd(&mut self, discoverd_msg: &DiscoverDMsg) {
        let _f = "add_saved_discoverd";
        let port_tree_id = discoverd_msg.get_port_tree_id();
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.saved_msgs {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_save_discoverd_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": port_tree_id, "msg": discoverd_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.saved_discoverd.insert(port_tree_id.to_tree_id(), discoverd_msg.clone());
    }
    fn add_saved_discover_ack_d(&mut self, discover_ack_d_msg: &DiscoverAckDMsg) {
        let _f = "add_saved_discover_ack_d";
        let port_tree_id = discover_ack_d_msg.get_port_tree_id();
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.saved_msgs {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_save_discoverd_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": port_tree_id, "msg": discover_ack_d_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.saved_discover_ack_d.insert(port_tree_id.to_tree_id(), discover_ack_d_msg.clone());
    }
    fn is_border_port(&self, port_number: &PortNumber) -> bool {
        self.border_port_tree_id_map.contains_key(port_number)
    }
    fn get_border_port(&self, test_originator_id: &OriginatorID) -> Result<PortNumber, Error> {
        let _f = "get_border_port";
        let entry = self.border_port_tree_id_map
            .iter()
            .find(|(_, originator_id)| test_originator_id == *originator_id)
            .ok_or(CellagentError::Sender { func_name: _f, cell_id: self.cell_id, originator_id: test_originator_id.clone() })?;
        Ok(*entry.0)
    }
    fn add_tree_name_map_item(&mut self, originator_id: OriginatorID, allowed_tree: &AllowedTree, allowed_tree_id: TreeID) {
        let _f = "add_tree_name_map_item";
        let mut locked = self.tree_name_map.lock().unwrap();
        let mut name_map = locked
            .get(&originator_id)
            .cloned()
            .unwrap_or_default();
        name_map.insert(allowed_tree_id, allowed_tree.clone());
        locked.insert(originator_id, name_map);
    }
    fn get_originator_ids(&self) -> Vec<OriginatorID> {
        let locked = self.tree_name_map.lock().unwrap();
        locked.keys().cloned().collect::<Vec<OriginatorID>>()
    }
    fn delete_tree_name_map_item(&mut self, delete_tree_id: &TreeID)
            -> Result<(), Error> {
        let mut locked1 = self.tree_name_map.lock().unwrap();
        let keys = self.get_originator_ids();
        keys
            .iter()
            .for_each(|originator_id|
                if let Some(tree_name_map) = locked1.get_mut(&originator_id) {
                    tree_name_map.remove_by_left(delete_tree_id);
                }
            );
        Ok(())
    }
    fn name_from_tree(&self, originator_id: OriginatorID, tree_id: TreeID) -> Result<AllowedTree, Error> {
        let _f = "name_from_tree";
        let locked = self.tree_name_map.lock().unwrap();
        let tree_name_map = locked
            .get(&originator_id)
            .ok_or::<Error>(CellagentError::TreeNameMap { cell_id: self.cell_id, func_name: _f, originator_id }.into())?;
        let new_tree_name = tree_name_map
            .get_by_left(&tree_id)
            .ok_or::<Error>(CellagentError::TreeMap { cell_id: self.cell_id, func_name: _f, tree_id, originator_id }.into())?;
        Ok(new_tree_name.clone())
    }
    fn tree_from_name(&self, originator_id: OriginatorID, tree_name: &AllowedTree) -> Result<TreeID, Error> {
        let _f = "tree_from_name";
        let locked = self.tree_name_map.lock().unwrap();
        let tree_name_map = locked
            .get(&originator_id)
            .ok_or::<Error>(CellagentError::TreeNameMap { cell_id: self.cell_id, func_name: _f, originator_id }.into())?;
        let tree_id = tree_name_map
            .get_by_right(tree_name)
            .ok_or::<Error>(CellagentError::NameMap { cell_id: self.cell_id, func_name: _f, tree_name: tree_name.clone(), originator_id }.into())?;
        Ok(tree_id.clone())
    }
    fn neighbor_names_from_port_nos(&self, port_nos: &[PortNo]) -> Vec<String> {
        port_nos.iter()
            .map(|port_no| self.neighbors.get(&port_no))
            .filter(|neighbor| neighbor.is_some() )
            .map(|neighbor| neighbor.unwrap().0.get_name())
            .collect::<Vec<_>>()
    }
    fn update_base_tree_map(&mut self, stacked_tree_id: PortTreeID, base_tree_id: TreeID) {
        let _f = "update_base_tree_map";
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.traph_entry {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_update_base_tree_map" };
                let trace = json!({ "cell_id": &self.cell_id, "stacked_tree_id": stacked_tree_id, "base_tree_id": base_tree_id, });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.base_tree_map.insert(stacked_tree_id, base_tree_id);
        self.tree_map.insert(stacked_tree_id.get_uuid(), base_tree_id.get_uuid());
    }
    fn get_tree(&self, port_tree_id: PortTreeID) -> Result<Option<Tree>, Error> {
        let _f = "get_tree";
        Ok(self.get_traph(port_tree_id)?
            .get_stacked_trees().lock().unwrap()
            .get(&port_tree_id.get_uuid())
            .cloned())
    }
    fn get_parent_tree_entry(&self, child_port_tree_id: PortTreeID)
            -> Result<Option<RoutingTableEntry>, Error> {
        let _f = "get_parent_tree_entry";
        Ok(self.get_tree(child_port_tree_id)?
            .map(|tree| tree.get_table_entry()))
    }
    fn get_parent_tree_id(&self, child_port_tree_id: PortTreeID) -> Result<Option<TreeID>, Error> {
        Ok(self.get_tree(child_port_tree_id)?
            .map(|tree| tree
                .get_parent_port_tree_id()
                .to_tree_id()))
    }
    fn get_base_tree_id(&self, port_tree_id: PortTreeID) -> Result<TreeID, Error> {
        let _f = "get_base_tree_id";
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.traph_entry {   // Debug print
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_get_base_tree_id" };
                let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": port_tree_id });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.base_tree_map
            .get(&port_tree_id)
            .cloned()
            .ok_or(CellagentError::BaseTree { func_name: _f, cell_id: self.cell_id, tree_id: port_tree_id }.into())
    }
    //pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
    // These functions specify the Discover quenching algorithms (never forward cell's own DiscoverMsgs)
    // Quench if this cell has already received a DiscoverMsg for this tree
    fn tree_seen(&self, tree_id: TreeID) -> bool {
        let _f = "tree_seen";
        tree_id != self.my_tree_id && self.traphs.contains_key(&tree_id.get_uuid())
    }
    // These functions specify the Discover quenching algorithms (never forward cell's own DiscoverMsgs)
    // Quench if this cell has already received a DiscoverMsg for this tree
    fn quench_simple(&self, tree_id: TreeID) -> bool {
        let _f = "quench_simple";
        self.tree_seen(tree_id)
    }
    // Quench if this cell has seen a DiscoverMsg for each root port of this tree
    fn quench_root_port(&self, port_tree_id: PortTreeID) -> bool {
        let _f = "quench_root_port";
        let tree_id = port_tree_id.to_tree_id();
        tree_id != self.my_tree_id && self.traphs
            .get(&tree_id.get_uuid())
            .map_or(false, |traph| {
                traph.get_port_trees().contains_key(&port_tree_id)
            })
    }
    // Quench if this cell has seen a DiscoverMsg for this tree on each of its ports
    fn quench_my_port(&self, tree_id: TreeID, port_no: PortNo) -> bool {
        let _f = "quench_my_port";
        tree_id != self.my_tree_id && self.traphs
            .get(&tree_id.get_uuid())
            .map_or(false, |traph| -> bool {
                traph.get_element(port_no).is_ok()
            })
    }
    fn update_traph(&mut self, base_port_tree_id: PortTreeID, port_number: PortNumber, port_state: PortState,
                    gvm_eqn: &GvmEquation, mut children: HashSet<PortNumber>,
                    hops: PathLength, path: Path)
                    -> Result<RoutingTableEntry, Error> {
        let _f = "update_traph";
        let base_tree_id = base_port_tree_id.to_tree_id();
        let neighbor = self.neighbors.get(&port_number.get_port_no());
        self.tree_id_map.insert(base_tree_id.get_uuid(), base_port_tree_id);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.traph_entry {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_update_traph" };
                let trace = json!({ "cell_id": &self.cell_id,
                "base_tree_id": base_port_tree_id, "neighbor": neighbor, "hops": &hops,
                "port_state": &port_state,
                "children": children, "gvm": &gvm_eqn });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let mut traph = self.traphs
            .remove(&base_tree_id.get_uuid()) // Remember to insert when done with updates
            .unwrap_or(Traph::new(self.cell_id, self.no_ports, base_tree_id, gvm_eqn)?);
        let (gvm_recv, gvm_send, _gvm_xtnd, _gvm_save) = {
            let variables = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
            let recv = gvm_eqn.eval_recv(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_recv") })?;
            let send = gvm_eqn.eval_send(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_send") })?;
            let xtnd = gvm_eqn.eval_xtnd(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_xtnd") })?;
            let save = gvm_eqn.eval_save(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_save") })?;
            (recv, send, xtnd, save)
        };
        let updated_hops = match port_state {
            PortState::Child => {
                let element = traph.get_parent_element().context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                // Need to coordinate the following with DiscoverMsg.update_discover_msg
                element.hops_plus_one()
            },
            PortState::Pruned | PortState::Broken => {
                let element = traph.get_parent_element().context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                element.get_hops()
            },
            // It's tempting to use min(hops, element.get_hops()), but I think it's better to
            // stick with the value associated with the first DiscoverMsg.  Most of the time
            // it will be the smallest, anyway.
            PortState::Parent | PortState::Unknown => hops,
        };
        let traph_state = traph.get_port_status(port_number);
        let entry_port_state = match traph_state {
            PortState::Pruned | PortState::Unknown => port_state,
            _ => traph_state  // Don't replace if Parent or Child
        };
        match entry_port_state {
            PortState::Parent  => traph.mark_parent(port_number),
            PortState::Child   => traph.mark_child(port_number),
            PortState::Pruned  => traph.mark_pruned(port_number),
            PortState::Unknown => traph.mark_unknown(port_number),
            PortState::Broken  => traph.mark_broken(port_number)
        }
        if gvm_recv { children.insert(PortNumber::new0()); }
        let mut entry = traph.update_element(base_tree_id, port_number,
                                             entry_port_state, &children, updated_hops, path).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
        if gvm_send { entry.enable_send() } else { entry.disable_send() }
        {
            let traph_state = traph.get_port_status(port_number);
            if CONFIG.debug_options.all || CONFIG.debug_options.traph_entry {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_updated_traph_entry" };
                let trace = json!({ "cell_id": &self.cell_id, "base_tree_id": base_tree_id,
                    "neighbor": neighbor, "traph_state": traph_state, "hops": hops, "updated hops": updated_hops, "entry": &entry });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        // Need traph even if cell only forwards on this tree
        self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S("base_tree_id") })?;
        let mut port_tree = traph
            .own_port_tree(base_port_tree_id)
            .unwrap_or(PortTree::new(base_port_tree_id, port_number.get_port_no(), hops));
        if base_tree_id != self.my_tree_id {  // Not my tree
            // The first port_tree entry is the one that denotes this branch
            let first_port_tree_id = traph.add_port_tree(&port_tree);
            let mut first_port_tree = traph.own_port_tree(first_port_tree_id).unwrap(); // Unwrap safe by previous line
            let mut new_entry = entry.clone(); // Clone so entry won't change when new_entry does
            new_entry.set_tree_id(first_port_tree.get_port_tree_id());
            first_port_tree.set_entry(new_entry);
            traph.add_port_tree(&first_port_tree);
            self.update_entry(&new_entry).context(CellagentError::Chain { func_name: _f, comment: S("first_port_tree_id") })?;
            if traph.get_port_trees().len() == 1 {
                self.tree_id_map.insert(first_port_tree_id.get_uuid(), first_port_tree_id);
            };
            // TODO: Need to update stacked tree ids to get port tree ids
            let locked = traph.get_stacked_trees().lock().unwrap();
            for stacked_tree in locked.values() {
                let mut entry = stacked_tree.get_table_entry();
                let stacked_port_tree_id = stacked_tree.get_port_tree_id();
                if stacked_port_tree_id == first_port_tree_id {
                    entry.set_tree_id(stacked_port_tree_id);
                    self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S("stacked_port_tree_id") })?;
                }
            }
        } else { // This is my tree, so each port_tree has one child
            let mask = Mask::new(port_number);
            let new_entry = RoutingTableEntry::new(port_tree.get_port_tree_id(), true,
                           PortNumber::new0(), mask, gvm_send).add_child(PortNumber::new0());
            port_tree.set_entry(new_entry);
            traph.add_port_tree(&port_tree);
            self.update_entry(&new_entry)?;
        }
        self.traphs.insert(base_tree_id.get_uuid(), traph);
        // Used so I can print traphs from main
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        // TODO: Need to update entries of stacked trees following a failover but not as base tree builds out
        //let entries = traph.update_stacked_entries(entry).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        //for entry in entries {
        //println!("Cell {}: sending entry {}", self.cell_id, entry);
        //self.ca_to_cm.send(CaToPePacket::Entry(entry)).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        //}
        Ok(entry)
    }
    fn get_traph_mut(&mut self, port_tree_id: PortTreeID) -> Result<&mut Traph, Error> {
        let _f = "get_traph_mut";
        let base_tree_id = self.get_base_tree_id(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let uuid = base_tree_id.get_uuid();
        self.traphs
            .get_mut(&uuid)
            .ok_or(CellagentError::NoTraph { cell_id: self.cell_id, func_name: "stack_tree", tree_id: base_tree_id }.into())
    }
    fn get_traph(&self, port_tree_id: PortTreeID) -> Result<&Traph, Error> {
        let _f = "get_traph";
        let base_tree_id = self.get_base_tree_id(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let uuid = base_tree_id.get_uuid();
        self.traphs
            .get(&uuid)
            .ok_or(CellagentError::NoTraph { cell_id: self.cell_id, func_name: "stack_tree", tree_id: base_tree_id }.into())
    }
    fn deploy(&mut self, originator_id: OriginatorID, deployment_port_tree_id: PortTreeID, _msg_tree_id: PortTreeID,
              _msg_tree_map: &MsgTreeMap, manifest: &Manifest) -> Result<(), Error> {
        let _f = "deploy";
        let tree_name_map = self.tree_name_map.lock().unwrap()
            .get(&originator_id)
            .cloned()
            .ok_or::<Error>(CellagentError::TreeNameMap { func_name: _f, cell_id: self.cell_id, originator_id }.into())?;
        for vm_spec in manifest.get_vms() {
            let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
            let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
            let container_specs = vm_spec.get_containers();
            let vm_id = VmID::new(self.cell_id, &vm_spec.get_id())?;
            let vm_allowed_trees = vm_spec.get_allowed_trees();
            let vm_originator_id = OriginatorID::new(self.cell_id, &vm_id.get_name())?;
            let up_tree_name = vm_spec.get_id();
            let mut allowed_trees = HashSet::new();
            allowed_trees.insert(AllowedTree::new(CONTROL_TREE_NAME));
            let mut vm = VirtualMachine::new(&vm_id, vm_to_ca, vm_allowed_trees);
            for vm_allowed_tree in vm_allowed_trees {
                tree_name_map
                    .get_by_right(vm_allowed_tree)
                    .ok_or::<Error>(CellagentError::NameMap { cell_id: self.cell_id, func_name: "deploy(vm)", originator_id, tree_name: vm_allowed_tree.clone() }.into())
                    .map(|allowed_tree_id| {
                        allowed_trees.insert(vm_allowed_tree.clone());
                        self.add_tree_name_map_item(originator_id, vm_allowed_tree, allowed_tree_id.clone());
                        self.add_tree_name_map_item(vm_originator_id, vm_allowed_tree, allowed_tree_id.clone());
                        // Functional style runs into a borrow problem
                        match self.tree_vm_map.get_mut(allowed_tree_id) {
                            Some(senders) => senders.push(ca_to_vm.clone()),
                            None => { self.tree_vm_map.insert(allowed_tree_id.clone(), vec![ca_to_vm.clone()]); }
                        }
                    })?;
            }
            vm.initialize(up_tree_name, vm_from_ca, &allowed_trees, container_specs)?;
            {
                if CONFIG.debug_options.all || CONFIG.debug_options.deploy {
                    let keys = self.tree_vm_map.keys().collect::<Vec<_>>();
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_deploy" };
                    let trace = json!({ "cell_id": &self.cell_id, "vm_id": vm_id,
                        "deployment_port_tree_id": deployment_port_tree_id, "tree_vm_map_keys":  &keys,
                        "up_tree_name": up_tree_name });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                }
            }
            self.ca_to_vms.insert(vm_id, ca_to_vm, );
            self.listen_uptree(vm_originator_id, vm_id, allowed_trees, ca_from_vm);
        }
        Ok(())
    }
    // SPAWN THREAD (listen_uptree_loop)
    fn listen_uptree(&self, originator_id: OriginatorID, vm_id: VmID, trees: HashSet<AllowedTree>,
                     ca_from_vm: CaFromVm) {
        let _f = "listen_uptree";
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {} listen_uptree_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn(move || {
            update_trace_header(child_trace_header);
            let _ = ca.listen_uptree_loop(originator_id, vm_id, &ca_from_vm).map_err(|e| write_err("cellagent", &e));
            if CONFIG.continue_on_error { ca.listen_uptree(originator_id, vm_id, trees, ca_from_vm); }
        }).expect("thread failed");
    }

    // WORKER (CaFromVm)
    fn listen_uptree_loop(&mut self, originator_id: OriginatorID, _vm_id: VmID, ca_from_vm: &CaFromVm)
                          -> Result<(), Error> {
        let _f = "listen_uptree_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let bytes = ca_from_vm.recv().context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            let serialized = bytes.to_string()?;
            let app_msg: Box<dyn AppMessage> = serde_json::from_str(&serialized).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_vm_app" };
                    let trace = json!({ "cell_id": &self.cell_id, "app_msg": app_msg });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.ca_to_cm[0].send(CaToCmBytes::TunnelUp((originator_id, bytes)))?;
        }
    }
    /*
        fn create_tree(&mut self, id: &str, target_tree_id: TreeID, port_no_mask: Mask, gvm_eqn: &GvmEquation)
                -> Result<(), Error> {
            let new_id = self.my_tree_id.add_component(id)?;
            let new_tree_id = TreeID::new(new_id.get_name())?;
            new_tree_id.add_to_trace()?;
            let ref my_tree_id = self.my_tree_id; // Need because self is borrowed mut
            let msg =  StackTreeMsg::new(&new_tree_id, &self.my_tree_id,&gvm_eqn);
            self.send_msg(line!(), target_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.get_id())})?;
            self.stack_tree(&new_tree_id, &my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.get_id())})?;
            Ok(())
        }
    */
    fn get_tree_entry(&self, port_tree_id: PortTreeID)
            -> Result<RoutingTableEntry, Error> {
        let _f = "get_tree_entry";
        let traph = self.get_traph(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let entry = traph.get_tree_entry(&port_tree_id.get_uuid()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        Ok(entry)
    }
    fn stack_tree(&mut self, originator_id: OriginatorID, allowed_tree: &AllowedTree,
                  new_port_tree_id: PortTreeID, parent_port_tree_id: PortTreeID,
                  new_port_tree_id_opt: Option<PortTreeID>,
                  gvm_eqn: &GvmEquation) -> Result<RoutingTableEntry, Error> {
        let _f = "stack_tree";
        let no_ports = self.no_ports;
        let base_tree_id = self.get_base_tree_id(parent_port_tree_id)
            .unwrap_or_else(|_| {
                let tree_id = parent_port_tree_id.to_tree_id();
                self.update_base_tree_map(new_port_tree_id, tree_id);
                tree_id
            });
        self.add_tree_name_map_item(originator_id, allowed_tree, new_port_tree_id.to_tree_id());
        self.update_base_tree_map(new_port_tree_id, base_tree_id);
        let traph = self.get_traph_mut(parent_port_tree_id).context(CellagentError::Chain { func_name: "stack_tree", comment: S("own_traph") })?;
        let parent_entry = traph.get_tree_entry(&parent_port_tree_id.get_uuid()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("get_tree_entry") })?;
        let mut entry = parent_entry; // Copy so parent_entry won't change when entry does
        entry.set_uuid(&new_port_tree_id.get_uuid());
        let params = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("get_params") })?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&params).context(CellagentError::Chain { func_name: _f, comment: S("gvm_xtnd") })?;
        let gvm_send = gvm_eqn.eval_send(&params).context(CellagentError::Chain { func_name: _f, comment: S("gvm_send") })?;
        if !gvm_xtnd { entry.clear_children(); }
        if gvm_send { entry.enable_send(); } else { entry.disable_send(); }
        let gvm_recv = gvm_eqn.eval_recv(&params).context(CellagentError::Chain { func_name: _f, comment: S("eval_recv") })?;
        if gvm_recv {
            entry.enable_receive();
        } else {
            entry.disable_receive(no_ports);
        }
        let tree = Tree::new(new_port_tree_id, base_tree_id, parent_port_tree_id, &gvm_eqn, entry);
        traph.stack_tree(tree);
        self.tree_map.insert(new_port_tree_id.get_uuid(), base_tree_id.get_uuid());
        self.tree_id_map.insert(new_port_tree_id.get_uuid(), new_port_tree_id);
        // TODO: Make sure that stacked tree entries for port trees get created
        self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        // Next line avoids a mutability error; requires NLL
        let traph = self.get_traph_mut(parent_port_tree_id).context(CellagentError::Chain { func_name: "stack_tree", comment: S("own_traph") })?;
        // No new_port_tree for uptrees, denoted by new_port_tree_id = None
        if new_port_tree_id_opt.is_some() {
            entry.set_tree_id(new_port_tree_id);
            let tree = Tree::new(new_port_tree_id, base_tree_id, parent_port_tree_id, &gvm_eqn, entry);
            traph.stack_tree(tree);
            self.update_entry(&entry)?;
        }
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.stack_tree { // Debug print
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_stack_tree" };
                let trace = json!({ "cell_id": &self.cell_id,
                "new_port_tree_id": &new_port_tree_id, "base_tree_id": &base_tree_id,
                "params": params, "gvm_send": gvm_send, "gvm_recv": gvm_recv, "gvm_xtnd": gvm_xtnd });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(parent_entry)
    }
    fn update_entries(&self, entries: &[RoutingTableEntry]) -> Result<(), Error> {
        let _f = "update_entries";
        for entry in entries { self.update_entry(entry)?; }
        Ok(())
    }
    fn update_entry(&self, entry: &RoutingTableEntry) -> Result<(), Error> {
        let _f = "update_entry";
        if CONFIG.trace_options.all || CONFIG.trace_options.ca {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_cm_entry" };
            //println!("Cellagent {}: {} msg {}", self.cell_id, _f, msg); // Should be msg.value()
            let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        self.ca_to_cm[0].send(CaToCmBytes::Entry(*entry)).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        Ok(())
    }

    // SPAWN THREAD (listen_port_loop)
    fn listen_port(&mut self, ca_from_ports: CaFromPort) -> JoinHandle<()> {
        let _f = "listen_port";
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {} listen_port_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn(move || {
            update_trace_header(child_trace_header);
            let _ = ca.listen_border_port_loop(&ca_from_ports).map_err(|e| write_err("cellagent", &e));
            if CONFIG.continue_on_error { let _ = ca.listen_port(ca_from_ports); }
        }).expect("cellagent port thread failed")
    }
    fn listen_border_port_loop(&mut self, ca_from_port: &CaFromPort) -> Result<(), Error> {
        let _f = "listen_border_port_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = ca_from_port.recv().context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                    match &msg {
                        PortToCaMsg::AppMsg(port_no, bytes) => {
                            let ec_msg: Box<dyn AppMessage> = serde_json::from_str(&bytes.to_string()?)?;
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_port_bytes" };
                            let trace = json!({ "cell_id": self.cell_id, "port": port_no, "ec_msg": ec_msg });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        PortToCaMsg::Status(port_no, status) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_port_status" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "status": status });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                    }
                }
            }
            match msg {
                PortToCaMsg::AppMsg(port_no, bytes) => {
                    self.ca_to_cm[0].send(CaToCmBytes::TunnelPort((port_no, bytes)))?;
                }
                PortToCaMsg::Status(port_no, port_status) => {
                    let is_border = true;
                    self.ca_to_cm[0].send(CaToCmBytes::Status((port_no, is_border, NumberOfPackets::new(), port_status)))?;
                }
            }
        }
    }
    // SPAWN THREAD (listen_cm_loop)
    fn listen_cm(&mut self, ca_from_cm: CaFromCm) -> JoinHandle<()> {
        let _f = "listen_cm";
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {} listen_cm_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn(move || {
            update_trace_header(child_trace_header);
            let _ = ca.listen_cm_loop(&ca_from_cm).map_err(|e| write_err("cellagent", &e));
            if CONFIG.continue_on_error { let _ = ca.listen_cm(ca_from_cm); }
        }).expect("cellagent cmodel thread failed")
    }

    // WORKER (CaFromCm)
    fn listen_cm_loop(&mut self, ca_from_cm: &CaFromCm) -> Result<(), Error> {
        let _f = "listen_cm_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = ca_from_cm.recv().context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                    match &msg {
                        CmToCaBytes::Bytes((port_no, _, _, bytes)) => {
                            let ec_msg: Box<dyn Message> = serde_json::from_str(&bytes.to_string()?)?;
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_cm_bytes" };
                            let trace = json!({ "cell_id": self.cell_id, "port": port_no, "msg": ec_msg });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToCaBytes::Status((port_no, is_border, number_of_packets, status)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_cm_status" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToCaBytes::TunnelPort((port_no, bytes)) => {
                            let app_msg: Box<dyn AppMessage> = serde_json::from_str(&bytes.to_string()?)?;
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_cm_bytes_port" };
                            let trace = json!({ "cell_id": self.cell_id, "port": port_no, "msg": app_msg });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToCaBytes::TunnelUp((originator_id, bytes)) => {
                            let app_msg: Box<dyn AppMessage> = serde_json::from_str(&bytes.to_string()?)?;
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_cm_bytes_up" };
                            let trace = json!({ "cell_id": self.cell_id, "originator_id": originator_id, "app_msg": app_msg });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                }
            }
            match msg {
                CmToCaBytes::Status((port_no, is_border, number_of_packets, status)) => match status {
                    PortStatus::Connected => self.port_connected(port_no, is_border).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " port_connected" })?,
                    PortStatus::Disconnected => self.port_disconnected(port_no, number_of_packets).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " port_disconnected" })?
                },
                CmToCaBytes::Bytes((port_no, is_ait, uuid, bytes)) => {
                    // The index may be pointing to the control tree because the other cell didn't get the StackTree or StackTreeD message in time
                    let mut msg = MsgType::msg_from_bytes(&bytes).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
                    {
                        if CONFIG.debug_options.all || CONFIG.debug_options.ca_msg_recv {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_msg" };
                            let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": &msg });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                        }
                    }
                    let msg_tree_id = {  // Use control tree if uuid not found
                        self.tree_id_map
                            .get(&uuid)
                            .unwrap_or(&self.control_tree_id.to_port_tree_id_0())
                            .clone()
                    };
                    msg.process_ca(self, port_no, msg_tree_id, is_ait).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
                },
                CmToCaBytes::TunnelPort((port_no, bytes)) => {
                    let port_number = port_no.make_port_number(self.no_ports).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " PortNumber" })?;
                    let originator_id = self.border_port_tree_id_map
                        .get(&port_number)
                        .cloned()
                        .ok_or::<Error>(CellagentError::Border { func_name: _f, cell_id: self.cell_id, port_no: *port_no }.into())?;
                    // Verify that this sender can name this tree
                    if !self.tree_name_map.lock().unwrap().contains_key(&originator_id) {
                        return Err(CellagentError::TreeNameMap { func_name: _f, cell_id: self.cell_id, originator_id }.into());
                    }
                    let serialized = bytes.to_string()?;
                    let app_msg: Box<dyn AppMessage> = serde_json::from_str(&serialized).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                    app_msg.process_ca(self, originator_id)?;
                }
                CmToCaBytes::TunnelUp((originator_id, bytes)) => {
                    if !self.tree_name_map.lock().unwrap().contains_key(&originator_id) {
                        return Err(CellagentError::TreeNameMap { func_name: _f, cell_id: self.cell_id, originator_id }.into());
                    }
                    let serialized = bytes.to_string()?;
                    let app_msg: Box<dyn AppMessage> = serde_json::from_str(&serialized).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                    app_msg.process_ca(self, originator_id)?;
                }
            }
        }
    }
    fn update_sender_tree_map(&mut self, originator_id: OriginatorID, allowed_trees: &Vec<AllowedTree>, tree_id: TreeID) {
        for allowed_tree in allowed_trees {
            self.add_tree_name_map_item(originator_id, allowed_tree, tree_id);
        }
    }
    fn delete_tree(&mut self, delete_tree_id: &TreeID) -> Result<(), Error> {
        let _f = "delete_tree";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_cm_delete_tree" };
                let trace = json!({ "cell_id": &self.cell_id, "delete_tree": delete_tree_id });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let uuid = delete_tree_id.get_uuid();
        self.ca_to_cm[0].send(CaToCmBytes::Delete(uuid)).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        println!("Cellagent {}: {} deleting tree {}", self.cell_id, _f, delete_tree_id);
        let traph = self.get_traph_mut(delete_tree_id.to_port_tree_id_0())?;
        traph.delete_tree(delete_tree_id);
        // The following is needed to protect against reused tree names
        self.delete_tree_name_map_item(delete_tree_id)?;
        Ok(())
    }
    pub fn process_interapplication_msg(&mut self, msg: &InterapplicationMsg, port_no: PortNo)
            -> Result<(), Error> {
        let _f = "process_interapplication_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_interapplication_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": S(msg) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_tree_id = msg.get_port_tree_id();
        let app_msg = msg.get_payload().get_app_msg();
        let allowed_trees = app_msg.get_allowed_trees();
        if allowed_trees.len() > 0 {
            let originator_id = msg.get_header().get_originator_id();
            self.update_sender_tree_map(originator_id, &allowed_trees, port_tree_id.to_tree_id());
        }
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.process_msg {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_interapplication_msg_dbg" };
                let trace = json!({ "cell_id": &self.cell_id,"port_tree_id": port_tree_id, "port_no": port_no, "msg": S(msg) });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_vm_app" };
                let trace = json!({ "cell_id": &self.cell_id, "app_msg": S(app_msg) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let senders = self.get_vm_senders(port_tree_id.to_tree_id()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let serialized = serde_json::to_string(app_msg as &dyn AppMessage)?;
        let bytes = ByteArray::new(&serialized);
        for sender in senders {
            sender.send(bytes.clone()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        }
        Ok(())
    }
    pub fn process_delete_tree_msg(&mut self, delete_tree_id: &TreeID)
            -> Result<(), Error> {
        let _f = "process_delete_tree_msg";
        if *delete_tree_id != self.my_tree_id { // Can't delete a black tree from an app message
            self.delete_tree(delete_tree_id)?;
        } else {
            return Err(CellagentError::MayNotDelete { func_name: _f, cell_id: self.cell_id, tree_id: delete_tree_id.clone() }.into());
        }
        Ok(())
    }
    pub fn process_discover_msg(&mut self, msg: &DiscoverMsg, port_no: PortNo)
                                -> Result<(), Error> {
        let _f = "process_discover_msg";
        let payload = msg.get_payload();
        let port_number = port_no.make_port_number(self.no_ports)?;
        let hops = payload.get_hops();
        let path = payload.get_path();
        let new_port_tree_id = payload.get_port_tree_id();
        let new_tree_id = new_port_tree_id.to_tree_id();
        self.tree_id_map.insert(new_port_tree_id.get_uuid(), new_port_tree_id);
        let tree_seen = self.tree_seen(new_tree_id);
        let port_tree_seen = self.quench_root_port(new_port_tree_id);
        let my_port_seen = self.quench_my_port(new_tree_id, port_no);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.discover {
                let neighbors: Vec<_> = self.neighbors.keys().collect();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value(),
                    "port_tree_id": new_port_tree_id, "seen_trees": tree_seen, "neighbors": neighbors });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let quench = match CONFIG.quench {
            Quench::Simple   => tree_seen,      // Must see this tree once
            Quench::RootPort => port_tree_seen, // Must see every root port for this tree once
            Quench::MyPort   => my_port_seen,   // Must see every tree on every connected port
        };
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.discoverd {
                let seen_ports = self.discoverd_seen_on_tree.get(&new_tree_id);
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discoverd_msg_dbg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() ,
                            "port_tree_id": new_port_tree_id,
                            "seen ports": seen_ports,
                            "seen trees": self.traphs.len(), "discoverd parent msg": self.discoverd_parent_msg.get(&new_tree_id),
                            "neighbor count": self.neighbors.len()});
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.update_base_tree_map(new_port_tree_id, new_tree_id);
        // The following is needed until I get port trees and trees straightened out.
        self.update_base_tree_map(new_tree_id.to_port_tree_id_0(), new_tree_id);
        let originator_id = OriginatorID::new(self.cell_id, "CellAgent")?;
        let in_reply_to = msg.get_sender_msg_seq_no();
        if new_tree_id != self.my_tree_id && !tree_seen {
            self.tree_count = self.tree_count + 1;
            let discoverd_parent_msg = DiscoverDMsg::new(in_reply_to, self.cell_id,
                                                         originator_id, new_port_tree_id, path,
                                                         DiscoverDType::Parent);
            self.discoverd_parent_msg.insert(new_tree_id, (port_number, discoverd_parent_msg));
            let discoverd_msg = DiscoverDMsg::new(in_reply_to, self.cell_id,
                                                  originator_id, new_port_tree_id, path,
                                                  DiscoverDType::NonParent);
            self.add_saved_discoverd(&discoverd_msg);
            self.send_msg(line!(), self.connected_tree_id, discoverd_msg,
                          DEFAULT_USER_MASK.all_but_port(port_number))?;
            self.update_traph(new_port_tree_id, port_number, PortState::Parent,
                              &gvm_equation, HashSet::new(), hops, path).context(CellagentError::Chain { func_name: _f, comment: S("DiscoverMsg") })?;
            let msgs = self.discoverd_parent_msg.clone();
            for &tree_id in msgs.keys() {
                if self.discoverd_done(tree_id, "Discover") {
                    let (port_number, discoverd_msg) = self.discoverd_parent_msg.remove(&tree_id).unwrap();
                    self.send_msg(line!(), self.connected_tree_id, discoverd_msg.clone(), Mask::new(port_number))?;
                }
            }
        }
        if CONFIG.breadth_first {
            let discover_ack_msg = DiscoverAckMsg::new(in_reply_to, self.cell_id,
                                                       originator_id, new_port_tree_id);
            self.send_msg(line!(), self.connected_tree_id, discover_ack_msg, Mask::new(port_number))?;
        }
        if !quench {
            let updated_msg = msg.update(self.cell_id);
            self.add_saved_discover(&updated_msg); // Discover message are always saved for late port connect
            if !CONFIG.breadth_first {
                let user_mask = DEFAULT_USER_MASK.all_but_port(port_number);
                self.send_msg(line!(), self.connected_tree_id, updated_msg, user_mask)?;
                self.discover_sent.insert(new_tree_id);
            }
        }
        Ok(())
    }
    pub fn process_discover_d_msg(&mut self, msg: &DiscoverDMsg, port_no: PortNo)
                                  -> Result<(), Error> {
        let _f = "process_discoverd_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discoverd_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_number = PortNumber::new(port_no, self.no_ports)?;
        let port_tree_id = msg.get_port_tree_id();
        let tree_id = port_tree_id.to_tree_id();
        self.discoverd_seen_on_tree
            .entry(tree_id)
            .or_insert(Default::default())
            .insert(port_no);
        if self.discoverd_done(tree_id, &S(msg.get_discoverd_type())) {
            if tree_id != self.my_tree_id {
                if let Some((port_number, discoverd_msg)) = self.discoverd_parent_msg.remove(&tree_id) {
                    self.send_msg(line!(), self.connected_tree_id, discoverd_msg, Mask::new(port_number))?;
                }
            }
        }
        match msg.get_discoverd_type() {
            DiscoverDType::NonParent => {
                // Traph may not have been created yet for this port tree since I send DiscoverD early
                if let Ok(traph) = self.get_traph_mut(port_tree_id) {
                    let element = traph.get_element_mut(port_no)?;
                    element.set_connected();
                    if element.is_state(PortState::Unknown) {
                        element.mark_pruned();
                    }
                }
            },
            DiscoverDType::Parent => {
                let path = msg.get_path();
                let port_number = port_no.make_port_number(self.no_ports)?;
                let children = [port_number]
                    .iter()
                    .cloned()
                    .collect::<HashSet<_>>();
                let port_state = PortState::Child;
                let mut eqns = HashSet::new();
                eqns.insert(GvmEqn::Recv("true"));
                eqns.insert(GvmEqn::Send("true"));
                eqns.insert(GvmEqn::Xtnd("false"));
                eqns.insert(GvmEqn::Save("false"));
                let gvm_eqn = GvmEquation::new(&eqns, &Vec::new());
                // Setting hops to 0 is a hack so I can use update_traph()
                self.update_traph(port_tree_id, port_number, port_state, &gvm_eqn,
                                          children, PathLength(CellQty(0)), path)?;
                let traph = self.traphs.get(&tree_id.get_uuid()).expect("Traph was just set");
                let parents_seen_on_tree = {
                    let count = self.parents_seen_on_tree
                        .entry(tree_id)
                        .or_insert(Default::default());
                    *count += 1;
                    *count
                };
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.discoverd {
                        let seen_ports = self.discoverd_seen_on_tree.get(&tree_id);
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discoverd_msg_noc" };
                        let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value(),
                            "tree_id": tree_id, "border port connected": self.is_border_port_connected,
                            "sent to NOC": self.sent_to_noc, "is border": self.is_border(),
                            "parents seen on tree": parents_seen_on_tree, "neighbor count": self.neighbors.len()});
                        let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                    }
                }
                if tree_id == self.my_tree_id &&
                    self.is_border_port_connected &&
                    !self.sent_to_noc &&
                    self.is_border() &&
                    self.discoverd_done(tree_id, "DiscoverD::Parent") {
                    self.send_base_tree_to_noc()?;
                }
            }
        }
        Ok(())
    }
    pub fn process_failover_msg(&mut self, msg: &FailoverMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_failover_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_failover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let header = msg.get_header();
        let payload = msg.get_payload();
        let originator_id = header.get_originator_id();
        let rw_port_tree_id = payload.get_rw_port_tree_id();
        let rw_tree_id = rw_port_tree_id.to_tree_id();
        let port_number = port_no.make_port_number(self.no_ports)?;
        if rw_tree_id == self.my_tree_id {
            println!("Cellagent {}: {} found path to rootward for port tree {}", self.cell_id, _f, rw_port_tree_id);
            let rw_traph = self.get_traph(rw_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("lw_traph") })?;
            let broken_port_number = {
                let broken_element = rw_traph.get_parent_element().context(CellagentError::Chain { func_name: _f, comment: S("lw element") })?;
                broken_element.get_port_no().make_port_number(self.no_ports)?
            };
            let my_traph = self.get_traph_mut(self.my_tree_id.to_port_tree_id_0()).context(CellagentError::Chain { func_name: _f, comment: S("my_traph") })?;
            my_traph.mark_broken(broken_port_number);
            let changed_entries = my_traph.change_child(rw_port_tree_id, broken_port_number, port_number)?;
            self.update_entries(&changed_entries)?;
            let mask = Mask::new(port_number);
            let in_reply_to = msg.get_sender_msg_seq_no();
            let no_packets = self.no_packets[broken_port_number.as_usize()];
            let failover_d_msg = FailoverDMsg::new(in_reply_to, self.cell_id,
                                                   originator_id, FailoverResponse::Success,
                                                   no_packets, payload);
            self.send_msg(line!(), self.connected_tree_id, failover_d_msg, mask)?;
        } else {
            self.failover_reply_ports.insert(rw_port_tree_id, port_no);
            self.find_new_parent(header, payload, port_no).context(CellagentError::Chain { func_name: _f, comment: S("find_new_parent") })?;
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    pub fn process_failover_d_msg(&mut self, msg: &FailoverDMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_failover_d_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca { // Needed for visualization
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_failover_d_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let failover_reply_ports = &self.failover_reply_ports.clone();
        let header = msg.get_header();
        let payload = msg.get_payload();
        let failover_payload = payload.get_failover_payload();
        let rw_port_tree_id = failover_payload.get_rw_port_tree_id();
        let lw_port_tree_id = failover_payload.get_lw_port_tree_id();
        let broken_port_tree_ids = failover_payload.get_broken_port_tree_ids();
        self.tree_id_map.insert(rw_port_tree_id.get_uuid(), rw_port_tree_id);
        if self.my_tree_id == lw_port_tree_id.to_tree_id() {
            println!("Cellagent {}: {} reached leafward node for port tree {}", self.cell_id, _f, rw_port_tree_id);
            match payload.get_response() {
                FailoverResponse::Failure => {
                    let lw_tree_id = lw_port_tree_id.to_tree_id();
                    let rw_tree_id = rw_port_tree_id.to_tree_id();
                    return Err(CellagentError::Partition { func_name: _f, lw_tree_id, rw_tree_id }.into())
                },
                FailoverResponse::Success => {
                    let broken_port_no = {
                        let rw_traph = self.get_traph(rw_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("lw_traph") })?;
                        let broken_element = rw_traph.get_parent_element().context(CellagentError::Chain { func_name: _f, comment: S("lw element") })?;
                        broken_element.get_port_no()
                    };
                    let no_packets = payload.get_number_of_packets();
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_cm_reroute" };
                            let trace = json!({ "cell_id": &self.cell_id, "broken_port_no": broken_port_no, "port_no": port_no, "no_packets": no_packets });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.ca_to_cm[0].send(CaToCmBytes::Reroute((broken_port_no, port_no, no_packets)))?;
                    // Following line is commented out because the packet engine does rerouting.
                    // Packets still go the out queue for the broken link, but the packet engine reroutes them
                    // to the failover port.  The traph will need to be repaired if another strategy is used.
                    //self.repair_traph(broken_port_tree_ids, port_number)?; // Must be done after Reroute
                }
            }
        } else {
            match payload.get_response() {
                FailoverResponse::Success => {
                    failover_reply_ports
                        .get(&rw_port_tree_id)
                        .map(|failover_port_no| -> Result<(), Error> {
                            let failover_port_number = failover_port_no.make_port_number(self.no_ports)?;
                            self.repair_traph(broken_port_tree_ids, failover_port_number)?;
                            let mask = Mask::new(failover_port_no.make_port_number(self.no_ports)?);
                            let in_reply_to = msg.get_sender_msg_seq_no();
                            let originator_id = header.get_originator_id();
                            let broken_port_number = payload
                                .get_failover_payload()
                                .get_broken_path()
                                .get_port_number()
                                .as_usize();
                            let failover_d_msg = FailoverDMsg::new(in_reply_to, self.cell_id,
                                                                   originator_id, FailoverResponse::Success,
                                                                   self.no_packets[broken_port_number],
                                                                   payload.get_failover_payload());
                            self.send_msg(line!(), self.connected_tree_id, failover_d_msg, mask)
                        })
                        .ok_or(CellagentError::FailoverPort { func_name: _f, cell_id: self.cell_id, port_tree_id: rw_port_tree_id })??;
                    self.failover_reply_ports.remove(&rw_port_tree_id);
                },
                FailoverResponse::Failure => {
                    self.find_new_parent(&msg.get_header(), msg.get_payload().get_failover_payload(), port_no)?;
                }
            }
        }
        // TODO: Flood new hops update for healed trees
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    pub fn process_hello_msg(&mut self, msg: &HelloMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_hello_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca { // Needed for visualization
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "#neighbors": self.neighbors.len(), "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_number = PortNumber::new(port_no, self.no_ports)?;
        let my_port_tree_id = self.my_tree_id.to_port_tree_id(port_number);
        let user_mask = Mask::new(port_number);
        let originator_id = OriginatorID::new(self.cell_id, "CellAgent")?;
        let path = Path::new(port_number);
        let in_reply_to = msg.get_sender_msg_seq_no();
        let payload = msg.get_payload();
        let neighbor_cell_id = msg.get_cell_id();
        let neigbor_port_no = msg.get_port_no();
        self.neighbors.insert(port_no, (neighbor_cell_id, neigbor_port_no));
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.hello {
                let neighbors: Vec<_> = self.neighbors.keys().collect();
                let neighbor = self.neighbor_names_from_port_nos(&[port_no]);
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg_dbg" };
                let trace = json!({ "cell_id": &self.cell_id, "recv_neighbor": neighbor,
                    "neighbors": neighbors, "neighbor_count": self.neighbors.len(),
                    "discover_sent": self.discover_sent,
                    "discoverd_sent": self.saved_discoverd.len(),
                    "discover_ack_d_sent": self.discover_ack_d_sent,
                    "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        // Send my DiscoverMsg and DiscoverDMsg
        let discover_msg = DiscoverMsg::new(self.cell_id, originator_id,
                                            my_port_tree_id, PathLength(CellQty(1)),
                                            Path::new(port_number));
        self.send_msg(line!(), self.connected_tree_id, discover_msg, user_mask)?;
        let discoverd_msg = DiscoverDMsg::new(in_reply_to, self.cell_id,
                         originator_id, my_port_tree_id, path, DiscoverDType::NonParent);
        self.send_msg(line!(), self.connected_tree_id, discoverd_msg, user_mask)?;
        for (tree_id, discoverd_msg) in &self.saved_discoverd {
            {
                if CONFIG.debug_options.all || CONFIG.debug_options.hello {
                    let neighbors: Vec<_> = self.neighbors.keys().collect();
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg_discoverd" };
                    let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id,
                    "discoverd_msg": discoverd_msg.value() });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                }
            }
            self.send_msg(line!(), self.connected_tree_id, discoverd_msg.clone(), user_mask)?;
        }
        for tree_id in &self.discover_sent {
            if let Some(discover_msg) = self.saved_discover.get(&tree_id) {
                if *tree_id != self.my_tree_id {
                    {
                        if CONFIG.debug_options.all || CONFIG.debug_options.hello {
                            let neighbors: Vec<_> = self.neighbors.keys().collect();
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg_discover" };
                            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "discover_msg": discover_msg.value() });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                        }
                    }
                    self.send_msg(line!(), self.connected_tree_id, discover_msg.clone(), user_mask)?;
                } else {
                    println!("Cellagent {}: {} DiscoverMsg not set for {}", self.cell_id, _f, tree_id.get_name());
                }
            }
        }
        if CONFIG.breadth_first {
            for tree_id in &self.discover_ack_d_sent.clone() {
                if self.discover_ack_d_done(*tree_id) {
                    if let Some(discover_ack_d_msg) = self.saved_discover_ack_d.get(&tree_id) {
                        {
                            if CONFIG.debug_options.all || CONFIG.debug_options.hello {
                                let neighbors: Vec<_> = self.neighbors.keys().collect();
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg_discover_ack_d" };
                                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "discover_ack_d_msg": discover_ack_d_msg.value() });
                                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                            }
                        }
                        self.send_msg(line!(), self.connected_tree_id, discover_ack_d_msg.clone(), user_mask)?;
                    }
                } else {
                    println!("Cellagent {}: {} DiscoverAckDMsg not set for {}", self.cell_id, _f, tree_id.get_name());
                }
            }
        }
        Ok(())
    }
    pub fn process_manifest_msg(&mut self, msg: &ManifestMsg, port_no: PortNo, msg_port_tree_id: PortTreeID)
            -> Result<(), Error> {
        let _f = "process_manifest_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_manifest_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let header = msg.get_header();
        let payload = msg.get_payload();
        let manifest = payload.get_manifest();
        let msg_tree_map = header.get_tree_map();
        let deployment_tree_id = payload.get_deploy_port_tree_id();
        let originator_id = header.get_originator_id();
        self.deploy(originator_id, deployment_tree_id, msg_port_tree_id, msg_tree_map, manifest).context(CellagentError::Chain { func_name: "process_ca", comment: S("ManifestMsg") })?;
        let tree_id = payload.get_deploy_port_tree_id();
        let traph = self.get_traph(tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        traph.get_tree_entry(&tree_id.get_uuid()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.manifest {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_manifest_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        Ok(())
    }
    pub fn _process_reroute_msg(&mut self) -> Result<(), Error> {
        let _f = "_process_reroute_msg";
        unimplemented!("Should never get here")
    }
    pub fn process_discover_ack_d_msg(&mut self, msg: &DiscoverAckDMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_discover_ack_d_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discover_ack_d_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_tree_id = msg.get_port_tree_id();
        let tree_id = port_tree_id.to_tree_id();
        if !self.discover_ack_d_sent.contains(&tree_id) && tree_id != self.my_tree_id {
            self.discover_ack_d_sent.insert(tree_id);
            let in_reply_to = msg.get_sender_msg_seq_no();
            let originator_id = msg.get_originator_id();
            let port_number = PortNumber::new(port_no, self.no_ports)?;
            if let Some(discover_msg) = self.saved_discover.get(&tree_id) {
                self.send_msg(line!(), self.connected_tree_id, discover_msg.clone(), DEFAULT_USER_MASK)?;
                self.discover_sent.insert(tree_id);
            } else {
                println!("Cellagent {}: {} DiscoverMsg not set for {}", self.cell_id, _f, tree_id.get_name());
            }
        }
        Ok(())
    }
    pub fn process_discover_ack_msg(&mut self, msg: &DiscoverAckMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_discover_ack_d_d_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca { // Needed for visualization
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discover_ack_d_d_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let tree_id = msg.get_port_tree_id().to_tree_id();
        let count = self.discover_ack_d
            .entry(tree_id)
            .or_insert(0);
        *count += 1;
        if *count == self.neighbors.len() {
            let in_reply_to = msg.get_sender_msg_seq_no();
            let originator_id = msg.get_originator_id();
            let port_tree_id = msg.get_port_tree_id();
            let discover_ack_d_msg = DiscoverAckDMsg::new(in_reply_to, self.cell_id,
                                                   originator_id, port_tree_id);
            self.add_saved_discover_ack_d(&discover_ack_d_msg);
            self.send_msg(line!(), self.connected_tree_id, discover_ack_d_msg, DEFAULT_USER_MASK)?;
            self.discover_ack_d_sent.insert(tree_id);
        }
        Ok(())
    }
    pub fn process_stack_tree_msg(&mut self, msg: &StackTreeMsg, port_no: PortNo, _msg_port_tree_id: PortTreeID)
            -> Result<(), Error> {
        let _f = "process_stack_tree_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca { // Needed for visualization
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_tree_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let header = msg.get_header();
        let payload = msg.get_payload();
        let allowed_tree = payload.get_allowed_tree();
        let parent_port_tree_id = payload.get_parent_port_tree_id();
        let new_port_tree_id = payload.get_new_port_tree_id();
        let originator_id = header.get_originator_id();
        let gvm_eqn = payload.get_gvm_eqn();
        let port_number = port_no.make_port_number(self.get_no_ports())?;
        let entry = self.stack_tree(originator_id, allowed_tree, new_port_tree_id, parent_port_tree_id,
                                    Some(new_port_tree_id), gvm_eqn)?;
        let traph = self.get_traph_mut(new_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        traph.set_tree_entry(&new_port_tree_id.get_uuid(), entry)?;
        let params = traph.get_params(gvm_eqn.get_variables())?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&params)?;
        let gvm_send = gvm_eqn.eval_send(&params)?;
        let gvm_recv = gvm_eqn.eval_recv(&params)?;
        // Update StackTreeMsg and forward
        let parent_entry = self.get_tree_entry(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.stack_tree {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_tree_msg_dbg1" };
                let trace = json!({ "cell_id": &self.cell_id, "new_port_tree_id": new_port_tree_id,
                    "parent_entry": parent_entry, "msg": msg.value(),
                    "gvm_send": gvm_send, "gvm_recv": gvm_recv, "gvm_eqn": gvm_eqn });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let parent_mask = parent_entry.get_mask().all_but_port(PortNumber::new0());
        self.update_entry(&entry)?;
        let child_ports = if gvm_xtnd {
            new_hashset(&parent_mask.get_port_nos())
        } else {
            HashSet::new()
        };
        if child_ports.len() == 0 {  // I am a leaf on the new tree
            let user_mask = Mask::new(port_number);
            let join_tree = gvm_xtnd || gvm_send || gvm_recv;
            let in_reply_to = msg.get_sender_msg_seq_no();
            let new_msg = StackTreeDMsg::new(in_reply_to, self.cell_id,
                                             originator_id, new_port_tree_id,
                                             parent_port_tree_id, join_tree);
            self.send_msg(line!(), self.get_connected_tree_id(), new_msg, user_mask)?;
        } else {
            let updated_msg = msg.update_sender(self.cell_id);
            self.send_msg(line!(), self.connected_tree_id, updated_msg, parent_mask)?; // Send to children of parent tree
        }
        self.child_ports.insert(new_port_tree_id.to_tree_id(), child_ports);
        let parent_port_tree_id = payload.get_parent_port_tree_id();
        let base_tree_id = self.get_base_tree_id(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        self.update_base_tree_map(new_port_tree_id, base_tree_id);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.stack_tree {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_tree_msg_dbg2" };
                let trace = json!({ "cell_id": &self.cell_id, "new_port_tree_id": new_port_tree_id,
                "port_no": port_no, "child_ports": self.child_ports.get(&new_port_tree_id.to_tree_id()), "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    pub fn process_stack_tree_d_msg(&mut self, msg: &StackTreeDMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_stack_treed_msg";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_treed_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let is_joining = msg.is_joining();
        let originator_id = msg.get_header().get_originator_id();
        let port_number = port_no.make_port_number(self.no_ports)?;
        let port_tree_id = msg.get_port_tree_id();
        let parent_port_tree_id = msg.get_parent_port_tree_id();
        let tree_uuid = port_tree_id.get_uuid();
        let traph = self.get_traph_mut(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let parent_port = traph.get_parent_port()?;
        if is_joining {
            let mut entry = traph.get_tree_entry(&tree_uuid).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            let user_mask = Mask::new(port_number);
            let mask = entry.get_mask().or(user_mask);
            entry.set_mask(mask);
            traph.set_tree_entry(&tree_uuid, entry)?;
            self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
        }
        let child_ports = self.child_ports
            .get_mut(&port_tree_id.to_tree_id())
            .expect("Child ports must exist");
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.stack_tree {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "stack_treed_msg_dbg" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": msg, "join": is_joining, "port": port_no, "parent tree": parent_port_tree_id, "child_ports": child_ports });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let old_len = child_ports.len();
        child_ports.remove(&port_no); // Can't move to after if-block due to borrow checker
        if old_len == 1 || (old_len == 0 && parent_port == PortNo(0)) { // I'm not a leaf, and I just heard from my last child, so tell my parent
            let port_number = parent_port.make_port_number(self.no_ports)?;
            if parent_port == PortNo(0) {
                // I am the root of the tree.  I need to tell the sender.
                let allowed_tree_id = port_tree_id.to_tree_id();
                let allowed_tree = self.name_from_tree(originator_id, allowed_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                let originator_id = msg.get_header().get_originator_id();
                self.add_tree_name_map_item(originator_id, &allowed_tree, allowed_tree_id);
                let tree_name_msg = AppTreeNameMsg::new("noc",
                               &AllowedTree::new(&parent_port_tree_id.get_name()),
                                    &allowed_tree);
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_noc_tree_name" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "app_msg": tree_name_msg });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                let serialized = serde_json::to_string(&tree_name_msg as &dyn AppMessage).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) })?;
                let bytes = ByteArray::new(&serialized);
                let port_no = self.get_border_port(&originator_id)?.get_port_no();
                let ca_to_port = self.ca_to_ports.get(&port_no).expect("cellagent.rs: border port sender must be set");
                ca_to_port.send(bytes)?;
            } else {
                let mask = Mask::new(port_number);
                let in_reply_to = msg.get_sender_msg_seq_no();
                let originator_id = msg.get_header().get_originator_id();
                let parent_port_tree_id = msg.get_parent_port_tree_id();
                let new_msg = StackTreeDMsg::new(in_reply_to, self.cell_id,
                                                 originator_id, port_tree_id,
                                                 parent_port_tree_id, true);
                self.send_msg(line!(), self.get_connected_tree_id(), new_msg, mask)?;
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    pub fn process_tree_name_msg(&mut self, msg: &TreeNameMsg, port_no: PortNo) -> Result<(), Error> {
        unimplemented!()
    }
    fn send_base_tree_to_noc(&mut self) -> Result<(), Error> {
        let _f = "send_base_tree_to_noc";
        // The first mover always gets access to Base tree
        let base_tree_name = AllowedTree::new(BASE_TREE_NAME);
        if let Some(port_number) = self.border_port_tree_id_map.keys().next() {
            self.send_tree_name_msg(port_number.get_port_no(), &base_tree_name)?;
            self.sent_to_noc = true;
        }
        Ok(())
    }
    fn find_new_parent(&mut self, header: &MsgHeader, payload: &FailoverMsgPayload, port_no: PortNo)
            -> Result<(), Error> {
        let _f = "find_new_parent";
        let originator_id = header.get_originator_id();
        let rw_port_tree_id = payload.get_rw_port_tree_id();
        let lw_port_tree_id = payload.get_lw_port_tree_id();
        let broken_path = payload.get_broken_path();
        let broken_tree_ids = payload.get_broken_port_tree_ids();
        self.failover_reply_ports.insert(rw_port_tree_id, port_no);
        let port_number = port_no.make_port_number(self.no_ports)?;
        let rw_traph = self.get_traph_mut(rw_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        rw_traph.add_tried_port(rw_port_tree_id, port_no);
        match rw_traph.find_new_parent_port(rw_port_tree_id, broken_path) {
            None => {
                rw_traph.clear_tried_ports(rw_port_tree_id);
                let mask = Mask::new(port_number);
                let in_reply_to = header.get_sender_msg_seq_no();
                let broken_port_number = broken_path.get_port_no().as_usize();
                let no_packets = self.no_packets[broken_port_number];
                let failover_d_msg = FailoverDMsg::new(in_reply_to, self.cell_id,
                                                       originator_id, FailoverResponse::Failure,
                                                       no_packets, payload);
                self.send_msg(line!(), self.connected_tree_id, failover_d_msg, mask)?;
            },
            Some(trial_port_no) => {
                let failover_msg = FailoverMsg::new(self.cell_id,
                                                    originator_id, rw_port_tree_id, lw_port_tree_id,
                                                    broken_path, &broken_tree_ids);
                let mask = Mask::new(trial_port_no.make_port_number(self.no_ports)?);
                self.send_msg(line!(), self.connected_tree_id, failover_msg, mask)?;
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    fn repair_traph(&mut self, broken_port_tree_ids: &HashSet<PortTreeID>, port_number: PortNumber) -> Result<(), Error> {
        let _f = "repair_traph";
        for broken_port_tree_id in broken_port_tree_ids {
            let traph = self.get_traph_mut(*broken_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            let parent_entries = traph.set_parent(port_number, *broken_port_tree_id)?;
            let child_entries = traph.add_child(*broken_port_tree_id, port_number)?;
            self.update_entries(&parent_entries)?;
            self.update_entries(&child_entries)?;
        }
        Ok(())
    }
    fn may_send(&self, port_tree_id: PortTreeID) -> Result<bool, Error> {
        let _f = "may_send";
        let entry = self.get_tree_entry(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        Ok(entry.may_send())
    }
    fn make_tree_map(&self, originator_id: OriginatorID, allowed_trees: &Vec<AllowedTree>)
                     -> Result<MsgTreeMap, Error> {
        let _f = "make_tree_map";
        // Make sure sender has permission to use all tree names included in message
        let mut tree_map = HashMap::new();
        for allowed_tree in allowed_trees {
            let tree_id = self.tree_from_name(originator_id, allowed_tree).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            tree_map.insert(S(allowed_tree.get_name()), tree_id);
        }
        Ok(tree_map)
    }
    pub fn app_interapplication(&mut self, app_msg: &AppInterapplicationMsg, originator_id: OriginatorID)
            -> Result<(), Error> {
        let _f = "app_interapplication";
        let target_tree_name = app_msg.get_target_tree_name();
        let allowed_trees = app_msg.get_allowed_trees();
        let tree_map = self.make_tree_map(originator_id, allowed_trees)?;
        let direction = app_msg.get_direction().into();
        let tree_id = self.tree_from_name(originator_id, target_tree_name).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let port_tree_id = tree_id.to_port_tree_id_0();
        if !self.may_send(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })? {
            return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id }.into());
        }
        let msg = InterapplicationMsg::new(self.cell_id, originator_id,
                                           false, tree_id, direction, &tree_map, app_msg);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.process_msg {   // Debug
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_app_interapplication_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.send_msg(line!(), tree_id, msg, DEFAULT_USER_MASK)?;
        Ok(())
    }
    pub fn app_delete_tree(&mut self, app_msg: &AppDeleteTreeMsg, originator_id: OriginatorID) -> Result<(), Error> {
        let _f = "app_delete_tree";
        let delete_tree_name = app_msg.get_delete_tree_name();
        let delete_tree_id = self.tree_from_name(originator_id, delete_tree_name).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        println!("Cellagent {}: {} deleting tree {}", self.cell_id, _f, delete_tree_id);
        let delete_port_tree_id = delete_tree_id.to_port_tree_id_0();
        if delete_tree_name.get_name() != BASE_TREE_NAME {  // Can't delete base tree
            // Must send on parent tree since some tree members can't read on delete_tree
            if let Some(parent_tree_id) = self.get_parent_tree_id(delete_port_tree_id)? {
                if let Some(is_root) = self.get_parent_tree_entry(delete_port_tree_id)?
                    .map(|entry| entry.get_parent() == PortNo(0)) {
                    if is_root {
                        let msg = DeleteTreeMsg::new(self.cell_id,
                                                     originator_id, delete_tree_id);
                        {
                            if CONFIG.debug_options.all || CONFIG.debug_options.process_msg {   // Debug
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_app_interapplication_msg" };
                                let trace = json!({ "cell_id": &self.cell_id, "delete_tree_id": delete_tree_id, "msg": msg.value() });
                                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                           }
                        }
                        self.send_msg(line!(), parent_tree_id, msg, DEFAULT_USER_MASK)?;
                        self.delete_tree(&delete_tree_id)?;
                    }
                }
            }
        } else {
            return Err(CellagentError::TreeNotAllowed { func_name: _f, cell_id: self.cell_id, originator_id, target_tree: delete_tree_name.clone() }.into());
        }
        Ok(())
    }
    pub fn app_manifest(&mut self, app_msg: &AppManifestMsg, originator_id: OriginatorID) -> Result<(), Error> {
        let _f = "app_manifest";
        let allowed_trees = app_msg.get_allowed_trees();
        let mut tree_map = self.make_tree_map(originator_id, allowed_trees)?;
        let deploy_tree_name = app_msg.get_deploy_tree_name();
        let deploy_tree_id = self.tree_from_name(originator_id, deploy_tree_name).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        tree_map.insert(S(deploy_tree_name.get_name()), deploy_tree_id);
        let deploy_port_tree_id = deploy_tree_id.to_port_tree_id_0();
        if !self.may_send(deploy_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })? {
            return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id: deploy_tree_id }.into());
        }
        let manifest = app_msg.get_payload().get_manifest();
        let msg = ManifestMsg::new(self.cell_id, originator_id,
                                   false, deploy_tree_id.clone(), &tree_map, &manifest);
        let mask = self.get_mask(deploy_port_tree_id)?;
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.process_msg {   // Debug
                let ports = mask.get_port_nos();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_manifest_app_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "deploy_tree_id": deploy_tree_id, "ports": ports, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
         self.send_msg(line!(), deploy_tree_id, msg, mask.or(Mask::port0())).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " send manifest" })?;
        Ok(())
    }
    pub fn app_query(&self, _msg: &AppQueryMsg, _originator_id: OriginatorID) -> Result<MsgTreeMap, Error> {
        let _f = "app_query";
        // Needs may_send test
        Err(UtilityError::Unimplemented { func_name: _f, feature: S("AppMsgType::Query")}.into())
    }
    pub fn app_stack_tree(&mut self, app_msg: &AppStackTreeMsg, originator_id: OriginatorID) -> Result<(), Error> {
        let _f = "app_stack_tree";
        let parent_tree_name = app_msg.get_target_tree_name();
        let new_tree_name = app_msg.get_new_tree_name();
        let gvm_eqn = app_msg.get_gvm();
        let app_msg_direction = app_msg.get_direction();
        let direction = app_msg_direction.into();
        let new_tree_id = self.my_tree_id.add_component(&new_tree_name.get_name()).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " new_tree_id" })?;
        self.add_tree_name_map_item(originator_id, new_tree_name, new_tree_id);
        let parent_tree_id = self.tree_from_name(originator_id, parent_tree_name).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let parent_entry = self.get_tree_entry(parent_tree_id.to_port_tree_id_0()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let parent_mask = parent_entry.get_mask().all_but_port(PortNumber::new0());
        let child_ports = new_hashset(&parent_mask.get_port_nos());
        self.child_ports.insert(new_tree_id, child_ports);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.process_msg {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_stack_tree_app_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "new_tree_id": new_tree_id, "msg": app_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let parent_port_tree_id = parent_tree_id.to_port_tree_id_0();
        if !self.may_send(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })? {
            return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id: parent_tree_id }.into());
        }
        // There is no new_port_tree for up trees
        let parent_entry = self.get_tree_entry(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("get parent_entry") })?;
        let parent_mask = parent_entry.get_mask();
        let stack_tree_msg = StackTreeMsg::new(self.cell_id, originator_id,
                                               new_tree_name, new_tree_id, parent_tree_id, direction, gvm_eqn);
        self.send_msg(line!(), self.control_tree_id, stack_tree_msg, Mask::port0())?;
        Ok(())
    }
    pub fn app_tree_name(&self, _msg: &AppTreeNameMsg, _originator_id: OriginatorID) -> Result<(), Error> {
        let _f = "app_tree_name";
        Err(CellagentError::AppMessageType { func_name: _f, cell_id: self.cell_id, msg: AppMsgType::AppTreeNameMsg }.into())
    }
    /*
    fn send_tree_names(&mut self, outside_tree_id: TreeID, allowed_tree_ids: Vec<TreeID>, port_number: PortNumber) {
        let port_no_mask = Mask::new(port_number);
        let mut allowed_trees = Vec::new();
        for allowed_tree_id in allowed_tree_ids.iter().cloned() {
            let allowed_tree_name = allowed_tree_id.get_name();
            self.tree_name_map.insert(S(allowed_tree_name), allowed_tree_id.clone());
            allowed_trees.push(AllowedTree::new(allowed_tree_name));
        }
        let tree_name_msg = TreeIdMsg::new(&outside_tree_id, &allowed_trees);
        let packets = tree_name_msg.to_packets(outside_tree_id)?;
        self.send_msg(outside_tree_id.get_uuid(), &packets, port_no_mask);
    }
    */
    fn send_tree_name_msg(&self, port_no: PortNo, base_tree_name: &AllowedTree) -> Result<(), Error> {
        let _f = "send_tree_name_msg";
        let tree_name_msg = AppTreeNameMsg::new("cell_agent",
                                                base_tree_name, base_tree_name);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_noc_base_tree_name" };
                let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "app_msg": tree_name_msg });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let serialized = serde_json::to_string(&tree_name_msg as &dyn AppMessage).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) })?;
        let bytes = ByteArray::new(&serialized);
        let ca_to_port = self.ca_to_ports.get(&port_no).expect("cellagent.rs send_tree_name_msg: send port must be set");
        ca_to_port.send(bytes)?;
        Ok(())
    }
    fn port_connected(&mut self, port_no: PortNo, is_border: bool) -> Result<(), Error> {
        let _f = "port_connected";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_connected" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "is_border": is_border });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_number = port_no.make_port_number(self.no_ports)?;
        if is_border {
            // Create tree to talk to outside
            let mut eqns = HashSet::new();
            eqns.insert(GvmEqn::Recv("true"));
            eqns.insert(GvmEqn::Send("true"));
            eqns.insert(GvmEqn::Xtnd("false"));
            eqns.insert(GvmEqn::Save("false"));
            let gvm_eqn = GvmEquation::new(&eqns, &Vec::new());
            let new_tree_id = self.my_tree_id.add_component("Noc").context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) })?;
            self.tree_id_map.insert(new_tree_id.get_uuid(), new_tree_id.to_port_tree_id_0());
            let _ = self.update_traph(new_tree_id.to_port_tree_id(port_number), port_number, PortState::Parent,
                                      &gvm_eqn, HashSet::new(), PathLength(CellQty(1)), Path::new0(), ).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) })?;
            let base_tree_name = AllowedTree::new(BASE_TREE_NAME);
            let originator_id = OriginatorID::new(self.cell_id, &format!("BorderPort+{}", *port_no))?;
            self.add_tree_name_map_item(originator_id,&base_tree_name, self.my_tree_id);
            self.border_port_tree_id_map.insert(port_number, originator_id);
            self.is_border_port_connected = true;
            // Assumes all ports connect before discover process ends for any tree
            Ok(())
        } else {
            if !self.is_border_port(&port_no.make_port_number(self.no_ports)?) {
                self.neighbors.remove(&port_no);
            }
            let originator_id = OriginatorID::new(self.cell_id, "CellAgent")?;
            let user_mask = Mask::new(port_number);
            self.connected_tree_entry.add_child(port_number); // Add to connected ports
            self.update_entry(&self.connected_tree_entry)?;
            let my_port_tree_id = self.my_tree_id.to_port_tree_id(port_number);
            self.update_base_tree_map(my_port_tree_id, self.my_tree_id);
            // I was sending DiscoverMsg here, but now I send it when processing HelloMsg
            let hello_msg = HelloMsg::new(self.cell_id, originator_id,
                                          self.cell_id, port_no);
            let _ = self.send_msg(line!(), self.connected_tree_id, hello_msg, user_mask);
            Ok(())
        }
    }
    fn port_disconnected(&mut self, port_no: PortNo, no_packets: NumberOfPackets) -> Result<(), Error> {
        let _f = "port_disconnected";
        if !self.is_border_port(&port_no.make_port_number(self.no_ports)?) {
            self.neighbors.remove(&port_no);
        }
        let port_number = port_no.make_port_number(self.no_ports)?;
        self.no_packets[port_no.as_usize()] = no_packets;
        self.connected_tree_entry.remove_child(port_number);
        self.update_entry(&self.connected_tree_entry)?;
        let mut broken_port_tree_ids = HashSet::new();
        let mut rw_traph_opt = None;
        for traph in self.traphs.values_mut() {
            traph.set_broken(port_number);
            if traph.has_broken_parent() {
                let broken_port_tree = traph.get_port_tree_by_port_number(&port_number);
                let broken_port_tree_id = broken_port_tree.get_port_tree_id();
                broken_port_tree_ids.insert(broken_port_tree_id);
                if traph.is_one_hop() { rw_traph_opt = Some(traph); }
            }
        }
        let rw_traph = match rw_traph_opt {
            Some(traph) => traph,
            None => {
                println!("Cellagent {}: {} no tree is one hop away on port {}", self.cell_id, _f, *port_no);
                return Ok(())
            }
        };
        let rw_port_tree_id = rw_traph.get_base_tree_id().to_port_tree_id(port_number);
        let broken_path = {
            let broken_element = rw_traph.get_element(port_no)?;
            broken_element.get_path()
        };
        rw_traph.add_tried_port(rw_port_tree_id, port_no);  // Don't try port attached to broken link
        if let Some(trial_parent_port) = rw_traph.find_new_parent_port(rw_port_tree_id, broken_path) {
            rw_traph.add_tried_port(rw_port_tree_id, trial_parent_port);
            let originator_id = OriginatorID::new(self.cell_id, "CellAgent")?;
            let rootward_tree_id = rw_traph.get_base_tree_id();
            let rw_port_number = broken_path.get_port_number();
            let rw_port_tree_id = rootward_tree_id.to_port_tree_id(rw_port_number);
            let lw_port_tree_id = self.my_tree_id.to_port_tree_id(port_number);
            let port_number = trial_parent_port.make_port_number(self.no_ports)?;
            let mask = Mask::new(port_number);
            let failover_msg = FailoverMsg::new(self.cell_id, originator_id,
                                                rw_port_tree_id, lw_port_tree_id,
                                                broken_path, &broken_port_tree_ids);
            println!("Cellagent {}: {} candidate parent for tree {} is port {}", self.cell_id, _f, rw_traph.get_base_tree_id(), *trial_parent_port);
            self.send_msg(line!(), self.connected_tree_id, failover_msg, mask).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
        } else {
            println!("Cellagent {}: {} no candidate parent found for tree {}", self.cell_id, _f, rw_traph.get_base_tree_id())
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    // Added line_no parameter for debugging purposes
    fn send_msg<T: Message>(&self, line_no: u32, tree_id: TreeID, msg: T, user_mask: Mask) -> Result<(), Error>
        where T: Message + Sized + serde::Serialize
    {
        let _f = "send_msg";
        let seq_no = msg.get_sender_msg_seq_no();
        let bytes = msg.to_bytes()?;
        {
            let mask = self.get_mask(tree_id.to_port_tree_id_0())?;
            let port_mask = user_mask.and(mask);
            let port_nos = Mask::get_port_nos(port_mask);
            let msg_type = msg.get_msg_type();
            if CONFIG.debug_options.all || CONFIG.debug_options.ca_msg_send {
                let neighbors = self.neighbor_names_from_port_nos(&port_nos);
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_send_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id,
                    "neighbors": neighbors, "length": bytes.len(), "msg": msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        self.send_bytes(line_no, tree_id, msg.is_ait(), user_mask, seq_no, bytes)?;
        Ok(())
    }
    fn send_bytes(&self, line_no: u32, tree_id: TreeID, is_ait: bool, user_mask: Mask,
                  seq_no: SenderMsgSeqNo, bytes: ByteArray) -> Result<(), Error> {
        let _f = "send_bytes";
        let tree_uuid = tree_id.get_uuid();
        // Make sure tree_id is legit
        self.tree_map
            .get(&tree_uuid)
            .ok_or::<Error>(CellagentError::Tree { func_name: _f, cell_id: self.cell_id, tree_uuid }.into())?;
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca || CONFIG.debug_options.all || CONFIG.debug_options.ca_msg_send {
                let port_nos = user_mask.get_port_nos();
                let neighbors = port_nos.iter()
                    .map(|port_no| self.neighbors.get(&port_no))
                    .filter(|neighbor| neighbor.is_some() )
                    .map(|neighbor| neighbor.unwrap().0.get_name())
                    .collect::<Vec<_>>();
                let msg: Box<dyn Message> = serde_json::from_str(&bytes.to_string()?)?;
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_to_cm_bytes" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id,
                "neighbors": neighbors, "sending line": line_no, "msg": msg });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let msg = CaToCmBytes::Bytes((tree_id, is_ait, user_mask, seq_no, bytes));
        self.ca_to_cm[0].send(msg)?;
        Ok(())
    }
    // For debugging only
    fn _dbg_get_traph_keys(&self) -> Vec<String> {
        let _f = "get_traph_keys";
        self.traphs
            .keys()
            .map(|key| self.tree_id_map.get(key))
            .map(|tree_id| {
                if tree_id.is_some() {
                    tree_id.unwrap().get_name()
                } else {
                    S("None")
                }
            })
            .collect::<Vec<String>>()
    }
}
impl fmt::Display for CellAgent {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Cell Agent {}", self.cell_info);
        for (_, traph) in self.traphs_mutex.lock().unwrap().iter() {
            write!(s, "\n{}", traph)?;
        }
        write!(_f, "{}", s) }
}
// Errors
#[derive(Debug, Fail)]
pub enum CellagentError {
    #[fail(display = "CellagentError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "CellagentError::AppMessageType {}: Unsupported request {:?} from border port on cell {}", func_name, msg, cell_id)]
    AppMessageType { func_name: &'static str, cell_id: CellID, msg: AppMsgType },
    #[fail(display = "CellagentError::BaseTree {}: No base tree for tree {} on cell {}", func_name, tree_id, cell_id)]
    BaseTree { func_name: &'static str, cell_id: CellID, tree_id: PortTreeID },
    #[fail(display = "CellagentError::Border {}: Port {} is not a border port on cell {}", func_name, port_no, cell_id)]
    Border { func_name: &'static str, cell_id: CellID, port_no: u8 },
//    #[fail(display = "CellAgentError::BorderMsgType {}: Message type {} is not accepted from a border port on cell {}", func_name, msg_type, cell_id)]
//    BorderMsgType { func_name: &'static str, cell_id: CellID, msg_type: MsgType },
    #[fail(display = "CellAgentError::FailoverPort {}: No reply port for tree {} on cell {}", func_name, port_tree_id, cell_id)]
    FailoverPort { func_name: &'static str, cell_id: CellID, port_tree_id: PortTreeID },
//    #[fail(display = "CellagentError::ManifestVms {}: No VMs in manifest for cell {}", func_name, cell_id)]
//    ManifestVms { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellagentError::MayNotSend {}: Cell {} does not have permission to delete tree {}", func_name, cell_id, tree_id)]
    MayNotDelete { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
    #[fail(display = "CellagentError::MayNotSend {}: Cell {} does not have permission to send on tree {}", func_name, cell_id, tree_id)]
    MayNotSend { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
    #[fail(display = "CellagentError::Message {}: Malformed request {:?} from border port on cell {}", func_name, msg, cell_id)]
    Message { func_name: &'static str, cell_id: CellID, msg: HashMap<String, String> },
//    #[fail(display = "CellAgentError::NoParentTraph {}: No one hop parent for port {} on cell {}", func_name, port_no, cell_id)]
//    NoParentTraph { cell_id: CellID, func_name: &'static str, port_no: u8 },
    #[fail(display = "CellAgentError::NameMap {}: Sender {} on cell {} has no name to tree entry for {}", func_name, originator_id, cell_id, tree_name)]
    NameMap { func_name: &'static str, cell_id: CellID, tree_name: AllowedTree, originator_id: OriginatorID },
    #[fail(display = "CellAgentError::NoTraph {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_id, cell_id)]
    NoTraph { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
//    #[fail(display = "CellagentError::SavedMsgType {}: Message type {} does not support saving", func_name, msg_type)]
//    SavedMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "CellAgentError::Partition {}: No path from {} to {}", func_name, lw_tree_id, rw_tree_id)]
    Partition { func_name: &'static str, lw_tree_id: TreeID, rw_tree_id: TreeID },
    #[fail(display = "CellAgentError::Sender {}: No port for sender {} on cell {}", func_name, originator_id, cell_id)]
    Sender { func_name: &'static str, cell_id: CellID, originator_id: OriginatorID },
    #[fail(display = "CellAgentError::StackTree {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)]
    StackTree { func_name: &'static str, tree_id: PortTreeID, cell_id: CellID },
//    #[fail(display = "CellAgentError::TenantMask {}: Cell {} has no tenant mask", func_name, cell_id)]
//    TenantMask { func_name: &'static str, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeNameMap {}: Cell {} has no tree name map entry for {}", func_name, cell_id, originator_id)]
    TreeNameMap { func_name: &'static str, cell_id: CellID, originator_id: OriginatorID },
    #[fail(display = "CellAgentError::TreeMap {}: Cell {} has no tree map entry for {} for sender {}", func_name, cell_id, tree_id, originator_id)]
    TreeMap { func_name: &'static str, cell_id: CellID, tree_id: TreeID, originator_id: OriginatorID },
    #[fail(display = "CellAgentError::TreeNotAllowed {}: Sender {} does not have permission to use tree {} on cell {}", func_name, originator_id, target_tree, cell_id)]
    TreeNotAllowed { func_name: &'static str, cell_id: CellID, originator_id: OriginatorID, target_tree: AllowedTree },
    #[fail(display = "CellAgentError::Tree {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid },
//    #[fail(display = "CellAgentError::TreeUuid {}: No tree associated with uuid {:?} on cell {}", func_name, uuid, cell_id)]
//    TreeUuid { func_name: &'static str, uuid: Uuid, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeVmMap {} Cell {} has no tree map entry for {}", func_name, cell_id, tree_id)]
    TreeVmMap { func_name: &'static str, cell_id: CellID, tree_id: TreeID }
}
