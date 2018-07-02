use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;

use serde;
use serde_json;

use config::{CONNECTED_PORTS_TREE_NAME, CONTROL_TREE_NAME, MAX_ENTRIES, MAX_PORTS,
             ByteArray, CellNo, CellType, DEBUG_OPTIONS, PathLength, PortNo, TableIndex};
use dal;
use gvm_equation::{GvmEquation, GvmEqn};
use message::{Message, MsgDirection, MsgTreeMap, MsgType, TcpMsgType, ApplicationMsg,
              DiscoverMsg, DiscoverDMsg, ManifestMsg, StackTreeDMsg, StackTreeMsg,
              TreeNameMsg};
use message_types::{CaToCm, CaFromCm,
                    CaToVm, VmFromCa, VmToCa, CaFromVm,
                    CaToCmBytes, CmToCaBytes};
use nalcell::CellConfig;
use name::{Name, CellID, SenderID, TreeID, UptreeID, VmID};
use port;
use routing_table_entry::{RoutingTableEntry};
use traph;
use traph::{Traph};
use tree::Tree;
use uptree_spec::{AllowedTree, Manifest};
use utility::{BASE_TENANT_MASK, DEFAULT_USER_MASK, Mask, Path,
              PortNumber, S, TraceHeader, TraceHeaderParams, TraceType, UtilityError};
//use uuid::Uuid;
use uuid_fake::Uuid;
use vm::VirtualMachine;

use failure::{Error, ResultExt};

type BorderTreeIDMap = HashMap<PortNumber, (SenderID, TreeID)>;

pub type SavedDiscover = DiscoverMsg;
// The following is a hack, because I can't get thread::spawn to accept Box<Message>
pub type SavedMsg = (Option<ApplicationMsg>, Option<ManifestMsg>);
pub type SavedStack = StackTreeMsg;
pub type SavedMsgs = HashMap<TreeID, Vec<SavedMsg>>;
pub type SavedStackMsgs = HashMap<TreeID, Vec<SavedStack>>;
pub type Traphs = HashMap<Uuid, Traph>;
pub type Trees = HashMap<TableIndex, TreeID>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeIDMap = HashMap<Uuid, TreeID>;
pub type TreeNameMap = HashMap<SenderID, MsgTreeMap>;
pub type TreeVmMap = HashMap<TreeID, Vec<CaToVm>>;

const MODULE: &'static str = "cellagent.rs";
#[derive(Debug, Clone)]
pub struct CellAgent {
    cell_id: CellID,
    cell_type: CellType,
    config: CellConfig,
    no_ports: PortNo,
    my_tree_id: TreeID,
    control_tree_id: TreeID,
    connected_tree_id: TreeID,
    my_entry: RoutingTableEntry,
    connected_tree_entry: Arc<Mutex<RoutingTableEntry>>,
    saved_discover: Arc<Mutex<Vec<SavedDiscover>>>,
    saved_msgs: Arc<Mutex<SavedMsgs>>,
    saved_stack: Arc<Mutex<SavedStackMsgs>>,
    free_indices: Arc<Mutex<Vec<TableIndex>>>,
    trees: Arc<Mutex<Trees>>,
    traphs: Arc<Mutex<Traphs>>,
    tree_map: Arc<Mutex<TreeMap>>, // Base tree for given stacked tree
    tree_name_map: TreeNameMap,
    border_port_tree_id_map: BorderTreeIDMap, // Find the tree id associated with a border port
    base_tree_map: HashMap<TreeID, TreeID>, // Find the black tree associated with any tree, needed for stacking
    tree_id_map: Arc<Mutex<TreeIDMap>>, // For debugging
    tenant_masks: Vec<Mask>,
    tree_vm_map: TreeVmMap,
    ca_to_vms: HashMap<VmID, CaToVm>,
    ca_to_cm: CaToCm,
    vm_id_no: usize,
    up_tree_senders: HashMap<UptreeID, HashMap<String,TreeID>>,
    up_traphs_clist: HashMap<TreeID, TreeID>,
}
impl CellAgent {
    pub fn new(cell_id: &CellID, cell_type: CellType, config: CellConfig, no_ports: PortNo,
               ca_to_cm: CaToCm )
               -> Result<CellAgent, Error> {
        let tenant_masks = vec![BASE_TENANT_MASK];
        let my_tree_id = TreeID::new(cell_id.get_name())?;
        let control_tree_id = TreeID::new(cell_id.get_name())?.add_component(CONTROL_TREE_NAME)?;
        let connected_tree_id = TreeID::new(cell_id.get_name())?.add_component(CONNECTED_PORTS_TREE_NAME)?;
        let mut free_indices = Vec::new();
        let trees = HashMap::new(); // For getting TreeID from table index
        for i in 0..(*MAX_ENTRIES) {
            free_indices.push(TableIndex(i)); // O reserved for control tree, 1 for connected tree
        }
        free_indices.reverse();
        let mut base_tree_map = HashMap::new();
        base_tree_map.insert(my_tree_id.clone(), my_tree_id.clone());
        let traphs = Arc::new(Mutex::new(HashMap::new()));
        Ok(CellAgent { cell_id: cell_id.clone(), my_tree_id, cell_type, config,
            control_tree_id, connected_tree_id,	tree_vm_map: HashMap::new(), ca_to_vms: HashMap::new(),
            no_ports, traphs, vm_id_no: 0, tree_id_map: Arc::new(Mutex::new(HashMap::new())),
            free_indices: Arc::new(Mutex::new(free_indices)), tree_map: Arc::new(Mutex::new(HashMap::new())),
            tree_name_map: HashMap::new(), border_port_tree_id_map: HashMap::new(),
            saved_msgs: Arc::new(Mutex::new(HashMap::new())), saved_discover: Arc::new(Mutex::new(Vec::new())),
            saved_stack: Arc::new(Mutex::new(HashMap::new())),
            my_entry: RoutingTableEntry::default(TableIndex(0))?, base_tree_map,
            connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(TableIndex(0))?)),
            tenant_masks, trees: Arc::new(Mutex::new(trees)), up_tree_senders: HashMap::new(),
            up_traphs_clist: HashMap::new(), ca_to_cm,
        })
    }
    pub fn initialize(&mut self, ca_from_cm: CaFromCm, mut trace_header: TraceHeader) -> Result<(), Error> {
        // Set up predefined trees - Must be first two in this order
        let port_number_0 = PortNumber::new(PortNo{v:0}, self.no_ports).unwrap(); // No error possible for port 0
        let other_index = TableIndex(0);
        let hops = PathLength(CellNo(0));
        let path = None;
        let control_tree_id = self.control_tree_id.clone();
        let connected_tree_id = self.connected_tree_id.clone();
        let my_tree_id = self.my_tree_id.clone();
        {
            let mut locked = self.tree_map.lock().unwrap();
            locked.insert(control_tree_id.get_uuid(), control_tree_id.get_uuid());
            locked.insert(connected_tree_id.get_uuid(), connected_tree_id.get_uuid());
            locked.insert(my_tree_id.get_uuid(), my_tree_id.get_uuid());
        }
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(eqns, Vec::new());
        self.update_traph(&control_tree_id, port_number_0,
                          traph::PortStatus::Parent, &gvm_equation,
                          &mut HashSet::new(), other_index, hops, path, &mut trace_header)?;
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("false"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(eqns, Vec::new());
        let connected_tree_entry = self.update_traph(&connected_tree_id, port_number_0,
                                                     traph::PortStatus::Parent, &gvm_equation,
                                                     &mut HashSet::new(), other_index, hops, path, &mut trace_header)?;
        self.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
        // Create my tree
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_equation = GvmEquation::new(eqns, Vec::new());
        self.my_entry = self.update_traph(&my_tree_id, port_number_0,
                                          traph::PortStatus::Parent, &gvm_equation,
                                          &mut HashSet::new(), other_index, hops, path, &mut trace_header)?;
        self.listen_cm(ca_from_cm, &mut trace_header.fork_trace())?;
        Ok(())
    }
    pub fn get_no_ports(&self) -> PortNo { self.no_ports }
    pub fn get_id(&self) -> CellID { self.cell_id.clone() }
//    pub fn get_traphs(&self) -> &Arc<Mutex<Traphs>> { &self.traphs }
//    pub fn get_tree_name_map(&self) -> &TreeNameMap { &self.tree_name_map }
    pub fn get_vm_senders(&self, tree_id: &TreeID) -> Result<Vec<CaToVm>, Error> {
        let f = "get_vm_senders";
        let senders = match self.tree_vm_map.get(tree_id).cloned() {
            Some(senders) => senders,
            None => return Err(CellagentError::Tree { func_name: f, cell_id: self.cell_id.clone(), tree_uuid: tree_id.get_uuid() }.into())
        };
        Ok(senders)
    }
    /*
        pub fn get_tree_id(&self, TableIndex(index): TableIndex) -> Result<TreeID, CellagentError> {
            let f = "get_tree_id";
            let trees = self.trees.lock().unwrap();
            match trees.get(&TableIndex(index)) {
                Some(t) => Ok(t.clone()),
                None => Err(CellagentError::TreeIndex { cell_id: self.cell_id.clone(), func_name: f, index: TableIndex(index) })
            }
        }
        pub fn get_hops(&self, tree_id: &TreeID) -> Result<PathLength, Error> {
            let f = "get_hops";
            if let Some(traph) = self.traphs.lock().unwrap().get(&tree_id.get_uuid()) {
                Ok(traph.get_hops()?)
            } else {
                Err(CellagentError::Tree { cell_id: self.cell_id.clone(), func_name: f, tree_uuid: tree_id.get_uuid() }.into())
            }
        }
    */
    fn get_mask(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<Mask, Error> {
        Ok(match self.get_traph(tree_id, trace_header) {
            Ok(t) => t.get_tree_entry(&tree_id.get_uuid())?.get_mask(),
            Err(_) => Mask::empty().not()
        })
    }
    fn get_gvm_eqn(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<GvmEquation, Error> {
        let f = "get_gvm_eqn";
        let tree_uuid = tree_id.get_uuid();
        let traph = self.get_traph(tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("") })?;
        let tree = traph.get_tree(&tree_uuid)?;
        let gvm_eqn = tree.get_gvm_eqn().clone();
        Ok(gvm_eqn.clone())
    }
    pub fn get_saved_discover(&self) -> Vec<SavedDiscover> {
        self.saved_discover.lock().unwrap().to_vec()
    }
    pub fn get_saved_stack_tree(&self, tree_id: &TreeID) -> Vec<SavedStack> {
        let locked = self.saved_stack.lock().unwrap();
        match locked.get(tree_id).cloned() {
            Some(msgs) => msgs,
            None => Vec::new()
        }
    }
    pub fn get_saved_msgs(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Vec<SavedMsg> {
        let locked = self.saved_msgs.lock().unwrap();
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
            let f = "get_saved_msgs";
            {
                let saved_msgs = match locked.get(tree_id) {
                    Some(msgs) => msgs.clone(),
                    None => Vec::new()
                };
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_get_saved_msgs" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "no_saved_msgs": saved_msgs.len() });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.saved_msgs { println!("Cellagent {}: {} for tree {} {}", self.cell_id, f, tree_id, saved_msgs.len()); }
            }
        }
        match locked.get(tree_id) {
            Some(msgs) => msgs.clone(),
            None => Vec::new()
        }
    }
    // The options argument is a hack, because I can't get thread::spawn to accept Box<Message>
    pub fn add_saved_msg(&mut self, tree_id: &TreeID, _: Mask, options: (Option<ApplicationMsg>, Option<ManifestMsg>),
                         trace_header: &mut TraceHeader) -> Result<(), Error> {
        let empty = Vec::new();
        let mut locked = self.saved_msgs.lock().unwrap();
        let saved = {
            let mut saved_msgs = locked.remove(tree_id).unwrap_or(empty);
            saved_msgs.push(options.clone());
            saved_msgs
        };
        locked.insert(tree_id.clone(), saved);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
            let f = "add_saved_msg";
            let saved_msgs = match locked.get(tree_id) {
                Some(msgs) => msgs.clone(),
                None => Vec::new()
            };
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_add_saved_msg" };
            let trace = json! ({ "cell_id": &self.cell_id, "tree_id": tree_id, "no_saved": saved_msgs.len(), "msg": &options });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.saved_msgs { println!("Cellagent {}: {} saved {} for tree {} msg {:?}", self.cell_id, f, saved_msgs.len(), tree_id, options); }
        }
        Ok(())
    }
    pub fn add_saved_stack_tree(&mut self, tree_id: &TreeID, stack_tree_msg: &SavedStack, trace_header: &mut TraceHeader) {
        let empty = &mut vec![];
        let mut locked = self.saved_stack.lock().unwrap();
        let saved = {
            let saved_msgs = locked.get_mut(tree_id).unwrap_or(empty);
            saved_msgs.push(stack_tree_msg.clone());
            saved_msgs.clone()
        };
        let saved_len = saved.len();
        locked.insert(tree_id.clone(), saved);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
            let f = "add_saved_stack_tree";
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_save_stack_tree_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "no_saved": saved_len, "msg": &stack_tree_msg });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.saved_msgs { println!("Cellagent {}: {} saving {} msg {}", self.cell_id, f, saved_len, stack_tree_msg); }
        }
    }
    pub fn add_saved_discover(&mut self, discover_msg: &SavedDiscover, trace_header: &mut TraceHeader) {
        let mut saved_discover = self.saved_discover.lock().unwrap();
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {    // Debug print
            let f = "add_saved_discover";
            let tree_id = discover_msg.get_tree_id().unwrap();
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_save_discover_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "msg": &discover_msg });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.saved_msgs { println!("Cell {}: save discover {}", self.cell_id, discover_msg); }
        }
        saved_discover.push(discover_msg.clone());
    }
    fn add_tree_name_map_item(&mut self, sender_id: &SenderID, allowed_tree: &AllowedTree, allowed_tree_id: &TreeID) {
        let mut tree_map = match self.tree_name_map.get(sender_id).cloned() {
            Some(map) => map,
            None => HashMap::new()
        };
        tree_map.insert(S(allowed_tree.get_name()), allowed_tree_id.clone());
        self.tree_name_map.insert(sender_id.clone(), tree_map);
    }
    /*
    pub fn get_tenant_mask(&self) -> Result<&Mask, CellagentError> {
        let f = "get_tenant_mask";
        if let Some(tenant_mask) = self.tenant_masks.last() {
            Ok(tenant_mask)
        } else {
            return Err(CellagentError::TenantMask { cell_id: self.get_id(), func_name: f } )
        }
    }
    */
    pub fn update_base_tree_map(&mut self, stacked_tree_id: &TreeID, base_tree_id: &TreeID,
            trace_header: &mut TraceHeader) {
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.traph_state {   // Debug print
            let f = "update_base_tree_map";
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_update_base_tree_map" };
            let trace = json!({ "cell_id": &self.cell_id, "stacked_tree_id": stacked_tree_id, "base_tree_id": base_tree_id, });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.traph_state { println!("Cellagent {}: {}: stacked tree {} {}, base tree {} {}", self.cell_id, f, stacked_tree_id, stacked_tree_id.get_uuid(), base_tree_id, base_tree_id.get_uuid()); }
        }
        self.base_tree_map.insert(stacked_tree_id.clone(), base_tree_id.clone());
        self.tree_map.lock().unwrap().insert(stacked_tree_id.get_uuid(), base_tree_id.get_uuid());
    }
    fn get_base_tree_id(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<TreeID, Error> {
        let f = "get_base_tree_id";
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.traph_state {   // Debug print
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_get_base_tree_id" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.traph_state { println!("Cell {}: {}: stacked tree {}", self.cell_id, f, tree_id); }
        }
        match self.base_tree_map.get(tree_id).cloned() {
            Some(id) => Ok(id),
            None => Err(CellagentError::BaseTree { func_name: f, cell_id: self.cell_id.clone(), tree_id: tree_id.clone() }.into())
        }
    }
    pub fn get_connected_ports_tree_id(&self) -> &TreeID { &self.connected_tree_id }
//    pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
    pub fn exists(&self, tree_id: &TreeID) -> bool {
        (*self.traphs.lock().unwrap()).contains_key(&tree_id.get_uuid())
    }
    /*
    fn is_on_tree(&self, tree_id: &TreeID) -> bool {
        let f = "is_on_tree";
        let traph = match self.get_traph(tree_id) {
            Ok(t) => t,
            Err(_) => return false,
        };
        let tree_entry = match traph.get_tree_entry(&tree_id.get_uuid()) {
            Ok(e) => e,
            Err(_) => return false
        };
        if tree_entry.get_mask().and(Mask::port0()).equal(Mask::port0()) {
            true
        } else {
            false
        }
    }
    */
    fn use_index(&mut self) -> Result<TableIndex, CellagentError> {
        let f = "use_index";
        match self.free_indices.lock().unwrap().pop() {
            Some(i) => Ok(i),
            None => Err(CellagentError::Size { cell_id: self.cell_id.clone(), func_name: f } )
        }
    }
//    fn free_index(&mut self, index: TableIndex) {
//        self.free_indices.lock().unwrap().push(index);
//    }
    pub fn update_traph(&mut self, base_tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
                        gvm_eqn: &GvmEquation, children: &mut HashSet<PortNumber>, other_index: TableIndex,
                        hops: PathLength, path: Option<Path>, trace_header: &mut TraceHeader)
                        -> Result<RoutingTableEntry, Error> {
        let f = "update_traph";
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.traph_state {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_update_traph" };
            let trace = json!({ "cell_id": &self.cell_id,
                "base_tree_id": base_tree_id, "port_number": &port_number, "hops": &hops,
                "other_index": other_index, "port_status": &port_status,
                "children": children, "gvm": &gvm_eqn });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        let (entry, _is_new_port) = {
            let mut traphs = self.traphs.lock().unwrap();
            let traph = match traphs.entry(base_tree_id.get_uuid()) { // Using entry voids lifetime problem
                Entry::Occupied(t) => t.into_mut(),
                Entry::Vacant(v) => {
                    //println!("Cell {} 1: update tree ID map {} {}", self.cell_id, base_tree_id, base_tree_id.get_uuid());
                    self.tree_id_map.lock().unwrap().insert(base_tree_id.get_uuid(), base_tree_id.clone());
                    let index = self.clone().use_index()?;
                    let t = Traph::new(&self.cell_id, &base_tree_id, index, gvm_eqn).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
                    v.insert(t)
                }
            };
            let (gvm_recv, gvm_send, _gvm_xtnd, _gvm_save) =  {
                    let variables = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
                    let recv = gvm_eqn.eval_recv(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_recv") })?;
                    let send = gvm_eqn.eval_send(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_send") })?;
                    let xtnd = gvm_eqn.eval_xtnd(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_xtnd") })?;
                    let save = gvm_eqn.eval_save(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_save") })?;
                    (recv, send, xtnd, save)
            };
            let (hops, path) = match port_status {
                traph::PortStatus::Child => {
                    let element = traph.get_parent_element().context(CellagentError::Chain { func_name: f, comment: S("") })?;
                    // Need to coordinate the following with DiscoverMsg.update_discover_msg
                    (element.hops_plus_one(), element.get_path())
                },
                _ => (hops, path)
            };
            let traph_status = traph.get_port_status(port_number);
            let port_status = match traph_status {
                traph::PortStatus::Pruned => port_status,
                _ => traph_status  // Don't replace if Parent or Child
            };
            if gvm_recv { children.insert(PortNumber::new0()); }
            let is_new_port =  !traph.is_port_connected(port_number);
            let mut entry = traph.new_element(base_tree_id, port_number, port_status, other_index, children, hops, path).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
            if gvm_send { entry.enable_send() } else { entry.disable_send() }
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.traph_state {
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_updated_traph_entry" };
                let trace = json!({ "cell_id": &self.cell_id, "base_tree_id": base_tree_id, "entry": &entry });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.traph_state { println!("CellAgent {}: entry {}", self.cell_id, entry); }
            }
            // Need traph even if cell only forwards on this tree
            self.trees.lock().unwrap().insert(entry.get_index(), base_tree_id.clone());
            self.ca_to_cm.send(CaToCmBytes::Entry(entry)).context(CellagentError::Chain { func_name: f, comment: S("") })?;
            // TODO: Need to update entries of stacked trees following a failover but not as base tree builds out
            //let entries = traph.update_stacked_entries(entry).context(CellagentError::Chain { func_name: f, comment: S("") })?;
            //for entry in entries {
                //println!("Cell {}: sending entry {}", self.cell_id, entry);
                //self.ca_to_cm.send(CaToPePacket::Entry(entry)).context(CellagentError::Chain { func_name: f, comment: S("") })?;
            //}
            (entry, is_new_port)
        };
        Ok(entry)
    }
    pub fn get_traph(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<Traph, Error> {
        let f = "get_traph";
        let base_tree_id = self.get_base_tree_id(tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("") })?;
        let mut locked = self.traphs.lock().unwrap();
        let uuid = base_tree_id.get_uuid();
        match locked.entry(uuid) {
            Entry::Occupied(o) => Ok(o.into_mut().clone()),
            Entry::Vacant(_) => Err(CellagentError::NoTraph { cell_id: self.cell_id.clone(), func_name: "stack_tree", tree_uuid: uuid }.into())
        }
    }
    pub fn deploy(&mut self, sender_id: &SenderID, deployment_tree_id: &TreeID, _msg_tree_id: &TreeID,
                  msg_tree_map: &MsgTreeMap, manifest: &Manifest, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "deploy";
        let mut tree_map = match self.tree_name_map.get(  sender_id).cloned() {
            Some(map) => map,
            None => return Err(CellagentError::TreeNameMap { cell_id: self.get_id(), func_name: f, sender_id: sender_id.clone() }.into())
        };
        for allowed_tree in manifest.get_allowed_trees() {
            match msg_tree_map.get(allowed_tree.get_name()) {
                Some(tree_id) => tree_map.insert(S(allowed_tree.get_name()), tree_id.clone()),
                None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: f, tree_name: allowed_tree.clone() }.into())
            };
        }
        for vm_spec in manifest.get_vms() {
            let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
            let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
            let vm_id = VmID::new(&self.cell_id, &vm_spec.get_id())?;
            let new_sender_id = SenderID::new(&self.cell_id, vm_id.get_name())?;
            let vm_allowed_trees = vm_spec.get_allowed_trees();
            let mut trees = HashSet::new();
            let mut tree_vm_map = self.tree_vm_map.clone();
            trees.insert(AllowedTree::new(CONTROL_TREE_NAME));
            for vm_allowed_tree in vm_allowed_trees {
                match tree_map.get(vm_allowed_tree.get_name()) {
                    Some(allowed_tree_id) => {
                        trees.insert(vm_allowed_tree.clone());
                        self.add_tree_name_map_item(&new_sender_id, vm_allowed_tree, &allowed_tree_id.clone());
                        //println!("Cellagent {}: Adding tree {} to vm map", self.cell_id, allowed_tree_id);
                        match tree_vm_map.clone().get_mut(allowed_tree_id) {
                            Some(senders) => senders.push(ca_to_vm.clone()),
                            None => { tree_vm_map.insert(allowed_tree_id.clone(), vec![ca_to_vm.clone()]); }
                        }
                    },
                    None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(vm)", tree_name: vm_allowed_tree.clone() }.into())
                }
            }
            self.tree_vm_map = tree_vm_map;
            //println!("Cellagent {}: added vm senders {:?}", self.cell_id, self.tree_vm_map.keys());
            let container_specs = vm_spec.get_containers();
            let mut vm = VirtualMachine::new(&vm_id, vm_to_ca, vm_allowed_trees);
            let up_tree_name = vm_spec.get_id();
            //println!("Cell {} starting VM on up tree {}", self.cell_id, up_tree_name);
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.deploy {
                let keys: Vec<TreeID> = self.tree_vm_map.iter().map(|(k,_)| k.clone()).collect();
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_deploy" };
                let trace = json!({ "cell_id": &self.cell_id,
                    "deployment_tree_id": deployment_tree_id, "tree_vm_map_keys":  &keys,
                    "up_tree_name": up_tree_name });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.deploy {
                    println!("Cellagent {}: deployment tree {}", self.cell_id, deployment_tree_id);
                    println!("Cellagent {}: added vm senders {:?}", self.cell_id, self.tree_vm_map.keys());
                    println!("Cellagent {}: starting VM on up tree {}", self.cell_id, up_tree_name);
                }
            }
            vm.initialize(up_tree_name, vm_from_ca, &trees, container_specs)?;
            let sender_id = SenderID::new(&self.cell_id, vm_id.get_name())?;
            self.ca_to_vms.insert(vm_id.clone(), ca_to_vm,);
            self.listen_uptree(sender_id, vm_id, trees, ca_from_vm, &mut trace_header.fork_trace());
        }
        Ok(())
    }
    /*
        fn stack_uptree(&mut self, up_tree_id: &TreeID, deployment_tree_id: &TreeID, port_no: PortNo, gvm_eqn: &GvmEquation) -> Result<(), Error> {
            let ref my_tree_id = self.my_tree_id.clone(); // Need to clone because self is borrowed mut
            let msg= StackTreeMsg::new(up_tree_id, &self.my_tree_id, gvm_eqn);
            let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "stack_uptree", comment: S(self.cell_id.clone())})?;
            let port_no_mask = Mask::all_but_zero(self.no_ports).and(Mask::new(port_number));
            self.send_msg(&deployment_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "stack_uptree", comment: S(self.cell_id.clone())})?;
            self.stack_tree(&up_tree_id, my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "stack_uptree", comment: S(self.cell_id.clone())})?;
            let mut tree_name_map = HashMap::new();
            tree_name_map.insert(AllowedTree::new(up_tree_id.get_name()),up_tree_id.clone());
            self.tree_name_map.insert(up_tree_id.clone(), tree_name_map);
            Ok(())
        }
    */
    fn listen_uptree(&self, sender_id: SenderID, vm_id: VmID, trees: HashSet<AllowedTree>,
                     ca_from_vm: CaFromVm, outer_trace_header: &mut TraceHeader) {
        let f = "listen_uptree";
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_listen_vm" };
            let trace = json!({ "cell_id": &self.cell_id, "vm_id": &vm_id.clone(), "sender_id": &sender_id.clone() });
            let _ = dal::add_to_trace(outer_trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        let mut ca = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = ca.listen_uptree_loop(&sender_id.clone(), &vm_id, &ca_from_vm, inner_trace_header).map_err(|e| ::utility::write_err("cellagent", e));
            let ref mut outer_trace_header = outer_trace_header_clone.fork_trace();
            let _ = ca.listen_uptree(sender_id, vm_id, trees, ca_from_vm, outer_trace_header);
        });
    }
    fn listen_uptree_loop(&mut self, sender_id: &SenderID, _vm_id: &VmID, ca_from_vm: &CaFromVm, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "listen_uptree_loop";
         loop {
             let tree_map = match self.tree_name_map.get(sender_id).cloned() {
                 Some(map) => map,
                 None => return Err(CellagentError::TreeNameMap { func_name: f, cell_id: self.cell_id.clone(), sender_id: sender_id.clone() }.into())
             };
             let (allowed_tree, msg_type, direction, bytes) = ca_from_vm.recv()?;
             let serialized = ::std::str::from_utf8(&bytes)?;
             if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.ca_msg_recv { // Debug print
                 let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_got_from_uptree" };
                 let trace = json!({ "cell_id": &self.cell_id,
                    "allowed_tree": &allowed_tree, "msg_type": &msg_type,
                    "direction": &direction, "tcp_msg": &serde_json::to_value(&serialized)? });
                 let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                 if DEBUG_OPTIONS.ca_msg_recv { println!("CellAgent {}: got msg {} {} {} {}", self.cell_id, allowed_tree, msg_type, direction, &serialized); }
             }
             let tree_map_updated = match msg_type {
                 TcpMsgType::Application => self.tcp_application(&sender_id, &allowed_tree, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_application") })?,
                 TcpMsgType::DeleteTree => self.tcp_delete_tree(&sender_id, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_delete_tree") })?,
                 TcpMsgType::Manifest => self.tcp_manifest(&sender_id, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_manifest") })?,
                 TcpMsgType::Query => self.tcp_query(&sender_id, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_query") })?,
                 TcpMsgType::StackTree => self.tcp_stack_tree(&sender_id, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_stack_tree") })?,
                 TcpMsgType::TreeName => self.tcp_tree_name(&sender_id, serialized, direction, &tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_tree_name") })?,
             };
             self.tree_name_map.insert(sender_id.clone(), tree_map_updated);
         }
    }
    /*
        fn create_tree(&mut self, id: &str, target_tree_id: &TreeID, port_no_mask: Mask, gvm_eqn: &GvmEquation)
                -> Result<(), Error> {
            let new_id = self.my_tree_id.add_component(id)?;
            let new_tree_id = TreeID::new(new_id.get_name())?;
            new_tree_id.add_to_trace()?;
            let ref my_tree_id = self.my_tree_id.clone(); // Need because self is borrowed mut
            let msg =  StackTreeMsg::new(&new_tree_id, &self.my_tree_id,&gvm_eqn);
            self.send_msg(target_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.cell_id.clone())})?;
            self.stack_tree(&new_tree_id, &my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.cell_id.clone())})?;
            Ok(())
        }
    */
    pub fn get_tree_entry(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<RoutingTableEntry, Error> {
        let traph = self.get_traph(&tree_id, trace_header)?;
        traph.get_tree_entry(&tree_id.get_uuid())
    }
    pub fn stack_tree(&mut self, sender_id: &SenderID, new_tree_id: &TreeID, parent_tree_id: &TreeID,
                      gvm_eqn: &GvmEquation, trace_header: &mut TraceHeader) -> Result<Option<RoutingTableEntry>, Error> {
        let f = "stack_tree";
        let base_tree_id = match self.get_base_tree_id(parent_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("") }) {
            Ok(id) => id,
            Err(_) => {
                self.update_base_tree_map(new_tree_id, parent_tree_id, trace_header);
                parent_tree_id.clone()
            }
        };
        self.add_tree_name_map_item( sender_id, &AllowedTree::new(new_tree_id.get_name()), new_tree_id);
        self.update_base_tree_map(new_tree_id, &base_tree_id, trace_header);
        let mut traph = self.get_traph(&parent_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        if traph.has_tree(new_tree_id) { return Ok(None); } // Check for redundant StackTreeMsg
        let parent_entry = self.get_tree_entry(&parent_tree_id, trace_header).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
        let index = self.use_index().context(CellagentError::Chain { func_name: f, comment: S("")})?;
        let mut entry = parent_entry.clone();
        entry.clear_other_indices();
        entry.set_mask(Mask::empty());
        entry.set_table_index(index);
        entry.set_uuid(&new_tree_id.get_uuid());
        entry.set_other_indices([TableIndex(0); MAX_PORTS.v as usize]);
        let params = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&params).context(CellagentError::Chain { func_name: f, comment: S("gvm_xtnd")})?;
        let gvm_send = gvm_eqn.eval_send(&params).context(CellagentError::Chain { func_name: f, comment: S("gvm_send")})?;
        if !gvm_xtnd { entry.clear_children(); }
        if gvm_send  { entry.enable_send(); } else { entry.disable_send(); }
        let gvm_recv = gvm_eqn.eval_recv(&params).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        let mask = if gvm_recv { entry.get_mask().or(Mask::port0()) }
            else        { entry.get_mask().and(Mask::all_but_zero(self.no_ports)) };
        entry.set_mask(mask);
        let tree = Tree::new(&new_tree_id, &base_tree_id, parent_tree_id, &gvm_eqn, entry);
        traph.stack_tree(tree);
        self.trees.lock().unwrap().insert(entry.get_index(), new_tree_id.clone());
        self.tree_map.lock().unwrap().insert(new_tree_id.get_uuid(), base_tree_id.get_uuid());
        self.tree_id_map.lock().unwrap().insert(new_tree_id.get_uuid(), new_tree_id.clone());
        self.update_entry(entry).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.stack_tree { // Debug print
            let keys: Vec<TreeID> = self.base_tree_map.iter().map(|(k,_)| k.clone()).collect();
            let values: Vec<TreeID> = self.base_tree_map.iter().map(|(_,v)| v.clone()).collect();
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_stack_tree" };
            let trace = json!({ "cell_id": &self.cell_id,
                "new_tree_id": &new_tree_id, "base_tree_id": &base_tree_id,
                "base_tree_map_keys": &keys, "base_tree_map_values": &values });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.stack_tree {
                println!("Cellagent {}: {} added new tree {} {} with base tree {} {}", self.cell_id, f, new_tree_id, new_tree_id.get_uuid(), base_tree_id, base_tree_id.get_uuid());
                println!("Cellagent {}: {} base tree map {:?}", self.cell_id, f, self.base_tree_map);
            }
        }
        Ok(Some(entry))
    }
    pub fn update_entry(&self, entry: RoutingTableEntry) -> Result<(), Error> {
        let f = "update_entry";
        self.ca_to_cm.send(CaToCmBytes::Entry(entry)).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        Ok(())
    }
    fn listen_cm(&mut self, ca_from_cm: CaFromCm, outer_trace_header: &mut TraceHeader) -> Result<(), Error>{
        let f = "listen_cm";
        let mut ca = self.clone();
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_listen_cm" };
            let trace = json!({ "cell_id": &self.cell_id });
            let _ = dal::add_to_trace(outer_trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        let mut outer_trace_header_clone = outer_trace_header.clone();
        thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = ca.listen_cm_loop(&ca_from_cm, inner_trace_header).map_err(|e| ::utility::write_err("cellagent", e));
            let ref mut outer_trace_header = outer_trace_header_clone.fork_trace();
            let _ = ca.listen_cm(ca_from_cm, outer_trace_header);
        });
        Ok(())
    }
    fn listen_cm_loop(&mut self, ca_from_cm: &CaFromCm, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "listen_cm_loop";
        loop {
            //println!("CellAgent {}: waiting for status or packet", ca.cell_id);
            match ca_from_cm.recv().context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})? {
                CmToCaBytes::Status((port_no, is_border, status)) => match status {
                    port::PortStatus::Connected => self.port_connected(port_no, is_border, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) +" port_connected"})?,
                    port::PortStatus::Disconnected => self.port_disconnected(port_no).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " port_disconnected"})?
                },
                CmToCaBytes::Bytes((port_no, index, bytes)) => {
                    // The index may be pointing to the control tree because the other cell didn't get the StackTree or StackTreeD message in time
                    let mut msg = MsgType::msg_from_bytes(&bytes).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
                        // Here's where I lock in the need for the tree_uuid in the packet header.  Can I avoid it?
                    let msg_tree_id = {
                        let locked = self.trees.lock().unwrap();
                        match locked.get(&index).cloned() {
                            Some(id) => id,
                            None => return Err(CellagentError::TreeIndex { func_name: f, cell_id: self.cell_id.clone(), index }.into())
                        }
                    };
                    if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.ca_msg_recv {   //Debug print
                        let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_got_msg" };
                        let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.value(), "port_no": port_no });
                        if DEBUG_OPTIONS.ca_msg_recv {
                            match msg.get_msg_type() {
                                MsgType::Discover => (),
                                MsgType::DiscoverD => {
                                    if msg.get_tree_id().unwrap().is_name("C:2") {
                                        println!("Cellagent {}: {} Port {} received {}", self.cell_id, f, *port_no, msg);
                                    }
                                },
                                _ => {
                                    println!("Cellagent {}: {} Port {} received {}", self.cell_id, f, *port_no, msg);
                                }
                            }
                        }
                        let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                    }
                    msg.process_ca(self, index, port_no, &msg_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
                },
                CmToCaBytes::Tcp((port_no, (allowed_tree, msg_type, direction, bytes))) => {
                    //println!("Cellagent {}: got {} TCP message", self.cell_id, msg_type);
                    let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " PortNumber" })?;
                    let sender_id = match self.border_port_tree_id_map.get(&port_number).cloned() {
                        Some(id) => id.0, // Get the SenderID
                        None => return Err(CellagentError::Border { func_name: f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    };
                    let ref mut tree_map = match self.tree_name_map.get(&sender_id).cloned() {
                        Some(map) => map,
                        None => return Err(CellagentError::TreeNameMap { func_name: f, cell_id: self.cell_id.clone(),  sender_id: sender_id.clone()}.into())
                    };
                    let serialized = ::std::str::from_utf8(&bytes)?;
                    let tree_map_updated = match msg_type {
                        TcpMsgType::Application => self.tcp_application(&sender_id, &allowed_tree, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_application")})?,
                        TcpMsgType::DeleteTree  => self.tcp_delete_tree(&sender_id, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_delete_tree")})?,
                        TcpMsgType::Manifest    => self.tcp_manifest(&sender_id, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_manifest")})?,
                        TcpMsgType::Query       => self.tcp_query(&sender_id, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_query")})?,
                        TcpMsgType::StackTree   => self.tcp_stack_tree(&sender_id, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_stack_tree")})?,
                        TcpMsgType::TreeName    => self.tcp_tree_name(&sender_id, &serialized, direction, tree_map, trace_header).context(CellagentError::Chain { func_name: f, comment: S("tcp_tree_name")})?,
                    };
                    self.tree_name_map.insert(sender_id.clone(), tree_map_updated);
                }
            }
        }
    }
    pub fn process_application_msg(&mut self, msg: &ApplicationMsg, _index: TableIndex,
            port_no: PortNo, msg_tree_id: &TreeID, trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "process_application_msg";
        let senders = self.get_vm_senders(&msg.get_tree_id().clone()).context(CellagentError::Chain { func_name: f, comment: S("") })?;
        for sender in senders {
            sender.send(msg.get_payload().get_body().clone()).context(CellagentError::Chain { func_name: f, comment: S("") })?;
        }
        let tree_id = msg.get_tree_id();
        let user_mask = self.get_mask(&tree_id, trace_header)?;
        let gvm_eqn = self.get_gvm_eqn(tree_id, trace_header)?;
        let save = self.gvm_eval_save(&msg_tree_id, &gvm_eqn, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug print
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_application_msg" };
            let trace = json!({ "cell_id": &self.cell_id,"tree_id": tree_id, "port_no": port_no, "save": save, "msg": msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} tree {} port {} save {} msg {}", self.cell_id, f, tree_id, *port_no, save, msg); }
        }
        if save && msg.is_leafward() { self.add_saved_msg(tree_id, user_mask, (Some(msg.clone()), None), trace_header)?; }
        Ok(())
    }
    pub fn process_discover_msg(&mut self, msg: &DiscoverMsg, port_no: PortNo, trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "process_discover_msg";
        let payload = msg.get_payload();
        let port_number = PortNumber::new(port_no, self.get_no_ports()).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
        let hops = payload.get_hops();
        let path = payload.get_path();
        let my_index;
        { // Limit scope of immutable borrow of self on the next line
            let new_tree_id = payload.get_tree_id();
            let senders_index = payload.get_index();
            let children = &mut HashSet::new();
            let exists = self.exists(new_tree_id);  // Have I seen this tree before?
            //if exists { println!("Cell {}: new_tree_id {} seen before on port {}", ca.get_id(), new_tree_id, *port_no); } else { println!("Cell {}: new_tree_id {} not seen before on port {}", ca.get_id(), new_tree_id, *port_no); }
            let status = if exists { traph::PortStatus::Pruned } else { traph::PortStatus::Parent };
            let mut eqns = HashSet::new();
            eqns.insert(GvmEqn::Recv("true"));
            eqns.insert(GvmEqn::Send("true"));
            eqns.insert(GvmEqn::Xtnd("true"));
            eqns.insert(GvmEqn::Save("false"));
            let gvm_equation = GvmEquation::new(eqns, Vec::new());
            let entry = self.update_traph(new_tree_id, port_number, status, &gvm_equation,
                                        children, senders_index, hops, Some(path), trace_header).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
            if exists { return Ok(()); } // Don't forward if traph exists for this tree - Simple quenching
            self.update_base_tree_map(new_tree_id, new_tree_id, trace_header);
            my_index = entry.get_index();
            let sender_id = SenderID::new(&self.get_id(), "CellAgent")?;
            // Send DiscoverD to sender
            let discoverd_msg = DiscoverDMsg::new(&sender_id, new_tree_id, my_index);
            let mask = Mask::new(port_number);
            // Forward Discover on all except port_no with updated hops and path
            self.send_msg(&self.get_connected_ports_tree_id(), &discoverd_msg, mask, trace_header).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_discover_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "new_tree_id": new_tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} tree_id {}, port_number {} {}", self.cell_id, f, new_tree_id, port_number, msg);  }
            }
        }
        let updated_msg = msg.update(&self.get_id(), my_index);
        let user_mask = DEFAULT_USER_MASK.all_but_port(PortNumber::new(port_no, self.get_no_ports()).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?);
        self.send_msg(&self.get_connected_ports_tree_id(), &updated_msg, user_mask, trace_header).context(CellagentError::Chain {func_name: "process_ca", comment: S("DiscoverMsg")})?;
        self.add_saved_discover(&msg, trace_header); // Discover message are always saved for late port connect
        Ok(())
    }
    pub fn process_discover_d_msg(&mut self, msg: &DiscoverDMsg, port_no: PortNo, trace_header: &mut TraceHeader)
                                  -> Result<(), Error> {
        let f = "process_discoverd_msg";
        let payload = msg.get_payload();
        let tree_id = payload.get_tree_id();
        let my_index = payload.get_table_index();
        let mut children = HashSet::new();
        let port_number = PortNumber::new(port_no, MAX_PORTS).context(CellagentError::Chain { func_name: "process_ca", comment: S("DiscoverDMsg")})?;
        children.insert(port_number);
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("false"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(eqns, Vec::new());
        // Need next statement even though "entry" is only used in debug print
        let _ = self.update_traph(tree_id, port_number, traph::PortStatus::Child, &gvm_eqn,
                              &mut children, my_index, PathLength(CellNo(0)), None, trace_header)?;
        let mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
        let tree_id = payload.get_tree_id();
        self.forward_stacked_trees(tree_id, mask, trace_header)?;
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg && tree_id.is_name("C:2") {   // Debug
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_discover_d_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "port_no": port_no, "msg": msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg && tree_id.is_name("C:2") {
                println!("Cellagent {}: {} tree_id {}, add child on port {} {}", self.cell_id, f, tree_id, port_number, msg);
                println!("Cellagent {}: {} send unblock", self.cell_id, f);
            }
        }
        self.ca_to_cm.send(CaToCmBytes::Unblock)?;
        Ok(())
    }
    pub fn process_manifest_msg(&mut self, msg: &ManifestMsg, _index: TableIndex, port_no: PortNo,
                                msg_tree_id: &TreeID, trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "process_manifest_msg";
        let header = msg.get_header();
        let payload = msg.get_payload();
        let manifest = payload.get_manifest();
        let msg_tree_map = header.get_tree_map();
        let deployment_tree_id = payload.get_deploy_tree_id();
        let sender_id = header.get_sender_id();
        self.deploy(sender_id, deployment_tree_id, msg_tree_id, msg_tree_map, manifest, trace_header).context(CellagentError::Chain { func_name: "process_ca", comment: S("ManifestMsg")})?;
        let tree_id = payload.get_deploy_tree_id();
        let traph = self.get_traph(tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        let entry = traph.get_tree_entry(&tree_id.get_uuid())?;
        let user_mask = entry.get_mask();
        let gvm_eqn = self.get_gvm_eqn(tree_id, trace_header)?;
        let save = self.gvm_eval_save(&msg_tree_id, &gvm_eqn, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug;
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_manifest_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "port_no": port_no, "msg": msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} tree {} save {} port {} manifest {}", self.cell_id, f, msg_tree_id, save, *port_no, manifest.get_id()); }
        }
        if save { self.add_saved_msg(tree_id, user_mask, (None, Some(msg.clone())), trace_header)?; }
        Ok(())
    }
    pub fn process_stack_tree_msg(&mut self, msg: &StackTreeMsg, port_no: PortNo,
                                  msg_tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "process_stack_tree_msg";
        let header = msg.get_header();
        let payload = msg.get_payload();
        //println!("Cellagent {}: {} port_no {} got {}", self.cell_id, f, *port_no, msg);
        let parent_tree_id = &payload.get_parent_tree_id().clone();
        let new_tree_id = &payload.get_new_tree_id().clone();
        let other_index = payload.get_table_index();
        //println!("Message Cell {}: StackTreeMsg on port {} index {} for tree {}", ca.get_id(), *port_no, *other_index, self.payload.get_new_tree_id());
        let sender_id = header.get_sender_id();
        let gvm_eqn = payload.get_gvm_eqn();
        if let Some(mut entry) = self.stack_tree(sender_id, new_tree_id, parent_tree_id, gvm_eqn, trace_header)? {
            let port_number = PortNumber::new(port_no, self.get_no_ports())?;
            let mut traph = self.get_traph(new_tree_id, trace_header)?;
            // Update StackTreeMsg and forward
            entry.add_other_index(port_number, other_index);
            traph.set_tree_entry(&new_tree_id.get_uuid(), entry)?;
            self.update_entry(entry).context(CellagentError::Chain { func_name: f, comment: S("") })?;
            let updated_msg = msg.update(entry.get_index());
            let parent_entry = self.get_tree_entry(parent_tree_id, trace_header)?;
            let parent_mask = parent_entry.get_mask().and(DEFAULT_USER_MASK);  // Excludes port 0
            self.send_msg(&self.connected_tree_id, &updated_msg, parent_mask, trace_header)?; // Send to children of parent tree
            // Send StackTreeDMsg only if can receive on this tree
            if entry.may_receive() {
                let fwd_index = self.use_index()?;
                let mut fwd_entry = entry.clone();
                fwd_entry.set_table_index(fwd_index);
                fwd_entry.set_mask(Mask::new(port_number));
                self.ca_to_cm.send(CaToCmBytes::Entry(fwd_entry))?;
                let mask = Mask::new(port_number);
                let new_msg = StackTreeDMsg::new(sender_id, new_tree_id, entry.get_index(), fwd_index);
                self.send_msg(self.get_connected_ports_tree_id(), &new_msg, mask, trace_header)?;
            }
            let parent_tree_id = payload.get_parent_tree_id();
            //println!("Cellagent {}: StackTree {} parent tree {}", self.cell_id, new_tree_id, parent_tree_id);
            let base_tree_id = self.get_base_tree_id(parent_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("") })?;
            self.update_base_tree_map(parent_tree_id, &base_tree_id, trace_header);
            let gvm_eqn = payload.get_gvm_eqn();
            let save = self.gvm_eval_save(&parent_tree_id, gvm_eqn, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) })?;
            if save { self.add_saved_stack_tree(parent_tree_id, msg, trace_header); }
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_stack_tree_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "new_tree_id": new_tree_id, "port_no": port_no, "msg": msg.value() });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} tree {} save {} port {} msg {}", self.cell_id, f, msg_tree_id, save, *port_no, msg); }
            }
        }
        self.ca_to_cm.send(CaToCmBytes::Unblock)?;
        Ok(())

    }
    pub fn process_stack_tree_d_msg(&mut self, msg: &StackTreeDMsg, port_no: PortNo,
                                    trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "process_stack_treed_msg";
        let payload = msg.get_payload();
        let port_number =PortNumber::new(port_no, self.no_ports)?;
        let tree_id = payload.get_tree_id();
        let tree_uuid = tree_id.get_uuid();
        let mut traph = self.get_traph(tree_id, trace_header)?;
        let mut entry = traph.get_tree_entry(&tree_uuid)?;
        let other_index = payload.get_table_index();
        entry.set_other_index(port_number, other_index);
        let user_mask = Mask::new(port_number);
        let mask = entry.get_mask().or(user_mask);
        entry.set_mask(mask);
        traph.set_tree_entry(&tree_uuid, entry)?;
        self.update_entry(entry).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) })?;
        //if !entry.may_receive() {
        //    let sender_id = msg.get_header().get_sender_id();
        //    let new_tree_id = msg.get_payload().get_tree_id();
            let my_fwd_index = payload.get_fwd_index();
        //    self.ca_to_cm.send(CaToPePacket::Entry(fwd_entry))?;
            self.forward_saved(tree_id, user_mask, my_fwd_index, trace_header)?;
        //    let mask = Mask::new(port_number);
        //    let new_msg = StackTreeDMsg::new(sender_id, new_tree_id, entry.get_index(), my_fwd_index);
        //    self.send_msg(self.get_connected_ports_tree_id(), &new_msg, mask, trace_header)?;
        //}
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_process_stack_tree_d_msg" };
            let trace = json!({ "cell_id": &self.cell_id });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        self.ca_to_cm.send(CaToCmBytes::Unblock)?;
        Ok(())
    }
    fn may_send(&self, tree_id: &TreeID, trace_header: &mut TraceHeader) -> Result<bool, Error> {
        let entry = self.get_tree_entry(tree_id, trace_header)?;
        Ok(entry.may_send())
    }
    fn tcp_application(&mut self, sender_id: &SenderID, allowed_tree: &AllowedTree, serialized: &str,
            direction: MsgDirection, tree_map: &MsgTreeMap, trace_header: &mut TraceHeader) -> Result<MsgTreeMap, Error> {
        let f = "tcp_application";
        let tree_id = match tree_map.get(allowed_tree.get_name()) {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { func_name: f, cell_id: self.cell_id.clone(), tree_name: allowed_tree.clone() }.into())
        };
        if !self.may_send(tree_id, trace_header)? { return Err(CellagentError::MayNotSend { func_name: f, cell_id: self.cell_id.clone(), tree_id: tree_id.clone() }.into()); }
        let msg = ApplicationMsg::new(sender_id, tree_id, direction, serialized);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_got_tcp_application_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": tree_id, "msg": msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} sending on tree {} application msg {}", self.cell_id, f, tree_id, msg); }
        }
        self.send_msg(tree_id, &msg, DEFAULT_USER_MASK, trace_header)?;
        if msg.is_leafward() { self.add_saved_msg(tree_id, DEFAULT_USER_MASK, (Some(msg), None), trace_header)?; }
        Ok(tree_map.clone())
    }
    fn tcp_delete_tree(&self, _sender_id: &SenderID, _serialized: &str, _direction: MsgDirection,
                       _tree_map: &MsgTreeMap, _trace_header: &mut TraceHeader) -> Result<MsgTreeMap, Error> {
        let f = "tcp_delete_tree";
        // Needs may_send test
        Err(UtilityError::Unimplemented { func_name: f, feature: S("TcpMsgType::Application")}.into())
    }
    fn tcp_manifest(&mut self, sender_id: &SenderID, serialized: &str, _direction: MsgDirection,
                    tree_map: &MsgTreeMap, trace_header: &mut TraceHeader)-> Result<MsgTreeMap, Error> {
        let f = "tcp_manifest";
        let tcp_msg = serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " deserialize StackTree" })?;
        let ref deploy_tree_name = self.get_msg_params(&tcp_msg, "deploy_tree_name").context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " parent tree name" })?;
        let deploy_tree_id = match tree_map.get(AllowedTree::new(deploy_tree_name).get_name()) {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { func_name: "listen_cm_loop 4", cell_id: self.cell_id.clone(), tree_name: AllowedTree::new(deploy_tree_name) }.into())
        };
        if !self.may_send(deploy_tree_id, trace_header)? { return Err(CellagentError::MayNotSend { func_name: f, cell_id: self.cell_id.clone(), tree_id: deploy_tree_id.clone() }.into()); }
        let manifest_ser = self.get_msg_params(&tcp_msg, "manifest").context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " manifest" })?;
        let manifest = serde_json::from_str::<Manifest>(&manifest_ser)?;
        let allowed_trees = manifest.get_allowed_trees().clone();
        let mut msg_tree_map = HashMap::new();
        for allowed_tree in allowed_trees {
            match tree_map.get(allowed_tree.get_name()) {
                Some(tree_id) => msg_tree_map.insert(S(allowed_tree.get_name()), tree_id.clone()),
                None => return Err(CellagentError::TreeMap { func_name: "listen_cm_loop 5", cell_id: self.cell_id.clone(), tree_name: allowed_tree.clone() }.into())
            };
        }
        let msg = ManifestMsg::new(sender_id, &deploy_tree_id, &msg_tree_map, &manifest);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_got_manifest_tcp_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "deploy_tree_id": deploy_tree_id, "msg": msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg { println!("Cellagent {}: {} sending on tree {} manifest tcp_msg {}", self.cell_id, f, deploy_tree_id, msg); }
        }
        let mask = self.get_mask(deploy_tree_id, trace_header)?;
        self.send_msg(deploy_tree_id, &msg, mask.or(Mask::port0()), trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " send manifest" })?;
        self.add_saved_msg(deploy_tree_id, mask, (None, Some(msg)), trace_header)?;
        Ok(tree_map.clone())
    }
    fn tcp_query(&self, _sender_id: &SenderID, _serialized: &str, _direction: MsgDirection,
                 _tree_map: &MsgTreeMap, _trace_header: &mut TraceHeader) -> Result<MsgTreeMap, Error> {
        let f = "tcp_query";
        // Needs may_send test
        Err(UtilityError::Unimplemented { func_name: f, feature: S("TcpMsgType::Application")}.into())
    }
    fn tcp_stack_tree(&mut self, sender_id: &SenderID, serialized: &str, direction: MsgDirection,
                      tree_map: &MsgTreeMap, trace_header: &mut TraceHeader) -> Result<MsgTreeMap, Error> {
        let f = "tcp_stack_tree";
        let tcp_msg = serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " deserialize StackTree" })?;
        let parent_tree_str = self.get_msg_params(&tcp_msg, "parent_tree_name")?;
        let parent_tree_name = AllowedTree::new(&parent_tree_str);
        let ref parent_tree_id = match tree_map.get(parent_tree_name.get_name()).cloned() {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { func_name: f, cell_id: self.cell_id.clone(), tree_name: parent_tree_name }.into())
        };
        if !self.may_send(parent_tree_id, trace_header)? { return Err(CellagentError::MayNotSend { func_name: f, cell_id: self.cell_id.clone(), tree_id: parent_tree_id.clone() }.into()); }
        let my_tree_id = &self.my_tree_id.clone();
        let new_tree_name = self.get_msg_params(&tcp_msg, "new_tree_name")?;
        let ref new_tree_id = self.my_tree_id.add_component(&new_tree_name).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " new_tree_id" })?;
        let gvm_eqn_serialized = self.get_msg_params(&tcp_msg, "gvm_eqn")?;
        let ref gvm_eqn = serde_json::from_str::<GvmEquation>(&gvm_eqn_serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " gvm" })?;
        let entry = match self.stack_tree(sender_id, new_tree_id, parent_tree_id, gvm_eqn, trace_header).context(CellagentError::Chain { func_name: f, comment: S("stack tree")})? {
            Some(e) => e,
            None => return Err(CellagentError::StackTree { func_name: f, cell_id: self.cell_id.clone(), tree_id: new_tree_id.clone() }.into())
        };
        let stack_tree_msg = StackTreeMsg::new(sender_id, new_tree_id, parent_tree_id, direction, entry.get_index(), gvm_eqn);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.process_msg {   // Debug
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_got_stack_tree_tcp_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "new_tree_id": new_tree_id, "entry": entry, "msg": stack_tree_msg.value() });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.process_msg {
                println!("Cellagent {}: {} sending on tree {} manifest tcp_msg {}", self.cell_id, f, new_tree_id, stack_tree_msg);
                println!("Cellagent {}: new tree id {} entry {}", self.cell_id, new_tree_id, entry);
            }
        }
        let mut tree_map_clone = tree_map.clone();
        tree_map_clone.insert(S(AllowedTree::new(&new_tree_name).get_name()), new_tree_id.clone());
        let parent_entry = self.get_tree_entry(&parent_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("get parent_entry") })?;
        let parent_mask = parent_entry.get_mask().and(DEFAULT_USER_MASK);  // Excludes port 0
        let traph = self.get_traph(&parent_tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        let variables = traph.get_params(gvm_eqn.get_variables())?;
        if gvm_eqn.eval_xtnd(&variables)? {
            self.send_msg(&self.connected_tree_id, &stack_tree_msg, parent_mask, trace_header)?;
            self.add_saved_stack_tree(my_tree_id, &stack_tree_msg, trace_header);
        }
        Ok(tree_map_clone)
    }
    fn tcp_tree_name(&self, _sender_id: &SenderID, _serialized: &str, _direction: MsgDirection,
                     _tree_map: &MsgTreeMap, _trace_header: &mut TraceHeader) -> Result<MsgTreeMap, Error> {
        let f = "tcp_tree_name";
        Err(CellagentError::TcpMessageType { func_name: f, cell_id: self.cell_id.clone(), msg: TcpMsgType::TreeName}.into())
    }
    fn get_msg_params(&self, msg: &HashMap<String, String>, param: &str) -> Result<String, Error> {
        //println!("Cellagent {}: get_param {}", self.cell_id, param);
        match msg.get(&S(param)).cloned() {
            Some(p) => Ok(p),
            None => Err(CellagentError::Message { func_name: "get_param", cell_id: self.cell_id.clone(), msg: msg.clone() }.into())
        }
    }
    fn gvm_eval_save(&self, tree_id: &TreeID, gvm_eqn: &GvmEquation, trace_header: &mut TraceHeader) -> Result<bool, Error> {
        let f = "gvm_eval_save";
        // True if I should save this message for children that join this tree later
        // TODO: Add test to see if all child ports on the parent tree have responded, in which case I can delete saved msgs
        match self.get_base_tree_id(tree_id, trace_header) {
            Ok(base_tree_id) => {
                //if msg_type == MsgType::StackTree { println!("Cellagent {}: {} found", self.cell_id, tree_uuid); }
                let mut locked = self.traphs.lock().unwrap();
                let traph = match locked.entry(base_tree_id.get_uuid().clone()) {
                    Entry::Occupied(t) => t.into_mut(),
                    Entry::Vacant(_) => return Err(CellagentError::Tree { cell_id: self.cell_id.clone(), func_name: f, tree_uuid: base_tree_id.get_uuid().clone() }.into())
                };
                let params = traph.get_params(gvm_eqn.get_variables())?;
                let save = gvm_eqn.eval_save(&params)?;
                let xtnd = gvm_eqn.eval_xtnd(&params)?;
                Ok(save && xtnd)
            },
            Err(_) => Ok(false)
        }
    }
    /*
	fn send_tree_names(&mut self, outside_tree_id: &TreeID, allowed_tree_ids: Vec<TreeID>, port_number: PortNumber) {
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
    fn port_connected(&mut self, port_no: PortNo, is_border: bool, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "port_connected";
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_send_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "is_border": is_border });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, f);
        }
        if is_border {
            // Create tree to talk to outside
            let mut eqns = HashSet::new();
            eqns.insert(GvmEqn::Recv("true"));
            eqns.insert(GvmEqn::Send("true"));
            eqns.insert(GvmEqn::Xtnd("false"));
            eqns.insert(GvmEqn::Save("false"));
            let gvm_eqn = GvmEquation::new(eqns, Vec::new());
            let new_tree_id = self.my_tree_id.add_component("Noc").context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            let _ = self.update_traph(&new_tree_id, port_number, traph::PortStatus::Parent,
                                          &gvm_eqn, &mut HashSet::new(), TableIndex(0), PathLength(CellNo(1)), None, trace_header).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            let base_tree = AllowedTree::new("Base");
            let my_tree_id = self.my_tree_id.clone();
            let sender_id = SenderID::new(&self.cell_id, &format!("BorderPort+{}", *port_no))?;
            self.add_tree_name_map_item(&sender_id,&base_tree, &my_tree_id);
            self.border_port_tree_id_map.insert(port_number, (sender_id.clone(), new_tree_id.clone()));
            let tree_name_msg = TreeNameMsg::new(&sender_id, &base_tree.get_name());
            let serialized = serde_json::to_string(&tree_name_msg).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            let bytes = ByteArray(serialized.into_bytes());
            self.ca_to_cm.send(CaToCmBytes::Tcp((port_number, (base_tree, TcpMsgType::TreeName, MsgDirection::Rootward, bytes)))).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) + "border" })?;
            Ok(())
        } else {
            let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?);
            let path = Path::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
            let hops = PathLength(CellNo(1));
            let my_table_index = self.my_entry.get_index();
            let sender_id = SenderID::new(&self.cell_id, "CellAgent")?;
            let discover_msg = DiscoverMsg::new(&sender_id, &self.my_tree_id, my_table_index, &self.cell_id, hops, path);
            //println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, discover_msg);
            let entry = CaToCmBytes::Entry(*self.connected_tree_entry.lock().unwrap());
            self.ca_to_cm.send(entry).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + "interior"})?;
            self.send_msg(&self.connected_tree_id, &discover_msg, port_no_mask, trace_header).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            self.forward_discover(port_no_mask, trace_header).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) })?;
            Ok(())
        }
    }
    fn port_disconnected(&self, port_no: PortNo) -> Result<(), Error> {
        //println!("Cell Agent {} got disconnected on port {}", self.cell_id, port_no);
        let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
        self.connected_tree_entry.lock().unwrap().and_with_mask(port_no_mask.not());
        let entry = CaToCmBytes::Entry(*self.connected_tree_entry.lock().unwrap());
        self.ca_to_cm.send(entry)?;
        Ok(())
    }
    fn forward_discover(&mut self, mask: Mask, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let saved = self.get_saved_discover();
        //if saved.len() > 0 { println!("Cell {}: forwarding {} discover msgs on ports {:?}", self.cell_id, saved.len(), mask.get_port_nos()); }
        for msg in saved.iter() {
            self.send_msg(&self.connected_tree_id, msg, mask, trace_header)?;
            {/*   // Debug print
                let msg_type = MsgType::msg_type(&packets[0]);
                println!("CellAgent {}: forward discover on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg_type);
            */}
        }
        Ok(())
    }
    fn forward_stacked_trees(&mut self, tree_id: &TreeID, mask: Mask, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "forward_stacked_trees";
        // Forward all saved StackTreeMsg of trees stacked on this one
        let traph = self.get_traph(tree_id, trace_header).context(CellagentError::Chain { func_name: f, comment: S("")})?;
        let trees = traph.get_stacked_trees();
        let locked = trees.lock().unwrap();
        //println!("Cellagent {}: {} locked {:?}", self.cell_id, f, locked.keys());
        //if tree_id.is_name("C:2") { println!("Cellagent {}: {} forwarding {} on tree {}", self.cell_id, f, locked.len(), tree_id); }
        for tree in locked.values() {
            self.forward_stack_tree(tree.get_tree_id(), mask, trace_header)?; // Forward stack tree messages on tree
            let stacked_tree_ids = tree.get_stacked_tree_ids();
            //if locked.len() > 1 { println!("CellAgent {}: {} {} stacked trees", self.cell_id, f, stacked_tree_ids.len()); }
            // Forward stack tree messages on trees stacked on tree
            for tree_id in stacked_tree_ids.iter() {
                self.forward_stack_tree(tree_id, mask, trace_header)?;
            }
        }
        Ok(())
    }
    fn forward_stack_tree(&mut self, tree_id: &TreeID, mask: Mask, trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "forward_stack_tree";
        let saved = self.get_saved_stack_tree(tree_id);
        for msg in saved.iter() {
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
                let msg_type = msg.get_msg_type();
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_forward_stack_tree_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "port_nos": &mask.get_port_nos(), "msg_type": &msg_type });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.saved_msgs { println!("CellAgent {}: {} tree on ports {:?} {}", self.cell_id, f, mask.get_port_nos(), msg_type); }
            }
            self.send_msg(&self.connected_tree_id, msg,mask, trace_header)?;
        }
        Ok(())
    }
    // Continuation of the hack due thread::spawn not accepting Box<Message>
    fn forward_saved(&self, tree_id: &TreeID, mask: Mask, fwd_index: TableIndex,
                     trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "forward_saved";
        let saved_msgs = self.get_saved_msgs(&tree_id, trace_header);
        for saved_msg in saved_msgs {
            match saved_msg.0 {
                Some(msg) => self.forward_saved_application(tree_id, mask, &msg, trace_header)?,
                None => ()
            }
            match saved_msg.1 {
                Some(msg) => self.forward_saved_manifest(tree_id, mask, &msg, trace_header)?,
                None => ()
            }
        }
        Ok(())
    }
    fn forward_saved_application(&self, tree_id: &TreeID, mask: Mask, msg: &ApplicationMsg, trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "forward_saved_application";
        //println!("Cellagent {}: {} {} msgs on tree {}", self.cell_id, f, saved.len(), tree_id);
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_forward_saved_msg" };
                let trace = json!({ "cell_id": &self.cell_id, "port_nos": mask.get_port_nos(), "msg_type": MsgType::Application });
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                if DEBUG_OPTIONS.saved_msgs { println!("Cellagent {}: {} on ports {:?} {}", self.cell_id, f, mask.get_port_nos(), MsgType::Application); }
            }
            //self.send_packets_by_index(fwd_index, mask, &packets)?;
            self.send_msg(tree_id, msg, mask, trace_header)?;
        Ok(())
    }
    fn forward_saved_manifest(&self, tree_id: &TreeID, mask: Mask, msg: &ManifestMsg, trace_header: &mut TraceHeader)
                                 -> Result<(), Error> {
        let f = "forward_saved_manifest";
        //println!("Cellagent {}: {} {} msgs on tree {}", self.cell_id, f, saved.len(), tree_id);
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.saved_msgs {   // Debug print
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_forward_saved_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "port_nos": mask.get_port_nos(), "msg_type": MsgType::Manifest });
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            if DEBUG_OPTIONS.saved_msgs { println!("Cellagent {}: {} on ports {:?} {}", self.cell_id, f, mask.get_port_nos(), MsgType::Manifest); }
        }
        //self.send_packets_by_index(fwd_index, mask, &packets)?;
        self.send_msg(tree_id, msg, mask, trace_header)?;
        Ok(())
    }
    fn send_msg<T: Message>(&self, tree_id: &TreeID, msg: &T, user_mask: Mask,
            trace_header: &mut TraceHeader) -> Result<(), Error>
        where T: Message + ::std::marker::Sized + serde::Serialize + fmt::Display
    {
        let f = "send_msg";
        if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.ca_msg_send {  // Debug print
            let mask = self.get_mask(tree_id, trace_header)?;
            let port_mask = user_mask.and(mask);
            let ports = Mask::get_port_nos(&port_mask);
            let msg_type = msg.get_msg_type();
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "ca_send_msg" };
            let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "port_nos": &ports, "msg": msg.value() });
            if DEBUG_OPTIONS.ca_msg_send {
                match msg_type {
                    MsgType::Discover => (),
                    MsgType::DiscoverD => {
                        match msg.get_tree_id() {
                            Some(tree_id) => if tree_id.is_name("C:2") {
                                println!("Cellagent {}: {} send on ports {:?} msg {}", self.cell_id, f, ports, msg);
                            },
                            None => ()
                        }
                    },
                    _ => {
                        println!("Cellagent {}: {} send on ports {:?} msg {}", self.cell_id, f, ports, msg)
                    }
                }
            }
            let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        let direction = msg.get_header().get_direction();
        let is_blocking = msg.is_blocking();
        let bytes = msg.to_bytes()?;
        self.send_bytes(tree_id, direction, is_blocking, user_mask, bytes, trace_header)?;
        Ok(())
    }
    fn send_bytes(&self, tree_id: &TreeID, direction: MsgDirection, is_blocking: bool, user_mask: Mask,
                  bytes: ByteArray, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "send_bytes";
        let tree_uuid = tree_id.get_uuid();
        let base_tree_uuid = match self.tree_map.lock().unwrap().get(&tree_uuid).cloned() {
            Some(id) => id,
            None => return Err(CellagentError::Tree { func_name: f, cell_id: self.cell_id.clone(), tree_uuid }.into())
        };
        let index = match self.traphs.lock().unwrap().get(&base_tree_uuid) {
            Some(traph) => traph.get_table_index(&tree_uuid).context(CellagentError::Chain { func_name: f, comment: S("")})?,
            None => return Err(CellagentError::NoTraph { cell_id: self.cell_id.clone(), func_name: f, tree_uuid }.into())
        };
        self.send_bytes_by_index(index, tree_id, user_mask, direction, is_blocking, bytes, trace_header)
    }
    fn send_bytes_by_index(&self, index: TableIndex, tree_id: &TreeID, user_mask: Mask, direction: MsgDirection,
                           is_blocking: bool, bytes: ByteArray, trace_header: &mut TraceHeader) -> Result<(), Error> {
        let msg = CaToCmBytes::Bytes((index, tree_id.clone(), user_mask, direction, is_blocking, bytes));
        self.ca_to_cm.send(msg)?;
        Ok(())
    }
}
impl fmt::Display for CellAgent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = format!("Cell Agent");
        for (_, traph) in self.traphs.lock().unwrap().iter() {
            s = s + &format!("\n{}", traph);
        }
        write!(f, "{}", s) }
}
// Errors
#[derive(Debug, Fail)]
pub enum CellagentError {
    #[fail(display = "CellagentError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "CellagentError::BaseTree {}: No base tree for tree {} on cell {}", func_name, tree_id, cell_id)]
    BaseTree { func_name: &'static str, cell_id: CellID, tree_id: TreeID },
    #[fail(display = "CellagentError::Border {}: Port {} is not a border port on cell {}", func_name, port_no, cell_id)]
    Border { func_name: &'static str, cell_id: CellID, port_no: u8 },
//    #[fail(display = "CellAgentError::BorderMsgType {}: Message type {} is not accepted from a border port on cell {}", func_name, msg_type, cell_id)]
//    BorderMsgType { func_name: &'static str, cell_id: CellID, msg_type: MsgType },
//    #[fail(display = "CellagentError::ManifestVms {}: No VMs in manifest for cell {}", func_name, cell_id)]
//    ManifestVms { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellagentError::MayNotSend {}: Cell {} does not have permission to send on tree {}", func_name, cell_id, tree_id)]
    MayNotSend { cell_id: CellID, func_name: &'static str, tree_id: TreeID },
    #[fail(display = "CellagentError::Message {}: Malformed request {:?} from border port on cell {}", func_name, msg, cell_id)]
    Message { func_name: &'static str, cell_id: CellID, msg: HashMap<String, String> },
    #[fail(display = "CellAgentError::NoTraph {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    NoTraph { cell_id: CellID, func_name: &'static str, tree_uuid: Uuid },
//    #[fail(display = "CellagentError::SavedMsgType {}: Message type {} does not support saving", func_name, msg_type)]
//    SavedMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "CellAgentError::Size {}: No more room in routing table for cell {}", func_name, cell_id)]
    Size { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellAgentError::StackTree {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)]
    StackTree { func_name: &'static str, tree_id: TreeID, cell_id: CellID },
    #[fail(display = "CellagentError::TcpMessageType {}: Unsupported request {:?} from border port on cell {}", func_name, msg, cell_id)]
    TcpMessageType { func_name: &'static str, cell_id: CellID, msg: TcpMsgType },
//    #[fail(display = "CellAgentError::TenantMask {}: Cell {} has no tenant mask", func_name, cell_id)]
//    TenantMask { func_name: &'static str, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeNameMap {}: Cell {} has no tree name map entry for {}", func_name, cell_id, sender_id)]
    TreeNameMap { func_name: &'static str, cell_id: CellID, sender_id: SenderID },
    #[fail(display = "CellAgentError::TreeMap {}: Cell {} has no tree map entry for {}", func_name, cell_id, tree_name)]
    TreeMap { func_name: &'static str, cell_id: CellID, tree_name: AllowedTree },
    #[fail(display = "CellAgentError::Tree {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid },
    #[fail(display = "CellAgentError::TreeIndex {}: No tree associated with index {:?} on cell {}", func_name, index, cell_id)]
    TreeIndex { func_name: &'static str, index: TableIndex, cell_id: CellID }
}