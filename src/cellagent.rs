use std::{fmt, fmt::Write,
          sync::{Arc, Mutex, mpsc::channel},
          thread,
          collections::{HashMap, HashSet}};

use serde;
use serde_json;

use crate::app_message::{AppMsgType, AppMsgDirection, AppTreeNameMsg};
use crate::app_message_formats::{CaToVm, VmFromCa, VmToCa, CaFromVm};
use crate::config::{CONNECTED_PORTS_TREE_NAME, CONTINUE_ON_ERROR, CONTROL_TREE_NAME, DEBUG_OPTIONS, QUENCH,
             MAX_NUM_PHYS_PORTS_PER_CELL, TRACE_OPTIONS,
             ByteArray, CellQty, CellType, CellConfig, CellInfo, Quench, PathLength, PortNo, PortQty};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{Message, MsgHeader, MsgDirection, MsgTreeMap, MsgType,
              InterapplicationMsg,
              DiscoverMsg, DiscoverDMsg, DiscoverDType,
              FailoverMsg, FailoverDMsg, FailoverMsgPayload, FailoverResponse,
              HelloMsg,
              ManifestMsg,
              StackTreeMsg, StackTreeDMsg};
use crate::ec_message_formats::{CaToCm, CaFromCm, CaToCmBytes, CmToCaBytes};
use crate::gvm_equation::{GvmEquation, GvmEqn};
use crate::name::{Name, CellID, SenderID, PortTreeID, TreeID, UptreeID, VmID};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::port_tree::PortTree;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::traph::{PortState, Traph};
use crate::tree::Tree;
use crate::uptree_spec::{AllowedTree, Manifest};
use crate::utility::{BASE_TENANT_MASK, DEFAULT_USER_MASK, Mask, Path,
              PortNumber, S, TraceHeader, TraceHeaderParams, TraceType, UtilityError,
              new_hash_set, write_err};
use crate::uuid_ec::Uuid;
use crate::vm::VirtualMachine;

use failure::{Error, ResultExt};

type BorderTreeIDMap = HashMap<PortNumber, (SenderID, TreeID)>;
pub type PortTreeIDMap = HashMap<Uuid, PortTreeID>;
pub type SavedDiscover = DiscoverMsg;
pub type SavedStackMsgs = HashMap<TreeID, Vec<StackTreeMsg>>;
pub type Traphs = HashMap<Uuid, Traph>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeNameMap = HashMap<SenderID, MsgTreeMap>;
pub type TreeVmMap = HashMap<TreeID, Vec<CaToVm>>;

#[derive(Debug, Clone)]
pub struct CellAgent {
    cell_id: CellID,
    cell_type: CellType,
    config: CellConfig,
    cell_info: CellInfo,
    no_ports: PortQty,
    my_tree_id: TreeID,
    control_tree_id: TreeID,
    connected_tree_id: TreeID,
    my_entry: RoutingTableEntry,
    connected_tree_entry: RoutingTableEntry,
    saved_discover: Vec<SavedDiscover>,
    // Next 2 items shared between listen_uptree and listen_cmodel
    tree_name_map: Arc<Mutex<TreeNameMap>>,
    saved_stack: SavedStackMsgs,
    traphs: Traphs,
    traphs_mutex: Arc<Mutex<Traphs>>, // Needed so I can print from main() because I have to clone to get self.traphs into the thread
    tree_map: TreeMap, // Base tree for given stacked tree
    border_port_tree_id_map: BorderTreeIDMap, // Find the tree id associated with a border port
    base_tree_map: HashMap<PortTreeID, TreeID>, // Find the black tree associated with any tree, needed for stacking
    tree_id_map: PortTreeIDMap,
    tenant_masks: Vec<Mask>,
    tree_vm_map: TreeVmMap,
    ca_to_vms: HashMap<VmID, CaToVm>,
    ca_to_cm: CaToCm,
    vm_id_no: usize,
    up_tree_senders: HashMap<UptreeID, HashMap<String,TreeID>>,
    up_traphs_clist: HashMap<TreeID, TreeID>,
    neighbors: HashMap<PortNo, (CellID, PortNo)>,
    failover_reply_ports: HashMap<PortTreeID, PortNo>,
    no_packets: [NumberOfPackets; MAX_NUM_PHYS_PORTS_PER_CELL.0 as usize],
}
impl CellAgent {
    pub fn new(cell_id: CellID, cell_type: CellType, config: CellConfig, no_ports: PortQty,
               ca_to_cm: CaToCm )
               -> Result<CellAgent, Error> {
        let tenant_masks = vec![BASE_TENANT_MASK];
        let my_tree_id = TreeID::new(&cell_id.get_name()).context(CellagentError::Chain { func_name: "new", comment: S("my_tree_id")})?;
        let control_tree_id = TreeID::new(&cell_id.get_name())?.
            add_component(CONTROL_TREE_NAME)?;
        let connected_tree_id = TreeID::new(&cell_id.get_name())?
            .add_component(CONNECTED_PORTS_TREE_NAME)?;
        let mut base_tree_map = HashMap::new();
        base_tree_map.insert(my_tree_id.to_port_tree_id_0(), my_tree_id);
        let my_entry = RoutingTableEntry::default().add_child(PortNumber::new0());
        Ok(CellAgent { cell_id, my_tree_id, cell_type, config, no_ports,
            control_tree_id, connected_tree_id, tree_vm_map: HashMap::new(), ca_to_vms: HashMap::new(),
            traphs: HashMap::new(), traphs_mutex: Arc::new(Mutex::new(HashMap::new())),
            vm_id_no: 0, tree_id_map: HashMap::new(), tree_map: HashMap::new(),
            tree_name_map: Arc::new(Mutex::new(HashMap::new())), border_port_tree_id_map: HashMap::new(),
            saved_discover: Vec::new(), saved_stack: HashMap::new(),
            my_entry, base_tree_map, neighbors: HashMap::new(),
            connected_tree_entry: RoutingTableEntry::default(),
            tenant_masks, up_tree_senders: HashMap::new(), cell_info: CellInfo::new(),
            up_traphs_clist: HashMap::new(), ca_to_cm, failover_reply_ports: HashMap::new(),
            no_packets: [NumberOfPackets::new(); MAX_NUM_PHYS_PORTS_PER_CELL.0 as usize]
        })
    }

    // WORKER (CellAgent)
    pub fn initialize(&mut self, ca_from_cm: CaFromCm) -> Result<&mut Self, Error> {
        let _f = "initialize";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        // Set up predefined trees - Must be first two in this order
        let port_number_0 = PortNumber::new0();
        let hops = PathLength(CellQty(0));
        let path = Path::new0();
        let control_tree_id = self.control_tree_id;
        let connected_tree_id = self.connected_tree_id;
        let my_tree_id = self.my_tree_id;
        self.tree_id_map.insert(control_tree_id.get_uuid(), control_tree_id.to_port_tree_id_0());
        self.tree_id_map.insert(connected_tree_id.get_uuid(), connected_tree_id.to_port_tree_id_0());
        self.tree_id_map.insert(my_tree_id.get_uuid(), my_tree_id.to_port_tree_id_0());
        self.tree_map.insert(control_tree_id.get_uuid(), control_tree_id.get_uuid());
        self.tree_map.insert(connected_tree_id.get_uuid(), connected_tree_id.get_uuid());
        self.tree_map.insert(my_tree_id.get_uuid(), my_tree_id.get_uuid());
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        self.update_traph(control_tree_id.to_port_tree_id_0(), port_number_0,
                          PortState::Parent, &gvm_equation,
                          HashSet::new(), hops, path)?;
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("false"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        let connected_tree_entry = self.update_traph(connected_tree_id.to_port_tree_id_0(),
                                                     port_number_0,
                                                     PortState::Parent, &gvm_equation,
                                                     HashSet::new(), hops, path)?;
        self.connected_tree_entry = connected_tree_entry;
        // Create my tree
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        self.my_entry = self.update_traph(my_tree_id.to_port_tree_id_0(), port_number_0,
                                          PortState::Parent, &gvm_equation,
                                          HashSet::new(), hops, path)?;
        self.listen_cm(ca_from_cm)?;
        Ok(self)
    }
    fn get_no_ports(&self) -> PortQty { self.no_ports }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_connected_tree_id(&self) -> TreeID { self. connected_tree_id }
    pub fn _get_control_tree_id(&self) -> TreeID { self.control_tree_id }
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
    fn get_gvm_eqn(&self, port_tree_id: PortTreeID) -> Result<GvmEquation, Error> {
        let _f = "get_gvm_eqn";
        let tree_uuid = port_tree_id.get_uuid();
        let traph = self.get_traph(port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let tree = traph.get_tree(&tree_uuid)?;
        let gvm_eqn = tree.get_gvm_eqn().clone();
        Ok(gvm_eqn.clone())
    }
    fn get_saved_discover(&self) -> &Vec<SavedDiscover> { &self.saved_discover }
    fn add_saved_discover(&mut self, discover_msg: &SavedDiscover) {
        let _f = "add_saved_discover";
        let port_tree_id = discover_msg.get_port_tree_id();
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_save_discover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": port_tree_id, "msg": &discover_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.saved_discover {
                println!("Cell {}: save discover {}", self.cell_id, discover_msg);
            }
        }
        self.saved_discover.push(discover_msg.clone());
    }
    fn add_saved_stack_tree(&mut self, port_tree_id: PortTreeID, stack_tree_msg: &StackTreeMsg) {
        let _f = "add_saved_stack_tree";
        let mut saved_msgs = self.saved_stack
                .get_mut(&port_tree_id.to_tree_id())
                .cloned()
                .unwrap_or_default();
        saved_msgs.push(stack_tree_msg.clone());
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_save_stack_tree_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": port_tree_id, "no_saved": saved_msgs.len(), "msg": &stack_tree_msg });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                if DEBUG_OPTIONS.all || DEBUG_OPTIONS.saved_msgs {   // Debug print
                    println!("Cellagent {}: {} saving {} msg {}", self.cell_id, _f, saved_msgs.len(), stack_tree_msg);
                }
            }
        }
        self.saved_stack.insert(port_tree_id.to_tree_id(), saved_msgs);
    }
    fn add_tree_name_map_item(&mut self, sender_id: SenderID, allowed_tree: &AllowedTree, allowed_tree_id: TreeID) {
        let _f = "add_tree_name_map_item";
        let mut locked = self.tree_name_map.lock().unwrap();
        let mut tree_map = locked
            .get(&sender_id)
            .cloned()
            .unwrap_or_default();
        tree_map.insert(S(allowed_tree.get_name()), allowed_tree_id);
        locked.insert(sender_id, tree_map);
    }
    fn update_base_tree_map(&mut self, stacked_tree_id: PortTreeID, base_tree_id: TreeID) {
        let _f = "update_base_tree_map";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_update_base_tree_map" };
                let trace = json!({ "cell_id": &self.cell_id, "stacked_tree_id": stacked_tree_id, "base_tree_id": base_tree_id, });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.traph_entry {
                println!("Cellagent {}: {}: stacked tree {}, base tree {}", self.cell_id, _f, stacked_tree_id, base_tree_id);
            }
        }
        self.base_tree_map.insert(stacked_tree_id, base_tree_id);
        self.tree_map.insert(stacked_tree_id.get_uuid(), base_tree_id.get_uuid());
    }
    fn get_base_tree_id(&self, port_tree_id: PortTreeID) -> Result<TreeID, Error> {
        let _f = "get_base_tree_id";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {   // Debug print
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_get_base_tree_id" };
                let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": port_tree_id });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.traph_entry {
                println!("Cell {}: {}: stacked tree {}", self.cell_id, _f, port_tree_id);
            }
        }
        self.base_tree_map
            .get(&port_tree_id)
            .cloned()
            .ok_or(CellagentError::BaseTree { func_name: _f, cell_id: self.cell_id, tree_id: port_tree_id }.into())
    }
    //pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
    // These functions specify the Discover quenching algorithms
    fn quench_simple(&self, tree_id: TreeID) -> bool {
        self.traphs.contains_key(&tree_id.get_uuid())
    }
    fn quench_root_port(&self, tree_id: TreeID, path: Path) -> bool {
        let _f = "quench_root_port";
        self.traphs
            .get(&tree_id.get_uuid())
            .map_or(false, |traph| {
                let port_no = path.get_port_no();
                traph.get_port_trees()
                    .iter()
                    .map(|port_tree| -> bool { port_tree.1.get_root_port_no() == port_no })
                    .any(|b| b)
                })
    }
    fn update_traph(&mut self, base_port_tree_id: PortTreeID, port_number: PortNumber, port_state: PortState,
                    gvm_eqn: &GvmEquation, mut children: HashSet<PortNumber>,
                    hops: PathLength, path: Path)
                    -> Result<RoutingTableEntry, Error> {
        let _f = "update_traph";
        let base_tree_id = base_port_tree_id.to_tree_id();
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_update_traph" };
                let trace = json!({ "cell_id": &self.cell_id,
                "base_tree_id": base_port_tree_id, "port_number": &port_number, "hops": &hops,
                "port_state": &port_state,
                "children": children, "gvm": &gvm_eqn });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        let mut traph = self.traphs
            .remove(&base_tree_id.get_uuid()) // Remove to avoid borrow problems on self
            .unwrap_or(Traph::new(&self.cell_id, self.no_ports, base_tree_id, gvm_eqn)?);
        let (gvm_recv, gvm_send, _gvm_xtnd, _gvm_save) =  {
                let variables = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
                let recv = gvm_eqn.eval_recv(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_recv") })?;
                let send = gvm_eqn.eval_send(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_send") })?;
                let xtnd = gvm_eqn.eval_xtnd(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_xtnd") })?;
                let save = gvm_eqn.eval_save(&variables).context(CellagentError::Chain { func_name: _f, comment: S("eval_save") })?;
                (recv, send, xtnd, save)
        };
        let (updated_hops, _) = match port_state {
            PortState::Child => {
                let element = traph.get_parent_element().context(CellagentError::Chain { func_name: _f, comment: S("") })?;
                // Need to coordinate the following with DiscoverMsg.update_discover_msg
                (element.hops_plus_one(), element.get_path())
            },
            _ => (hops, path)
        };
        let traph_status = traph.get_port_status(port_number);
        let entry_port_status = match traph_status {
            PortState::Pruned | PortState::Unknown => port_state,
            _ => traph_status  // Don't replace if Parent or Child
        };
        if gvm_recv { children.insert(PortNumber::new0()); }
        let mut entry = traph.update_element(base_tree_id, port_number, entry_port_status, &children, updated_hops, path).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
        if gvm_send { entry.enable_send() } else { entry.disable_send() }
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_updated_traph_entry" };
                let trace = json!({ "cell_id": &self.cell_id, "base_tree_id": base_tree_id, "entry": &entry });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.traph_entry {
                println!("CellAgent {}: entry {}", self.cell_id, entry);
            }
        }
        // Need traph even if cell only forwards on this tree
        self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S("base_tree_id") })?;
        let mut port_tree = traph
            .own_port_tree(base_port_tree_id)
            .unwrap_or(PortTree::new(base_port_tree_id, port_number.get_port_no(), updated_hops));
        if base_tree_id != self.my_tree_id {  // Not my tree
            // The first port_tree entry is the one that denotes this branch
            let first_port_tree_id = traph.add_port_tree(&port_tree);
            let mut first_port_tree = traph.own_port_tree(first_port_tree_id).unwrap(); // Unwrap safe by previous line
            let mut new_entry = entry; // Copy so entry won't change when new_entry does
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
    fn deploy(&mut self, sender_id: SenderID, deployment_port_tree_id: PortTreeID, _msg_tree_id: PortTreeID,
                  msg_tree_map: &MsgTreeMap, manifest: &Manifest) -> Result<(), Error> {
        let _f = "deploy";
        let mut tree_map = self.tree_name_map.lock().unwrap()
            .get(&sender_id)
            .cloned()
            .ok_or::<Error>(CellagentError::TreeNameMap { cell_id: self.cell_id, func_name: _f, sender_id }.into())?;
        for allowed_tree in manifest.get_allowed_trees() {
            // Want match so I can return on an error
            match msg_tree_map.get(allowed_tree.get_name()) {
                Some(tree_id) => tree_map.insert(S(allowed_tree.get_name()), *tree_id),
                None => return Err(CellagentError::TreeMap { cell_id: self.cell_id, func_name: _f, tree_name: allowed_tree.clone() }.into())
            };
        }
        // TODO: The next line breaks confinement by giving every application permission to send on the black tree
        tree_map.insert(S("Base"), self.my_tree_id); // <--- Breaks confinement
        // TODO: End of confinement breaking code
        for vm_spec in manifest.get_vms() {
            let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
            let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
            let container_specs = vm_spec.get_containers();
            let vm_id = VmID::new(self.cell_id, &vm_spec.get_id())?;
            let vm_allowed_trees = vm_spec.get_allowed_trees();
            let vm_sender_id = SenderID::new(self.cell_id, &vm_id.get_name())?;
            let up_tree_name = vm_spec.get_id();
            let mut allowed_trees = HashSet::new();
            allowed_trees.insert(AllowedTree::new(CONTROL_TREE_NAME));
            let mut vm = VirtualMachine::new(&vm_id, vm_to_ca, vm_allowed_trees);
            vm.initialize(up_tree_name, vm_from_ca, &allowed_trees, container_specs)?;
            for vm_allowed_tree in vm_allowed_trees {
                tree_map.get(vm_allowed_tree.get_name())
                    .ok_or::<Error>(CellagentError::TreeMap { cell_id: self.cell_id, func_name: "deploy(vm)", tree_name: vm_allowed_tree.clone() }.into())
                    .map(|allowed_tree_id| {
                        allowed_trees.insert(vm_allowed_tree.clone());
                        self.add_tree_name_map_item(sender_id, vm_allowed_tree, allowed_tree_id.clone());
                        self.add_tree_name_map_item(vm_sender_id, vm_allowed_tree, allowed_tree_id.clone());
                        // Functional style runs into a borrow problem
                        match self.tree_vm_map.get_mut(allowed_tree_id) {
                            Some(senders) => senders.push(ca_to_vm.clone()),
                            None => { self.tree_vm_map.insert(allowed_tree_id.clone(), vec![ca_to_vm.clone()]); }
                        }
                    })?;
            }
            let keys = self.tree_vm_map.keys().collect::<Vec<_>>();
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_deploy" };
                    let trace = json!({ "cell_id": &self.cell_id,
                    "deployment_port_tree_id": deployment_port_tree_id, "tree_vm_map_keys":  &keys,
                    "up_tree_name": up_tree_name });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                }
                if DEBUG_OPTIONS.all || DEBUG_OPTIONS.deploy {
                    println!("Cellagent {}: deployment tree {}", self.cell_id, deployment_port_tree_id);
                    println!("Cellagent {}: added vm senders {:?}", self.cell_id, self.tree_vm_map.keys());
                    println!("Cellagent {}: starting VM on up tree {}", self.cell_id, up_tree_name);
                }
            }
            self.ca_to_vms.insert(vm_id, ca_to_vm,);
            self.listen_uptree(vm_sender_id, vm_id, allowed_trees, ca_from_vm);
        }
        Ok(())
    }
    // SPAWN THREAD (listen_uptree_loop)
    fn listen_uptree(&self, sender_id: SenderID, vm_id: VmID, trees: HashSet<AllowedTree>,
                     ca_from_vm: CaFromVm) {
        let _f = "listen_uptree";
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {} listen_uptree_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = ca.listen_uptree_loop(sender_id, vm_id, &ca_from_vm).map_err(|e| write_err("cellagent", &e));
            if CONTINUE_ON_ERROR { ca.listen_uptree(sender_id, vm_id, trees, ca_from_vm); }
        }).expect("thread failed");
    }

    // WORKER (CaFromVm)
    fn listen_uptree_loop(&mut self, sender_id: SenderID, _vm_id: VmID, ca_from_vm: &CaFromVm)
            -> Result<(), Error> {
        let _f = "listen_uptree_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = ca_from_vm.recv()?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_from_uptree" };
                    let trace = json!({ "cell_id": &self.cell_id, "msg": msg });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let (is_ait, allowed_tree, msg_type, direction, bytes) = msg;
            let tree_map = self.tree_name_map.lock().unwrap()
                .get(&sender_id)
                .cloned()
                .ok_or::<Error>(CellagentError::TreeNameMap { func_name: _f, cell_id: self.cell_id, sender_id }.into())?;
            let serialized = ::std::str::from_utf8(&bytes)?;
            let tree_map_updated = match msg_type {
                AppMsgType::Interapplication => self.app_interapplication(sender_id, is_ait, &allowed_tree, serialized, direction, &tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_interapplication") })?,
                AppMsgType::DeleteTree => self.app_delete_tree(sender_id, serialized, direction, &tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_delete_tree") })?,
                AppMsgType::Manifest => self.app_manifest(sender_id, serialized, direction, &tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_manifest") })?,
                AppMsgType::Query => self.app_query(sender_id, serialized, direction, &tree_map,).context(CellagentError::Chain { func_name: _f, comment: S("app_query") })?,
                AppMsgType::StackTree => self.app_stack_tree(sender_id, serialized, direction, &tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_stack_tree") })?,
                AppMsgType::TreeName => self.app_tree_name(sender_id, serialized, direction, &tree_map,).context(CellagentError::Chain { func_name: _f, comment: S("app_tree_name") })?,
            };
            self.tree_name_map.lock().unwrap().insert(sender_id, tree_map_updated);
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
            self.send_msg(target_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.get_id())})?;
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
    fn stack_tree(&mut self, sender_id: SenderID, allowed_tree: &AllowedTree,
                      new_port_tree_id: PortTreeID, parent_tree_id: PortTreeID, new_port_tree_id_opt: Option<PortTreeID>,
                      gvm_eqn: &GvmEquation) -> Result<Option<RoutingTableEntry>, Error> {
        let _f = "stack_tree";
        let no_ports = self.no_ports;
        let base_tree_id = self.get_base_tree_id(parent_tree_id)
            .unwrap_or_else(|_| {
                let tree_id = parent_tree_id.to_tree_id();
                self.update_base_tree_map(new_port_tree_id, tree_id);
                tree_id
            });
        self.add_tree_name_map_item( sender_id, allowed_tree, new_port_tree_id.to_tree_id());
        self.update_base_tree_map(new_port_tree_id, base_tree_id);
        let traph = self.get_traph_mut(parent_tree_id).context(CellagentError::Chain { func_name: "stack_tree", comment: S("own_traph")})?;
        if traph.has_tree(new_port_tree_id) { return Ok(None); } // Check for redundant StackTreeMsg
        let parent_entry = traph.get_tree_entry(&parent_tree_id.get_uuid()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("get_tree_entry")})?;
        let mut entry = parent_entry; // Copy so parent_entry won't change when entry does
        entry.set_uuid(&new_port_tree_id.get_uuid());
        let params = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("get_params")})?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&params).context(CellagentError::Chain { func_name: _f, comment: S("gvm_xtnd")})?;
        let gvm_send = gvm_eqn.eval_send(&params).context(CellagentError::Chain { func_name: _f, comment: S("gvm_send")})?;
        if !gvm_xtnd { entry.clear_children(); }
        if gvm_send  { entry.enable_send(); } else { entry.disable_send(); }
        let gvm_recv = gvm_eqn.eval_recv(&params).context(CellagentError::Chain { func_name: _f, comment: S("eval_recv")})?;
        let mask = if gvm_recv {
            entry.get_mask().or(Mask::port0())
        } else {
            entry.get_mask().and(Mask::all_but_zero(no_ports))
        };
        entry.set_mask(mask);
        let tree = Tree::new(new_port_tree_id, base_tree_id, parent_tree_id, &gvm_eqn, entry);
        traph.stack_tree(tree);
        self.tree_map.insert(new_port_tree_id.get_uuid(), base_tree_id.get_uuid());
        self.tree_id_map.insert(new_port_tree_id.get_uuid(), new_port_tree_id);
        // TODO: Make sure that stacked tree entries for port trees get created
        self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S("")})?;
        // Next line avoids a mutability error; requires NLL
        let traph = self.get_traph_mut(parent_tree_id).context(CellagentError::Chain { func_name: "stack_tree", comment: S("own_traph")})?;
        // I get a mutability error if I use map for the following
        match new_port_tree_id_opt {
            Some(new_port_tree_id) => {
                entry.set_tree_id(new_port_tree_id);
                let tree = Tree::new(new_port_tree_id, base_tree_id, parent_tree_id, &gvm_eqn, entry);
                traph.stack_tree(tree);
                self.update_entry(&entry)?;
            },
            None => () // Called from app_stack_tree, so no port tree defined
        }
        let keys: Vec<PortTreeID> = self.base_tree_map.iter().map(|(k, _)| k.clone()).collect();
        let values: Vec<TreeID> = self.base_tree_map.iter().map(|(_, v)| v.clone()).collect();
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_stack_tree" };
                let trace = json!({ "cell_id": &self.cell_id,
                "new_port_tree_id": &new_port_tree_id, "base_tree_id": &base_tree_id,
                "base_tree_map_keys": &keys, "base_tree_map_values": &values });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.stack_tree { // Debug print
                println!("Cellagent {}: {} added new tree {} {} with base tree {} {}", self.cell_id, _f, new_port_tree_id, new_port_tree_id.get_uuid(), base_tree_id, base_tree_id.get_uuid());
                println!("Cellagent {}: {} base tree map {:?}", self.cell_id, _f, self.base_tree_map);
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(Some(parent_entry))
    }
    fn update_entries(&self, entries: &[RoutingTableEntry]) -> Result<(), Error> {
        let _f = "update_entries";
        for entry in entries { self.update_entry(entry)?; }
        Ok(())
    }
    fn update_entry(&self, entry: &RoutingTableEntry) -> Result<(), Error> {
        let _f = "update_entry";
        self.ca_to_cm.send(CaToCmBytes::Entry(*entry)).context(CellagentError::Chain { func_name: _f, comment: S("")})?;
        Ok(())
    }

    // SPAWN THREAD (listen_cm_loop)
    fn listen_cm(&mut self, ca_from_cm: CaFromCm) -> Result<(), Error>{
        let _f = "listen_cm";
        let mut ca = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {} listen_cm_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = ca.listen_cm_loop(&ca_from_cm).map_err(|e| write_err("cellagent", &e));
            // println!("Cellagent {}: Back from listen_cm_loop", ca.cell_id);
            if CONTINUE_ON_ERROR { let _ = ca.listen_cm(ca_from_cm); }
        })?;
        Ok(())
    }

    // WORKER (CaFromCm)
    fn listen_cm_loop(&mut self, ca_from_cm: &CaFromCm) -> Result<(), Error> {
        let _f = "listen_cm_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = ca_from_cm.recv().context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id)})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_from_cm" };
                    let trace = json!({ "cell_id": &self.cell_id, "msg": &msg });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            match msg {
                CmToCaBytes::Status((port_no, is_border, number_of_packets, status)) => match status {
                    PortStatus::Connected => self.port_connected(port_no, is_border).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " port_connected"})?,
                    PortStatus::Disconnected => self.port_disconnected(port_no, number_of_packets).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " port_disconnected"})?
                },
                CmToCaBytes::Bytes((port_no, is_ait, uuid, bytes)) => {
                    // The index may be pointing to the control tree because the other cell didn't get the StackTree or StackTreeD message in time
                    let mut msg = MsgType::msg_from_bytes(&bytes).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id)})?;
                    let msg_tree_id = {  // Use control tree if uuid not found
                        self.tree_id_map
                            .get(&uuid)
                            .unwrap_or(&self.control_tree_id.to_port_tree_id_0())
                            .clone()
                    };
                    {
                        if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_msg" };
                            let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.value(), "port_no": port_no });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                        }
                        if DEBUG_OPTIONS.all || DEBUG_OPTIONS.ca_msg_recv {   //Debug print
                            match msg.get_msg_type() {
                                MsgType::Discover => (),
                                MsgType::DiscoverD => {
                                    if msg.get_port_tree_id().is_name("Tree:C:2") {
                                        println!("Cellagent {}: {} Port {} received {}", self.cell_id, _f, *port_no, msg);
                                    }
                                },
                                _ => {
                                    println!("Cellagent {}: {} Port {} received {}", self.cell_id, _f, *port_no, msg);
                                }
                            }
                        }
                    }
                    msg.process_ca(self, port_no, msg_tree_id, is_ait).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id)})?;
                },
                CmToCaBytes::App((port_no, (is_ait, allowed_tree, msg_type, direction, bytes))) => {
                    let port_number = port_no.make_port_number(self.no_ports).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " PortNumber" })?;
                    let sender_id = self.border_port_tree_id_map
                        .get(&port_number)
                        .cloned()
                        .ok_or::<Error>(CellagentError::Border { func_name: _f, cell_id: self.cell_id, port_no: *port_no }.into())?
                        .0;
                    let tree_map= &(self.tree_name_map.lock().unwrap()
                        .get(&sender_id)
                        .cloned()
                        .ok_or::<Error>(CellagentError::TreeNameMap { func_name: _f, cell_id: self.cell_id,  sender_id }.into())?);
                    let serialized = ::std::str::from_utf8(&bytes)?;
                    let tree_map_updated = match msg_type {
                        AppMsgType::Interapplication => self.app_interapplication(sender_id, is_ait, &allowed_tree, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_interapplication")})?,
                        AppMsgType::DeleteTree  => self.app_delete_tree(sender_id, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_delete_tree")})?,
                        AppMsgType::Manifest    => self.app_manifest(sender_id, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_manifest")})?,
                        AppMsgType::Query       => self.app_query(sender_id, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_query")})?,
                        AppMsgType::StackTree   => self.app_stack_tree(sender_id, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_stack_tree")})?,
                        AppMsgType::TreeName    => self.app_tree_name(sender_id, &serialized, direction, tree_map).context(CellagentError::Chain { func_name: _f, comment: S("app_tree_name")})?,
                    };
                    self.tree_name_map.lock().unwrap().insert(sender_id, tree_map_updated);
                }
            }
        }
    }
    pub fn process_interapplication_msg(&mut self, msg: &InterapplicationMsg, port_no: PortNo, msg_tree_id: PortTreeID,
                                   is_ait: bool) -> Result<(), Error> {
        let _f = "process_interapplication_msg";
        let port_tree_id = msg.get_port_tree_id();
        let gvm_eqn = self.get_gvm_eqn(port_tree_id)?;
        let save = self.gvm_eval_save(msg_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id)})?;
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_interapplication_msg" };
                let trace = json!({ "cell_id": &self.cell_id,"port_tree_id": port_tree_id, "port_no": port_no, "save": save, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.process_msg {
                println!("Cellagent {}: {} tree {} port {} save {} msg {}", self.cell_id, _f, port_tree_id, *port_no, save, msg);
            }
        }
        let senders = self.get_vm_senders(port_tree_id.to_tree_id()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        for sender in senders {
            sender.send((is_ait, msg.get_payload().get_body().clone())).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
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
        let tree_seen = self.quench_simple(new_tree_id);
        // Send DiscoverD::First to sender first time this tree is seen, otherwise DiscoverD::Subsequent
        let discover_type = if !tree_seen {
            DiscoverDType::First
        } else {
            DiscoverDType::Subsequent
        };
        let sender_id = SenderID::new(self.cell_id, "CellAgent")?;
        let in_reply_to = msg.get_sender_msg_seq_no();
        let discoverd_msg = DiscoverDMsg::new(in_reply_to, sender_id,
                                              self.cell_id, new_port_tree_id, path, discover_type);
        let mask = Mask::new(port_number);
        self.send_msg(self.get_connected_tree_id(),
                          &discoverd_msg, mask).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg") })?;
        let port_tree_seen = self.quench_root_port(new_tree_id, path);
        let quench = match QUENCH {
            Quench::Simple => tree_seen,     // Must see this tree once
            Quench::RootPort => port_tree_seen // Must see every root port for this tree once
        };
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(&eqns, &Vec::new());
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "quench": quench, "new_port_tree_id": new_tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.discover {
                println!("Cellagent {}: {} tree_id {}, port_number {} {}", self.cell_id, _f, new_port_tree_id, port_number, msg);
            }
        }
        if !port_tree_seen {
            let port_state = match discover_type {
                DiscoverDType::First => PortState::Parent,
                DiscoverDType::Subsequent => PortState::Pruned
            };
            self.update_traph(new_port_tree_id, port_number, port_state, &gvm_equation,
                              HashSet::new(), hops, path).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg") })?;
        }
        self.update_base_tree_map(new_port_tree_id, new_tree_id);
        // The following is needed until I get port trees and trees straighened out.
        self.update_base_tree_map(new_tree_id.to_port_tree_id_0(), new_tree_id);
        if quench { return Ok(()); }
        // Forward Discover on all except port_no with updated hops and path
        let updated_msg = msg.update(self.cell_id);
        let user_mask = DEFAULT_USER_MASK.all_but_port(port_no.make_port_number(self.no_ports).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg") })?);
        self.send_msg(self.get_connected_tree_id(),
                      &updated_msg, user_mask).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg") })?;
        self.add_saved_discover(&msg); // Discover message are always saved for late port connect
        Ok(())
    }
    pub fn process_discover_d_msg(&mut self, msg: &DiscoverDMsg, port_no: PortNo)
                                  -> Result<(), Error> {
        let _f = "process_discoverd_msg";
        let payload = msg.get_payload();
        let base_port_tree_id = payload.get_port_tree_id();
        let discover_type = payload.get_discover_type();
        if discover_type == DiscoverDType::Subsequent {
            let traph = self.get_traph_mut(base_port_tree_id)?;
            let element = traph.get_element_mut(port_no)?;
            element.set_connected();
            if element.is_state(PortState::Unknown) {
                element.mark_pruned();
            }
            return Ok(());
        }
        let path = payload.get_path();
        let port_number = port_no.make_port_number(self.no_ports)?;
        let children = new_hash_set(Box::new([port_number]));
        let port_state = PortState::Child;
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("false"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(&eqns, &Vec::new());
        let _ = self.update_traph(base_port_tree_id, port_number, port_state, &gvm_eqn,
                                  children, PathLength(CellQty(0)), path)?;
        let mask = Mask::new(port_number);
        self.forward_stacked_trees(base_port_tree_id, mask)?;
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_discover_d_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": base_port_tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.discoverd {
                println!("Cellagent {}: {} port_tree_id {}, add child on port {} {}", self.cell_id, _f, base_port_tree_id, port_number, msg);
                println!("Cellagent {}: {} send unblock", self.cell_id, _f);
            }
        }
        Ok(())
    }
    pub fn process_failover_msg(&mut self, msg: &FailoverMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_failover_msg";
        let _cell_id = self.cell_id; // Needed for debug printouts
        let header = msg.get_header();
        let payload = msg.get_payload();
        let sender_id = header.get_sender_id();
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

            let failover_d_msg = FailoverDMsg::new(in_reply_to, sender_id,
                                                   FailoverResponse::Success,
                                                   no_packets, payload);
            self.send_msg(self.connected_tree_id, &failover_d_msg, mask)?;
        } else {
            self.failover_reply_ports.insert(rw_port_tree_id, port_no);
            self.find_new_parent(header, payload, port_no).context(CellagentError::Chain { func_name: _f, comment: S("find_new_parent") })?;
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    pub fn process_failover_d_msg(&mut self, msg: &FailoverDMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_failover_d_msg";
        let _cell_id = self.cell_id; // Needed for debug print
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
                    self.ca_to_cm.send(CaToCmBytes::Reroute((broken_port_no, port_no, no_packets)))?;
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
                            let sender_id = header.get_sender_id();
                            let broken_port_number = payload
                                .get_failover_payload()
                                .get_broken_path()
                                .get_port_number()
                                .as_usize();
                            let failover_d_msg = FailoverDMsg::new(in_reply_to, sender_id,
                                                                   FailoverResponse::Success,
                                                                   self.no_packets[broken_port_number],
                                                                   payload.get_failover_payload());
                            self.send_msg(self.connected_tree_id, &failover_d_msg, mask)
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
    pub fn process_hello_msg(&mut self, msg: &HelloMsg, port_no: PortNo)
            -> Result<(), Error> {
        let _f = "process_hello_msg";
        let payload = msg.get_payload();
        let neighbor_cell_id = payload.get_cell_id();
        let neigbor_port_no = payload.get_port_no();
        self.neighbors.insert(port_no, (neighbor_cell_id, *neigbor_port_no));
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_hello_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "recv_port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.process_msg {   // Debug
                let sending_cell = payload.get_cell_id();
                let sending_port = payload.get_port_no();
                println!("Cellagent {}: {} sending cell {} sending port {}", self.cell_id, _f, sending_cell, **sending_port);
            }
        }
        Ok(())
    }
    pub fn process_manifest_msg(&mut self, msg: &ManifestMsg, port_no: PortNo, msg_port_tree_id: PortTreeID)
            -> Result<(), Error> {
        let _f = "process_manifest_msg";
        let header = msg.get_header();
        let payload = msg.get_payload();
        let manifest = payload.get_manifest();
        let msg_tree_map = header.get_tree_map();
        let deployment_tree_id = payload.get_deploy_port_tree_id();
        let sender_id = header.get_sender_id();
        self.deploy(sender_id, deployment_tree_id, msg_port_tree_id, msg_tree_map, manifest).context(CellagentError::Chain { func_name: "process_ca", comment: S("ManifestMsg")})?;
        let tree_id = payload.get_deploy_port_tree_id();
        let traph = self.get_traph(tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        traph.get_tree_entry(&tree_id.get_uuid()).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let gvm_eqn = self.get_gvm_eqn(tree_id)?;
        let save = self.gvm_eval_save(msg_port_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id)})?;
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_manifest_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.manifest {
                println!("Cellagent {}: {} tree {} save {} port {} manifest {}", self.cell_id, _f, msg_port_tree_id, save, *port_no, manifest.get_id());
            }
        }
        Ok(())
    }
    pub fn _process_reroute_msg(&self) -> Result<(), Error> { panic!("Should never get here") }
    pub fn process_stack_tree_msg(&mut self, msg: &StackTreeMsg, port_no: PortNo, msg_port_tree_id: PortTreeID)
            -> Result<(), Error> {
        let _f = "process_stack_tree_msg";
        let header = msg.get_header();
        let payload = msg.get_payload();
        let allowed_tree = payload.get_allowed_tree();
        let parent_tree_id = payload.get_parent_port_tree_id();
        let new_port_tree_id = payload.get_new_port_tree_id();
        let sender_id = header.get_sender_id();
        let gvm_eqn = payload.get_gvm_eqn();
        let port_number = port_no.make_port_number(self.get_no_ports())?;
        if let Some(entry) = self.stack_tree(sender_id, allowed_tree, new_port_tree_id, parent_tree_id,
                                             Some(new_port_tree_id), gvm_eqn)? {
            let traph = self.get_traph_mut(new_port_tree_id)?;
            traph.set_tree_entry(&new_port_tree_id.get_uuid(), entry)?;
            // Update StackTreeMsg and forward
            let parent_entry = self.get_tree_entry(parent_tree_id)?;
            let parent_mask = parent_entry.get_mask().all_but_port(PortNumber::new0());
            self.send_msg(self.connected_tree_id, msg, parent_mask)?; // Send to children of parent tree
            self.update_entry(&entry)?;
            // Send StackTreeDMsg
            let mask = Mask::new(port_number);
            let in_reply_to = msg.get_sender_msg_seq_no();
            let new_msg = StackTreeDMsg::new(in_reply_to, sender_id, new_port_tree_id);
            self.send_msg(self.get_connected_tree_id(), &new_msg, mask)?;
            let parent_tree_id = payload.get_parent_port_tree_id();
            let base_tree_id = self.get_base_tree_id(parent_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            self.update_base_tree_map(parent_tree_id, base_tree_id);
            let save = self.gvm_eval_save(parent_tree_id, gvm_eqn).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
            if save { self.add_saved_stack_tree(parent_tree_id, msg); }
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_tree_msg" };
                    let trace = json!({ "cell_id": &self.cell_id, "new_port_tree_id": new_port_tree_id, "port_no": port_no, "msg": msg.value() });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                }
                if DEBUG_OPTIONS.all || DEBUG_OPTIONS.stack_tree {
                    println!("Cellagent {}: {} tree {} save {} port {} msg {}", self.cell_id, _f, msg_port_tree_id, save, *port_no, msg);
                }
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())

    }
    pub fn process_stack_tree_d_msg(&mut self, msg: &StackTreeDMsg, port_no: PortNo) -> Result<(), Error> {
        let _f = "process_stack_treed_msg";
        let port_number = port_no.make_port_number(self.no_ports)?;
        let port_tree_id = msg.get_port_tree_id();
        let tree_uuid = port_tree_id.get_uuid();
        let traph = self.get_traph_mut(port_tree_id)?;
        let mut entry = traph.get_tree_entry(&tree_uuid).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let user_mask = Mask::new(port_number);
        let mask = entry.get_mask().or(user_mask);
        entry.set_mask(mask);
        traph.set_tree_entry(&tree_uuid, entry)?;
        self.update_entry(&entry).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_process_stack_tree_d_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": msg, "port": port_no });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    fn find_new_parent(&mut self, header: &MsgHeader, payload: &FailoverMsgPayload, port_no: PortNo)
            -> Result<(), Error> {
        let _f = "find_new_parent";
        let sender_id = header.get_sender_id();
        let rw_port_tree_id = payload.get_rw_port_tree_id();
        let lw_port_tree_id = payload.get_lw_port_tree_id();
        let broken_path = payload.get_broken_path();
        let broken_tree_ids = payload.get_broken_port_tree_ids();
        self.failover_reply_ports.insert(rw_port_tree_id, port_no);
        let port_number = port_no.make_port_number(self.no_ports)?;
        let rw_traph = self.get_traph_mut(rw_port_tree_id)?;
        rw_traph.add_tried_port(rw_port_tree_id, port_no);
        match rw_traph.find_new_parent_port(rw_port_tree_id, broken_path) {
            None => {
                rw_traph.clear_tried_ports(rw_port_tree_id);
                let mask = Mask::new(port_number);
                let in_reply_to = header.get_sender_msg_seq_no();
                let broken_port_number = broken_path.get_port_no().as_usize();
                let no_packets = self.no_packets[broken_port_number];
                let failover_d_msg = FailoverDMsg::new(in_reply_to, sender_id,
                                                       FailoverResponse::Failure,
                                                       no_packets, payload);
                self.send_msg(self.connected_tree_id, &failover_d_msg, mask)?;
            },
            Some(trial_port_no) => {
                let failover_msg = FailoverMsg::new(sender_id, rw_port_tree_id, lw_port_tree_id,
                                                    broken_path, &broken_tree_ids);
                let mask = Mask::new(trial_port_no.make_port_number(self.no_ports)?);
                self.send_msg(self.connected_tree_id, &failover_msg, mask)?;
            }
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    fn repair_traph(&mut self, broken_port_tree_ids: &HashSet<PortTreeID>, port_number: PortNumber) -> Result<(), Error> {
        for broken_port_tree_id in broken_port_tree_ids {
            let traph = self.get_traph_mut(*broken_port_tree_id)?;
            let parent_entries = traph.set_parent(port_number, *broken_port_tree_id)?;
            let child_entries = traph.add_child(*broken_port_tree_id, port_number)?;
            self.update_entries(&parent_entries)?;
            self.update_entries(&child_entries)?;
        }
        Ok(())
    }
    fn may_send(&self, port_tree_id: PortTreeID) -> Result<bool, Error> {
        let entry = self.get_tree_entry(port_tree_id)?;
        Ok(entry.may_send())
    }
    fn app_interapplication(&mut self, sender_id: SenderID, _is_ait: bool, allowed_tree: &AllowedTree, serialized: &str,
                       direction: AppMsgDirection, tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_interapplication";
        let tree_id = tree_map
            .get(allowed_tree.get_name())
            .ok_or::<Error>(CellagentError::TreeMap { func_name: _f, cell_id: self.cell_id, tree_name: allowed_tree.clone() }.into())?;
        let port_tree_id = tree_id.to_port_tree_id_0();
        if !self.may_send(port_tree_id)? { return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id: *tree_id }.into()); }
        let msg = InterapplicationMsg::new(sender_id, false, *tree_id, MsgDirection::from_app(direction), serialized);
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_app_interapplication_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.process_msg {   // Debug
                println!("Cellagent {}: {} sending on tree {} interapplication msg {}", self.cell_id, _f, tree_id, msg);
            }
        }
        self.send_msg(*tree_id, &msg, DEFAULT_USER_MASK)?;
        Ok(tree_map.clone())
    }
    fn app_delete_tree(&self, _sender_id: SenderID, _serialized: &str, _direction: AppMsgDirection,
                        _tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_delete_tree";
        // Needs may_send test
        Err(UtilityError::Unimplemented { func_name: _f, feature: S("AppMsgType::Interapplication")}.into())
    }
    fn app_manifest(&mut self, sender_id: SenderID, serialized: &str, _direction: AppMsgDirection,
                    tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_manifest";
        let app_msg = &mut serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " deserialize StackTree" })?;
        let deploy_tree_name = &self.get_msg_params(app_msg, "deploy_tree_name").context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " parent tree name" })?;
        let deploy_tree_id = tree_map
            .get(AllowedTree::new(deploy_tree_name).get_name())
            .ok_or::<Error>(CellagentError::TreeMap { func_name: "listen_cm_loop 4", cell_id: self.cell_id, tree_name: AllowedTree::new(deploy_tree_name) }.into())?;
        let deploy_port_tree_id = deploy_tree_id.to_port_tree_id_0();
        if !self.may_send(deploy_port_tree_id)? { return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id: *deploy_tree_id }.into()); }
        let manifest_ser = self.get_msg_params(app_msg, "manifest").context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " manifest" })?;
        let manifest = serde_json::from_str::<Manifest>(&manifest_ser)?;
        let allowed_trees = manifest.get_allowed_trees().clone();
        let mut msg_tree_map = HashMap::new();
        for allowed_tree in allowed_trees {
            tree_map
                .get(allowed_tree.get_name())
                .map(|tree_id| msg_tree_map.insert(S(allowed_tree.get_name()), *tree_id))
                .ok_or::<Error>(CellagentError::TreeMap { func_name: "listen_cm_loop 5", cell_id: self.cell_id, tree_name: allowed_tree.clone() }.into())?;
        }
        let msg = ManifestMsg::new(sender_id, false, deploy_tree_id.clone(), &msg_tree_map, &manifest);
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_manifest_app_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "deploy_tree_id": deploy_tree_id, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.process_msg {   // Debug
                println!("Cellagent {}: {} sending on tree {} manifest app_msg {}", self.cell_id, _f, deploy_tree_id, msg);
            }
        }
        let mask = self.get_mask(deploy_port_tree_id)?;
        self.send_msg(*deploy_tree_id, &msg, mask.or(Mask::port0())).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " send manifest" })?;
        Ok(tree_map.clone())
    }
    fn app_query(&self, _sender_id: SenderID, _serialized: &str, _direction: AppMsgDirection,
                 _tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_query";
        // Needs may_send test
        Err(UtilityError::Unimplemented { func_name: _f, feature: S("AppMsgType::Interapplication")}.into())
    }
    fn app_stack_tree(&mut self, sender_id: SenderID, serialized: &str, direction: AppMsgDirection,
                      tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_stack_tree";
        let app_msg = &mut serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " deserialize StackTree" })?;
        let parent_tree_str = self.get_msg_params(app_msg, "parent_tree_name")?;
        let parent_tree_name = AllowedTree::new(&parent_tree_str);
        let parent_tree_id = *tree_map
            .get(parent_tree_name.get_name())
            .ok_or::<Error>(CellagentError::TreeMap { func_name: _f, cell_id: self.cell_id, tree_name: parent_tree_name }.into())?;
        let parent_port_tree_id = parent_tree_id.to_port_tree_id_0();
        if !self.may_send(parent_port_tree_id)? { return Err(CellagentError::MayNotSend { func_name: _f, cell_id: self.cell_id, tree_id: parent_tree_id }.into()); }
        let new_tree_name = self.get_msg_params(app_msg, "new_tree_name")?;
        let allowed_tree = AllowedTree::new(&new_tree_name);
        let new_tree_id = self.my_tree_id.add_component(&new_tree_name).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " new_tree_id" })?;
        let gvm_eqn_serialized = self.get_msg_params(app_msg, "gvm_eqn")?;
        let gvm_eqn = &serde_json::from_str::<GvmEquation>(&gvm_eqn_serialized).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) + " gvm" })?;
        // There is no port_tree for up trees
        let entry = self.stack_tree(sender_id, &allowed_tree, new_tree_id.to_port_tree_id_0(),
                                    parent_tree_id.to_port_tree_id_0(),
                                    None, gvm_eqn)?
            .ok_or::<Error>( CellagentError::StackTree { func_name: _f, cell_id: self.cell_id, tree_id: new_tree_id.to_port_tree_id_0() }.into())?;
        let allowed_tree = AllowedTree::new(&new_tree_name);
        let stack_tree_msg = StackTreeMsg::new(sender_id, &allowed_tree, new_tree_id, parent_tree_id, MsgDirection::from_app(direction), gvm_eqn);
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_got_stack_tree_app_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "new_tree_id": new_tree_id, "entry": entry, "msg": stack_tree_msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.process_msg {
                println!("Cellagent {}: {} sending on tree {} manifest app_msg {}", self.cell_id, _f, new_tree_id, stack_tree_msg);
                println!("Cellagent {}: new tree id {} entry {}", self.cell_id, new_tree_id, entry);
            }
        }
        let mut tree_map_clone = tree_map.clone();
        tree_map_clone.insert(S(AllowedTree::new(&new_tree_name).get_name()), new_tree_id);
        let parent_entry = self.get_tree_entry(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("get parent_entry") })?;
        let parent_mask = parent_entry.get_mask().and(DEFAULT_USER_MASK);  // Excludes port 0
        let traph = self.get_traph(parent_port_tree_id).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
        let variables = traph.get_params(gvm_eqn.get_variables())?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&variables)?;
        if gvm_xtnd {
            self.send_msg(self.connected_tree_id, &stack_tree_msg, parent_mask)?;
            self.add_saved_stack_tree(self.my_tree_id.to_port_tree_id_0(), &stack_tree_msg);
        }
        Ok(tree_map_clone)
    }
    fn app_tree_name(&self, _sender_id: SenderID, _serialized: &str, _direction: AppMsgDirection,
                     _tree_map: &MsgTreeMap) -> Result<MsgTreeMap, Error> {
        let _f = "app_tree_name";
        Err(CellagentError::AppMessageType { func_name: _f, cell_id: self.cell_id, msg: AppMsgType::TreeName}.into())
    }
    fn get_msg_params(&self, msg: &mut HashMap<String, String>, param: &str) -> Result<String, Error> {
        let msg_err_msg = msg.clone();
        msg
            .get(param)
            .map(|param| S(param))
            .ok_or::<Error>(CellagentError::Message { func_name: "get_param", cell_id: self.cell_id, msg: msg_err_msg.clone() }.into())
    }
    fn gvm_eval_save(&self, port_tree_id: PortTreeID, gvm_eqn: &GvmEquation) -> Result<bool, Error> {
        let _f = "gvm_eval_save";
        // True if I should save this message for children that join this tree later
        // TODO: Add test to see if all child ports on the parent tree have responded, in which case I can delete saved msgs
        self.get_traph(port_tree_id)
            .map(|traph|{
                let params = traph.get_params(gvm_eqn.get_variables())?;
                let save = gvm_eqn.eval_save(&params)?;
                let xtnd = gvm_eqn.eval_xtnd(&params)?;
                Ok(save && xtnd)
            })
            .unwrap_or(Ok(false))
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
    fn port_connected(&mut self, port_no: PortNo, is_border: bool) -> Result<(), Error> {
        let _f = "port_connected";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
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
            let base_tree = AllowedTree::new("Base");
            let my_tree_id = self.my_tree_id;
            let sender_id = SenderID::new(self.cell_id, &format!("BorderPort+{}", *port_no))?;
            self.add_tree_name_map_item(sender_id,&base_tree, my_tree_id);
            self.border_port_tree_id_map.insert(port_number, (sender_id, new_tree_id));
            let tree_name_msg = AppTreeNameMsg::new(sender_id, &base_tree.get_name());
            let serialized = serde_json::to_string(&tree_name_msg).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) })?;
            let bytes = ByteArray(serialized.into_bytes());
            self.ca_to_cm.send(CaToCmBytes::App((port_number, (false, base_tree, AppMsgType::TreeName, AppMsgDirection::Rootward, bytes)))).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id) + "border" })?;
            Ok(())
        } else {
            let sender_id = SenderID::new(self.cell_id, "CellAgent")?;
            let port_no_mask = Mask::new(port_number);
            self.connected_tree_entry.or_with_mask(port_no_mask); // Add to connected ports
            self.update_entry(&self.connected_tree_entry)?;
            let hello_msg = HelloMsg::new(sender_id, self.cell_id, port_no);
            self.send_msg(self.connected_tree_id, &hello_msg, port_no_mask)?;
            let path = Path::new(port_no, self.no_ports)?;
            let hops = PathLength(CellQty(1));
            let my_port_tree_id = self.my_tree_id.to_port_tree_id(port_number);
            self.update_base_tree_map(my_port_tree_id, self.my_tree_id);
            let discover_msg = DiscoverMsg::new(sender_id, my_port_tree_id,
                                                self.cell_id, hops, path);
            self.send_msg(self.connected_tree_id, &discover_msg, port_no_mask).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
            self.forward_discover(port_no_mask).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
            Ok(())
        }
    }
    fn port_disconnected(&mut self, port_no: PortNo, no_packets: NumberOfPackets) -> Result<(), Error> {
        let _f = "port_disconnected";
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
            let sender_id = SenderID::new(self.cell_id, "CellAgent")?;
            let rootward_tree_id = rw_traph.get_base_tree_id();
            let rw_port_number = broken_path.get_port_number();
            let rw_port_tree_id = rootward_tree_id.to_port_tree_id(rw_port_number);
            let lw_port_tree_id = self.my_tree_id.to_port_tree_id(port_number);
            let port_number = trial_parent_port.make_port_number(self.no_ports)?;
            let mask = Mask::new(port_number);
            let failover_msg = FailoverMsg::new(sender_id, rw_port_tree_id, lw_port_tree_id,
                                                broken_path, &broken_port_tree_ids);
            println!("Cellagent {}: {} candidate parent for tree {} is port {}", self.cell_id, _f, rw_traph.get_base_tree_id(), *trial_parent_port);
            self.send_msg(self.connected_tree_id, &failover_msg, mask).context(CellagentError::Chain { func_name: _f, comment: S(self.cell_id) })?;
        } else {
            println!("Cellagent {}: {} no candidate parent found for tree {}", self.cell_id, _f, rw_traph.get_base_tree_id())
        }
        (*self.traphs_mutex.lock().unwrap()) = self.traphs.clone();
        Ok(())
    }
    fn forward_discover(&self, mask: Mask) -> Result<(), Error> {
        for msg in self.get_saved_discover().iter() {
            self.send_msg(self.connected_tree_id, msg, mask)?;
        }
        Ok(())
    }
    fn forward_stacked_trees(&mut self, port_tree_id: PortTreeID, mask: Mask) -> Result<(), Error> {
        let _f = "forward_stacked_trees";
        // Forward all saved StackTreeMsg of trees stacked on this one
        let traph = self.get_traph(port_tree_id)
            .context(CellagentError::Chain { func_name: _f, comment: S("") })?
            .clone();
        let trees = traph.get_stacked_trees();
        let locked = trees.lock().unwrap();
        for tree in locked.values() {
            self.forward_stack_tree(tree.get_port_tree_id(), mask).context(CellagentError::Chain { func_name: _f, comment: S("") })?; // Forward stack tree messages on tree
            let stacked_tree_ids = tree.get_stacked_tree_ids();
            // Forward stack tree messages on trees stacked on tree
            for port_tree_id in stacked_tree_ids.iter() {
                self.forward_stack_tree(*port_tree_id, mask).context(CellagentError::Chain { func_name: _f, comment: S("") })?;
            }
        }
        Ok(())
    }
    fn forward_stack_tree(&mut self, port_tree_id: PortTreeID, mask: Mask)
            -> Result<(), Error> {
        let _f = "forward_stack_tree";
        self.saved_stack
            .get(&port_tree_id.to_tree_id())
            .map(|saved| -> Result<(), Error> {
                for msg in saved.iter() {
                    {
                        let msg_type = msg.get_msg_type();
                        if TRACE_OPTIONS.all || TRACE_OPTIONS.ca {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_forward_stack_tree_msg" };
                            let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": &port_tree_id, "port_nos": &mask.get_port_nos(), "msg_type": &msg_type });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                        }
                        if DEBUG_OPTIONS.all || DEBUG_OPTIONS.saved_msgs {
                            println!("CellAgent {}: {} tree on ports {:?} {}", self.cell_id, _f, mask.get_port_nos(), msg_type);
                        }
                    }
                    self.send_msg(self.connected_tree_id, msg,mask)?;
                }
                Ok(())
            });
        Ok(())
    }
    fn send_msg<T: Message>(&self, tree_id: TreeID, msg: &T, user_mask: Mask) -> Result<(), Error>
        where T: Message + ::std::marker::Sized + serde::Serialize + fmt::Display
    {
        let _f = "send_msg";
        {
            let mask = self.get_mask(tree_id.to_port_tree_id_0())?;
            let port_mask = user_mask.and(mask);
            let ports = Mask::get_port_nos(port_mask);
            let msg_type = msg.get_msg_type();
            if TRACE_OPTIONS.all || TRACE_OPTIONS.ca || DEBUG_OPTIONS.all || DEBUG_OPTIONS.ca_msg_send {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "ca_send_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "port_nos": &ports, "msg": msg.value() });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
            }
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.ca_msg_send {
                match msg_type {
                    MsgType::Discover => (),
                    MsgType::DiscoverD => println!("Cellagent {}: {} send on ports {:?} msg {}", self.cell_id, _f, ports, msg),
                    _ => {
                        println!("Cellagent {}: {} send on ports {:?} msg {}", self.cell_id, _f, ports, msg)
                    }
                }
            }
        }
        let bytes = msg.to_bytes()?;
        self.send_bytes(tree_id, msg.is_ait(), user_mask, bytes)?;
        Ok(())
    }
    fn send_bytes(&self, tree_id: TreeID, is_ait: bool, user_mask: Mask,
                  bytes: ByteArray) -> Result<(), Error> {
        let _f = "send_bytes";
        let tree_uuid = tree_id.get_uuid();
        // Make sure tree_id is legit
        self.tree_map
            .get(&tree_uuid)
            .ok_or::<Error>(CellagentError::Tree { func_name: _f, cell_id: self.cell_id, tree_uuid }.into())?;
        let msg = CaToCmBytes::Bytes((tree_id, is_ait, user_mask, bytes));
        self.ca_to_cm.send(msg)?;
        Ok(())
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
    #[fail(display = "CellagentError::MayNotSend {}: Cell {} does not have permission to send on tree {}", func_name, cell_id, tree_id)]
    MayNotSend { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
    #[fail(display = "CellagentError::Message {}: Malformed request {:?} from border port on cell {}", func_name, msg, cell_id)]
    Message { func_name: &'static str, cell_id: CellID, msg: HashMap<String, String> },
//    #[fail(display = "CellAgentError::NoParentTraph {}: No one hop parent for port {} on cell {}", func_name, port_no, cell_id)]
//    NoParentTraph { cell_id: CellID, func_name: &'static str, port_no: u8 },
    #[fail(display = "CellAgentError::NoTraph {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_id, cell_id)]
    NoTraph { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
//    #[fail(display = "CellagentError::SavedMsgType {}: Message type {} does not support saving", func_name, msg_type)]
//    SavedMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "CellAgentError::Partition {}: No path from {} to {}", func_name, lw_tree_id, rw_tree_id)]
    Partition { func_name: &'static str, lw_tree_id: TreeID, rw_tree_id: TreeID },
    #[fail(display = "CellAgentError::StackTree {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)]
    StackTree { func_name: &'static str, tree_id: PortTreeID, cell_id: CellID },
//    #[fail(display = "CellAgentError::TenantMask {}: Cell {} has no tenant mask", func_name, cell_id)]
//    TenantMask { func_name: &'static str, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeNameMap {}: Cell {} has no tree name map entry for {:?}", func_name, cell_id, sender_id)]
    TreeNameMap { func_name: &'static str, cell_id: CellID, sender_id: SenderID },
    #[fail(display = "CellAgentError::TreeMap {}: Cell {} has no tree map entry for {}", func_name, cell_id, tree_name)]
    TreeMap { func_name: &'static str, cell_id: CellID, tree_name: AllowedTree },
    #[fail(display = "CellAgentError::Tree {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid },
//    #[fail(display = "CellAgentError::TreeUuid {}: No tree associated with uuid {:?} on cell {}", func_name, uuid, cell_id)]
//    TreeUuid { func_name: &'static str, uuid: Uuid, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeVmMap {} Cell {} has no tree map entry for {}", func_name, cell_id, tree_id)]
    TreeVmMap { func_name: &'static str, cell_id: CellID, tree_id: TreeID }
}
