use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
//use uuid::Uuid;

use config::{BASE_TREE_NAME, CONNECTED_PORTS_TREE_NAME, CONTROL_TREE_NAME, MAX_ENTRIES, CellNo, CellType, PathLength, PortNo, TableIndex};
use gvm_equation::{GvmEquation, GvmEqn};
use message::{Message, MsgHeader, MsgTreeMap, MsgType, TcpMsgType, DiscoverMsg, ManifestMsg, StackTreeMsg, TreeNameMsg};
use message_types::{CaToPe, CaFromPe, CaToVm, VmFromCa, VmToCa, CaFromVm, CaToPePacket, PeToCaPacket,
	VmToTree, VmFromTree, TreeToVm, TreeFromVm};
use nalcell::CellConfig;
use name::{Name, CellID, TreeID, UptreeID, VmID};
use packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use port;
use routing_table_entry::{RoutingTableEntry};
use serde;
use serde_json;
use service::NocAgent;
use traph;
use traph::{Traph};
use tree::Tree;
use uptree_spec::{AllowedTree, Manifest, VmSpec};
use utility::{BASE_TENANT_MASK, DEFAULT_USER_MASK, Mask, Path, PortNumber, S, UtilityError};
use uuid_fake::Uuid;
use vm::VirtualMachine;

use failure::{Error, Fail, ResultExt};

type BorderTreeIDMap = HashMap<PortNumber, TreeID>;

pub type SavedDiscover = Vec<Packet>;
pub type SavedMsg = (Mask, Vec<Packet>);
pub type SavedMsgs = HashMap<TreeID, Vec<SavedMsg>>;
pub type Traphs = HashMap<Uuid, Traph>;
pub type Trees = HashMap<TableIndex, TreeID>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeIDMap = HashMap<Uuid, TreeID>;
pub type TreeNameMap = HashMap<TreeID, MsgTreeMap>;
pub type TreeVmMap = HashMap<TreeID, Vec<CaToVm>>;

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
	saved_msgs: Arc<Mutex<SavedMsgs>>,
	saved_discover: Arc<Mutex<Vec<SavedDiscover>>>,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<Trees>>,
	traphs: Arc<Mutex<Traphs>>,
	tree_map: Arc<Mutex<TreeMap>>,
	tree_name_map: TreeNameMap,
    border_port_tree_id_map: BorderTreeIDMap, // Find the tree id associated with a border port
    base_tree_map: HashMap<TreeID, TreeID>, // Find the black tree associated with any tree, needed for stacking
	tree_id_map: Arc<Mutex<TreeIDMap>>, // For debugging
	tenant_masks: Vec<Mask>,
    tree_vm_map: TreeVmMap,
    ca_to_vms: HashMap<VmID, CaToVm>,
	ca_to_pe: CaToPe,
	vm_id_no: usize,
	up_tree_senders: HashMap<UptreeID, HashMap<String,TreeID>>,
	up_traphs_clist: HashMap<TreeID, TreeID>,
	packet_assemblers: PacketAssemblers,
}
impl CellAgent {
	pub fn new(cell_id: &CellID, cell_type: CellType, config: CellConfig, no_ports: PortNo, ca_to_pe: CaToPe ) 
				-> Result<CellAgent, Error> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let my_tree_id = TreeID::new(cell_id.get_name())?;
		my_tree_id.append2file()?;	
		let control_tree_id = TreeID::new(cell_id.get_name())?.add_component(CONTROL_TREE_NAME)?;
		let connected_tree_id = TreeID::new(cell_id.get_name())?.add_component(CONNECTED_PORTS_TREE_NAME)?;
		connected_tree_id.append2file()?;
		let mut free_indices = Vec::new();
		let trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..(*MAX_ENTRIES) { 
			free_indices.push(TableIndex(i)); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		Ok(CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id, cell_type: cell_type, config: config,
			control_tree_id: control_tree_id, connected_tree_id: connected_tree_id,	tree_vm_map: HashMap::new(),
			no_ports: no_ports, traphs: traphs, vm_id_no: 0, tree_id_map: Arc::new(Mutex::new(HashMap::new())),
			free_indices: Arc::new(Mutex::new(free_indices)), tree_map: Arc::new(Mutex::new(HashMap::new())),
			tree_name_map: HashMap::new(), ca_to_vms: HashMap::new(), border_port_tree_id_map: HashMap::new(),
			saved_msgs: Arc::new(Mutex::new(HashMap::new())), saved_discover: Arc::new(Mutex::new(Vec::new())),
			my_entry: RoutingTableEntry::default(TableIndex(0))?, base_tree_map: HashMap::new(),
			connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(TableIndex(0))?)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), up_tree_senders: HashMap::new(),
			up_traphs_clist: HashMap::new(), ca_to_pe: ca_to_pe, packet_assemblers: PacketAssemblers::new()})
		}
	pub fn initialize(&mut self, cell_type: CellType, ca_from_pe: CaFromPe) -> Result<(), Error> {
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = PortNumber::new(PortNo{v:0}, self.no_ports).unwrap(); // No error possible for port 0
		let other_index = TableIndex(0);
		let hops = PathLength(CellNo(0));
		let path = None;
		let control_tree_id = self.control_tree_id.clone();
		let connected_tree_id = self.connected_tree_id.clone();
		let my_tree_id = self.my_tree_id.clone();
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_equation = GvmEquation::new(eqns, Vec::new());
		self.update_traph(&control_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(&gvm_equation),
				&mut HashSet::new(), other_index, hops, path)?;
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("false"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_equation = GvmEquation::new(eqns, Vec::new());
		let connected_tree_entry = self.update_traph(&connected_tree_id, port_number_0, 
			traph::PortStatus::Parent, Some(&gvm_equation),
			&mut HashSet::new(), other_index, hops, path)?;
		self.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
		// Create my tree
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_equation = GvmEquation::new(eqns, Vec::new());
		self.my_entry = self.update_traph(&my_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(&gvm_equation), 
				&mut HashSet::new(), other_index, hops, path)?; 
		self.listen_pe(ca_from_pe)?;
		Ok(())
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_traphs(&self) -> &Arc<Mutex<Traphs>> { &self.traphs }
    pub fn get_tree_name_map(&self) -> &TreeNameMap { &self.tree_name_map }
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
	pub fn get_saved_msgs(&self, base_tree_id: &TreeID) -> Option<Vec<SavedMsg>> {
        match self.saved_msgs.lock().unwrap().get(base_tree_id) {
            Some(msgs) => Some(msgs.clone()),
            None => None
        }
	}
	pub fn get_saved_discover(&self) -> Vec<SavedDiscover> {
		self.saved_discover.lock().unwrap().to_vec()
	}
	pub fn add_saved_msg(&mut self, base_tree_id: &TreeID, mask: Mask, packets: &Vec<Packet>) -> Option<Vec<SavedMsg>> {
        {
            let mut locked = self.saved_msgs.lock().unwrap();
            let saved_msgs = match locked.get_mut(base_tree_id) {
                Some(saved) => {
                    saved.push((mask, packets.to_owned()));
                    saved.to_owned()
                },
                None => vec![(mask, packets.to_owned())]
            };
            //println!("Cell {}: saving msg {}", self.cell_id, packets[0]);
            //let msg = MsgType::get_msg(&packets).unwrap();
            //println!("Cell {}: save generic {}", self.cell_id, msg);
            locked.insert(base_tree_id.clone(), saved_msgs);
        }
		self.get_saved_msgs(base_tree_id)
	}
	pub fn add_saved_discover(&mut self, packets: &SavedDiscover) -> Vec<SavedDiscover> {
		{
			let mut saved_discover = self.saved_discover.lock().unwrap();
			//let msg = MsgType::get_msg(&packets).unwrap();
			//println!("Cell {}: save discover {}", self.cell_id, msg);
			saved_discover.push(packets.clone());
		}
		self.get_saved_discover()
	}
	pub fn get_tenant_mask(&self) -> Result<&Mask, CellagentError> {
		let f = "get_tenant_mask";
		if let Some(tenant_mask) = self.tenant_masks.last() {
			Ok(tenant_mask)
		} else {
			return Err(CellagentError::TenantMask { cell_id: self.get_id(), func_name: f } )
		}
	}
	//pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn get_connected_ports_tree_id(&self) -> &TreeID { &self.connected_tree_id }
	pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(&tree_id.get_uuid())
	}
	fn use_index(&mut self) -> Result<TableIndex, CellagentError> {
		let f = "use_index";
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(CellagentError::Size { cell_id: self.cell_id.clone(), func_name: f } )
		}
	}
	fn free_index(&mut self, index: TableIndex) {
		self.free_indices.lock().unwrap().push(index);
	}
	pub fn update_traph(&mut self, base_tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				gvm_eqn: Option<&GvmEquation>, children: &mut HashSet<PortNumber>, 
				other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry, Error> {
        let f = "update_traph";
		let (entry, is_new_port) = {
			let mut traphs = self.traphs.lock().unwrap();
			let traph = match traphs.entry(base_tree_id.get_uuid()) { // Using entry voids lifetime problem
				Entry::Occupied(t) => t.into_mut(),
				Entry::Vacant(v) => {
					//println!("Cell {}: update traph {} {}", self.cell_id, base_tree_id, base_tree_id.get_uuid());
					self.tree_map.lock().unwrap().insert(base_tree_id.get_uuid(), base_tree_id.get_uuid());
					self.tree_id_map.lock().unwrap().insert(base_tree_id.get_uuid(), base_tree_id.clone());
					let index = self.clone().use_index().context(CellagentError::Chain { func_name: f, comment: S("") })?;
					let t = Traph::new(&self.cell_id, &base_tree_id, index, gvm_eqn).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
					v.insert(t)
				}
			};
			let (gvm_recv, gvm_send, gvm_xtnd, gvm_save) = match gvm_eqn {
				Some(eqn) => {
					let variables = traph.get_params(eqn.get_variables()).context(CellagentError::Chain { func_name: "update_traph", comment: S("") })?;
					let recv = eqn.eval_recv(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_recv") })?;
					let send = eqn.eval_send(&variables).context(CellagentError::Chain { func_name: f, comment: S("eval_send") })?;
					let xtnd = eqn.eval_xtnd(&variables).context(CellagentError::Chain { func_name: "f", comment: S("eval_xtnd") })?;
					let save = eqn.eval_save(&variables).context(CellagentError::Chain { func_name: "f", comment: S("eval_save") })?;
					(recv, send, xtnd, save)
				},
				None => (false, false, false, false),
			};
			let (hops, path) = match port_status {
				traph::PortStatus::Child => {
					let element = traph.get_parent_element().context(CellagentError::Chain { func_name: "f", comment: S("") })?;
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
			//println!("CellAgent {}: entry {}", self.cell_id, entry);
			// Need traph even if cell only forwards on this tree
			self.trees.lock().unwrap().insert(entry.get_index(), base_tree_id.clone());
			self.ca_to_pe.send(CaToPePacket::Entry(entry)).context(CellagentError::Chain { func_name: "f", comment: S("") })?;
			// Update entries for stacked trees
			let entries = traph.update_stacked_entries(entry).context(CellagentError::Chain { func_name: "f", comment: S("") })?;
			for entry in entries {
				//println!("Cell {}: sending entry {}", self.cell_id, entry);
				self.ca_to_pe.send(CaToPePacket::Entry(entry)).context(CellagentError::Chain { func_name: "f", comment: S("") })?;
			}
            (entry, is_new_port)
		};
        // Forward saved messages whenever the new port is added to traph
        if is_new_port { self.forward_saved(base_tree_id, entry.get_mask()).context(CellagentError::Chain { func_name: "f", comment: S("") })? }
		Ok(entry)
	}
	pub fn get_traph(&self, base_tree_id: &TreeID) -> Result<Traph, CellagentError> {
		let mut locked = self.traphs.lock().unwrap();
		let uuid = base_tree_id.get_uuid();
		match locked.entry(uuid) {
			Entry::Occupied(o) => Ok(o.into_mut().clone()),
			Entry::Vacant(_) => Err(CellagentError::NoTraph { cell_id: self.cell_id.clone(), func_name: "stack_tree", tree_uuid: uuid })
		}
	}
	pub fn deploy(&mut self, port_no: PortNo, msg_tree_id: &TreeID, msg_tree_map: &MsgTreeMap,
                  manifest: &Manifest) -> Result<(), Error> {
		println!("Cell {}: got manifest on tree {} {}", self.cell_id, msg_tree_id, manifest);
		let deployment_tree = manifest.get_deployment_tree();
        println!("Cell {}: deployment tree {}, msg tree {}", self.cell_id, deployment_tree, msg_tree_id);
        if deployment_tree.get_name() != msg_tree_id.get_name() { return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy", tree_name: deployment_tree.clone() }.into()); }
		let mut tree_name_map = match self.tree_name_map.get(  msg_tree_id).cloned() {
			Some(map) => map,
			None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(map)", tree_name: deployment_tree.clone() }.into())
		};
        for allowed_tree in manifest.get_allowed_trees() {
            match msg_tree_map.get(allowed_tree) {
                Some(tree_id) => tree_name_map.insert(allowed_tree.clone(), tree_id.clone()),
                None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(map)", tree_name: allowed_tree.clone() }.into())
            };
        }
		for vm_spec in manifest.get_vms() {
			let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
			let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
            let vm_id = VmID::new(&self.cell_id, &vm_spec.get_id())?;
            let vm_allowed_trees = vm_spec.get_allowed_trees();
            let mut trees = HashSet::new();
            trees.insert(AllowedTree::new(CONTROL_TREE_NAME));
            for vm_allowed_tree in vm_allowed_trees {
                match tree_name_map.get(vm_allowed_tree) {
                    Some(allowed_tree_id) =>{
                        trees.insert(vm_allowed_tree.to_owned());
                        match self.tree_vm_map.clone().get_mut(allowed_tree_id) {
                            Some(senders) => senders.push(ca_to_vm.clone()),
                            None => { self.tree_vm_map.insert(allowed_tree_id.to_owned(), vec![ca_to_vm.clone()]); }
                        }
                    },
                    None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(vm)", tree_name: vm_allowed_tree.clone() }.into())
                }
            }
            let container_specs = vm_spec.get_containers();
            let mut vm = VirtualMachine::new(&vm_id, vm_to_ca, vm_allowed_trees);
            let up_tree_name = vm_spec.get_id();
            println!("Cell {} starting VM on up tree {}", self.cell_id, up_tree_name);
            vm.initialize(up_tree_name, vm_from_ca, &trees, container_specs)?;
            //self.ca_to_vms.insert(vm_id, ca_to_vm,);
            //self.listen_uptree(&up_tree_id, vm.get_id(), &trees, ca_from_vm)?;
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
	fn listen_uptree(&self, tree_id_ref: &TreeID, vm_id_ref: &VmID, trees: &HashSet<AllowedTree>, ca_from_vm: CaFromVm)
            -> Result<(), Error> {
		let ca = self.clone();
		let vm_id = vm_id_ref.clone();
		let tree_id = tree_id_ref.clone(); // Needed for lifetime under spawn
        let tree_name_map = self.tree_name_map.get(tree_id_ref).unwrap().clone();
		::std::thread::spawn( move || -> Result<(), Error> {
		loop {
			//println!("CellAgent {}: listening to vm {} on tree {}", ca.cell_id, vm_id, tree_id);
			let (tree, msg) = ca_from_vm.recv()?;
            let foo = tree_name_map.get(&AllowedTree::new(&tree)).unwrap();
			println!("CellAgent {}: got vm msg {} for tree {} on tree {}", ca.cell_id, tree, msg, tree_id);
		}	
		});
		Ok(())
	}
	fn create_tree(&mut self, id: &str, target_tree_id: &TreeID, port_no_mask: Mask, gvm_eqn: &GvmEquation)
            -> Result<(), Error> {
        let new_id = self.my_tree_id.add_component(id)?;
        let new_tree_id = TreeID::new(new_id.get_name())?;
        new_tree_id.append2file()?;
        let ref my_tree_id = self.my_tree_id.clone(); // Need because self is borrowed mut
        let msg =  StackTreeMsg::new(&new_tree_id, &self.my_tree_id,&gvm_eqn);
        self.send_msg(target_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.cell_id.clone())})?;
        self.stack_tree(&new_tree_id, &my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "create_tree", comment: S(self.cell_id.clone())})?;
        Ok(())
	}	
	pub fn stack_tree(&mut self, new_tree_id: &TreeID, parent_tree_id: &TreeID,
			gvm_eqn: &GvmEquation) -> Result<(), Error> {
        //println!("Cell {}: base tree map {:?}", self.cell_id, self.base_tree_map);
        let base_tree_id = match self.base_tree_map.get(parent_tree_id).cloned() {
            Some(id) => id,
            None => {
                self.base_tree_map.insert(new_tree_id.clone(), parent_tree_id.clone());
                parent_tree_id.clone()
            }
        };
		let mut traph = self.get_traph(&base_tree_id).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
		if traph.has_tree(new_tree_id) { return Ok(()); } // Check for redundant StackTreeMsg
		let parent_entry = traph.get_tree_entry(&parent_tree_id.get_uuid()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
		let mut entry = parent_entry.clone();
		let index = self.use_index().context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
		entry.set_table_index(index);
		entry.set_uuid(&new_tree_id.get_uuid());
		let params = traph.get_params(gvm_eqn.get_variables()).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
        let gvm_xtnd = gvm_eqn.eval_xtnd(&params).context(CellagentError::Chain { func_name: "stack_tree", comment: S("gvm_xtnd")})?;
        let gvm_send = gvm_eqn.eval_send(&params).context(CellagentError::Chain { func_name: "stack_tree", comment: S("gvm_send")})?;
        if !gvm_xtnd { entry.clear_children(); }
        if gvm_send  { entry.enable_send(); } else { entry.disable_send(); }
        let gvm_recv = gvm_eqn.eval_recv(&params).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
		let mask = if gvm_recv { entry.get_mask().or(Mask::new0()) }
                          else        { entry.get_mask().and(Mask::all_but_zero(self.no_ports)) };
        entry.set_mask(mask);
        let tree = Tree::new(&new_tree_id, &base_tree_id, Some(&gvm_eqn), entry);
		//println!("Cell {}: stack tree {} {}", self.cell_id, new_tree_id, new_tree_id.get_uuid());
		traph.stack_tree(tree);
		self.tree_map.lock().unwrap().insert(new_tree_id.get_uuid(), base_tree_id.get_uuid());
		self.tree_id_map.lock().unwrap().insert(new_tree_id.get_uuid(), new_tree_id.clone());
		self.ca_to_pe.send(CaToPePacket::Entry(entry)).context(CellagentError::Chain { func_name: "stack_tree", comment: S("")})?;
		Ok(())
	}
	fn listen_pe(&mut self, ca_from_pe: CaFromPe) -> Result<(), Error>{
		let mut ca = self.clone();
		::std::thread::spawn( move || { 
			let _ = ca.listen_pe_loop(ca_from_pe).map_err(|e| ::utility::write_err("cellagent", e));
		});
		Ok(())
	}
	fn listen_pe_loop(&mut self, ca_from_pe: CaFromPe) -> Result<(), Error> {
        let f = "listen_pe_loop";
		loop {
			//println!("CellAgent {}: waiting for status or packet", ca.cell_id);
			match ca_from_pe.recv().context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})? {
				PeToCaPacket::Status((port_no, is_border, status)) => match status {
					port::PortStatus::Connected => self.port_connected(port_no, is_border)?,
					port::PortStatus::Disconnected => self.port_disconnected(port_no)?
				},
				PeToCaPacket::Packet((port_no, old_mask, index, packet)) => {
                    //println!("Cellagent other_index {}", *index);
					let tree_id = match self.trees.lock().unwrap().get(&index).cloned() {
						Some(t) => t,
						None => return Err(CellagentError::TreeIndex { cell_id: self.cell_id.clone(), func_name: "get_tree_id_from_index", index: index }.into())
					};
					let msg_id = packet.get_header().get_msg_id();
					let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
					// I hope I can remove the tree UUID from the packet header to save bits
					let tree_uuid = tree_id.get_uuid(); 
					let (last_packet, packets) = packet_assembler.add(packet);
					if last_packet {
						let mut msg = MsgType::get_msg(&packets).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
                        //if msg.get_msg_type() != MsgType::Discover { println!("Cell {}: Port {} old_mask {}, received {}", self.cell_id, *port_no, old_mask, msg); }
                        msg.process_ca(self, &tree_id, port_no).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
						let save = self.gvm_eval_save(tree_uuid, &msg).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
						if save && (tree_uuid != self.connected_tree_id.get_uuid()) {
                            let msg_type = msg.get_msg_type();
                            if msg_type == MsgType::Discover { self.add_saved_discover(packets); }
                            else { // Any msg type that can be saved must implement get_parent_tree_id()
                                let parent_tree_id = match msg_type {
                                    MsgType::StackTree => msg.get_payload_stack_tree()?.get_parent_tree_id(),
                                    _ => return Err(CellagentError::SavedMsgType { func_name: f, msg_type: msg_type }.into())
                                };
                                let base_tree_id = match self.base_tree_map.get(parent_tree_id).cloned() {
                                    Some(id) => id,
                                    None => parent_tree_id.clone()
                                };
                                self.base_tree_map.insert(parent_tree_id.clone(), base_tree_id.clone());
                                self.add_saved_msg(&base_tree_id, old_mask, packets);
                                let base_tree_uuid = base_tree_id.get_uuid();
                                let traph = self.get_traph(&base_tree_id).context(CellagentError::Chain { func_name: f, comment: S("") })?;
                                let entry = traph.get_tree_entry(&base_tree_uuid).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
                                self.forward_saved(&base_tree_id, entry.get_mask()).context(CellagentError::Chain { func_name: f, comment: S("") })?;
                            }
                        }
					} else {
						let assembler = PacketAssembler::create(msg_id, packets);
						self.packet_assemblers.insert(msg_id, assembler);
					}
				},
                PeToCaPacket::Tcp((port_no, (msg_type, serialized))) => {
                    let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " PortNumber" })?;
                    let border_tree_id = match self.border_port_tree_id_map.get(&port_number).cloned() {
                        Some(id) => id,
                        None => return Err(CellagentError::Border { func_name: "listen_pe_loop 2", cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    };
                    let ref mut tree_map = match self.tree_name_map.get(&border_tree_id).cloned() {
                        Some(map) => map,
                        None => return Err(CellagentError::Border { func_name: "listen_pe_loop 1", cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    };
                    match msg_type {
                        TcpMsgType::Application => self.tcp_application(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_application")})?,
                        TcpMsgType::DeleteTree  => self.tcp_delete_tree(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_delete_tree")})?,
                        TcpMsgType::Manifest    => self.tcp_manifest(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_manifest")})?,
                        TcpMsgType::Query       => self.tcp_query(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_query")})?,
                        TcpMsgType::StackTree   => self.tcp_stack_tree(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_stack_tree")})?,
                        TcpMsgType::TreeName    => self.tcp_tree_name(&serialized, tree_map).context(CellagentError::Chain { func_name: "f", comment: S("tcp_tree_name")})?,
                    };
                    self.tree_name_map.insert(border_tree_id.clone(), tree_map.clone());
                }
			}
		}
	}
    fn tcp_application(&self, serialized: &String, tree_map: &HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
        let f = "tcp_application";
        Err(UtilityError::Unimplemented { func_name: f, feature: S("TcpMsgType::Application")}.into())
    }
    fn tcp_delete_tree(&self, serialized: &String, tree_map: &HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
        let f = "tcp_delete_tree";
        Err(UtilityError::Unimplemented { func_name: f, feature: S("TcpMsgType::Application")}.into())
    }
    fn tcp_manifest(&self, serialized: &String, tree_map: &HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
        let f = "tcp_manifest";
        let msg = serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " deserialize StackTree" })?;
        let ref deploy_tree_name = self.get_msg_params(&msg, "deploy_tree_name").context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " parent tree name" })?;
        let deploy_tree_id = match tree_map.get(&AllowedTree::new(deploy_tree_name)) {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { func_name: "listen_pe_loop 4", cell_id: self.cell_id.clone(), tree_name: AllowedTree::new(deploy_tree_name) }.into())
        };
        let manifest_ser = self.get_msg_params(&msg, "manifest").context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " manifest" })?;
        let manifest = serde_json::from_str::<Manifest>(&manifest_ser)?;
        let allowed_trees = vec![];//manifest.get_allowed_trees();
        let mut msg_tree_map = HashMap::new();
        for allowed_tree in allowed_trees {
            match tree_map.get(allowed_tree) {
                Some(tree_id) => msg_tree_map.insert(allowed_tree.clone(), tree_id.clone()),
                None => return Err(CellagentError::TreeMap { func_name: "listen_pe_loop 5", cell_id: self.cell_id.clone(), tree_name: allowed_tree.clone() }.into())
            };
        }
        println!("Cell {} deploy on tree {} {} {}", self.cell_id, deploy_tree_id, deploy_tree_id.get_uuid(), manifest);
        let msg = ManifestMsg::new(&msg_tree_map, &manifest);
        self.send_msg(deploy_tree_id, &msg, DEFAULT_USER_MASK.or(Mask::new0())).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " send manifest" })?;
        Ok(())
    }
    fn tcp_query(&self, serialized: &String, tree_map: &HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
        let f = "tcp_query";
        Err(UtilityError::Unimplemented { func_name: f, feature: S("TcpMsgType::Application")}.into())
    }
    fn tcp_stack_tree(&mut self, serialized: &String, tree_map: &mut HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
        let f = "tcp_stack_tree";
        let msg = serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " deserialize StackTree" })?;
        let parent_tree_str = self.get_msg_params(&msg, "parent_tree_name")?;
        let parent_tree_name = AllowedTree::new(&parent_tree_str);
        let ref parent_tree_id = match tree_map.get(&parent_tree_name).cloned() {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { func_name: "listen_pe_loop 3", cell_id: self.cell_id.clone(), tree_name: parent_tree_name }.into())
        };
        let ref my_tree_id = self.my_tree_id.clone();
        let new_tree_name = self.get_msg_params(&msg, "new_tree_name")?;
        let ref new_tree_id = self.my_tree_id.add_component(&new_tree_name).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " new_tree_id" })?;
        tree_map.insert(AllowedTree::new(&new_tree_name), new_tree_id.clone());
        println!("Cellagent {}: new tree id {} {}", self.cell_id, new_tree_id, new_tree_id.get_uuid());
        let gvm_eqn_serialized = self.get_msg_params(&msg, "gvm_eqn")?;
        let ref gvm_eqn = serde_json::from_str::<GvmEquation>(&gvm_eqn_serialized).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " gvm" })?;
        self.stack_tree(new_tree_id, parent_tree_id, gvm_eqn).context(CellagentError::Chain { func_name: f, comment: S("stack tree")})?;
        let stack_tree_msg = StackTreeMsg::new(new_tree_id, parent_tree_id, gvm_eqn);
        let packets = stack_tree_msg.to_packets(parent_tree_id).context(CellagentError::Chain { func_name: f, comment: S("get packets") })?;
        self.add_saved_msg(my_tree_id, Mask::empty(), &packets);
        //println!("Cellagent {}: Sending with old_mask {} msg {}", self.cell_id, DEFAULT_USER_MASK, stack_tree_msg);
        self.send_msg(parent_tree_id, &stack_tree_msg, DEFAULT_USER_MASK).context(CellagentError::Chain { func_name: f, comment: S(self.cell_id.clone()) + " send_msg" })?;
        Ok(())
    }
    fn tcp_tree_name(&self, serialized: &String, tree_map: &HashMap<AllowedTree, TreeID>) -> Result<(), Error> {
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
	fn gvm_eval_save(&self, tree_uuid: Uuid, msg: &Box<Message>) -> Result<bool, Error> {
		let f = "gvm_eval_save";
		if let Some(gvm_eqn) = msg.get_gvm_eqn() {
			//let locked = self.tree_id_map.lock().unwrap();
			//println!("Cell {}: tree_uuid {}", self.cell_id, tree_uuid);
			if let Some(base_tree_uuid) = self.tree_map.lock().unwrap().get(&tree_uuid) {
				//println!("Cell {}: base_tree_uuid {}", self.cell_id, base_tree_uuid);
				let mut locked = self.traphs.lock().unwrap();
				let traph = match locked.entry(*base_tree_uuid) {
					Entry::Occupied(t) => t.into_mut(),
					Entry::Vacant(_) => return Err(CellagentError::Tree { cell_id: self.cell_id.clone(), func_name: f, tree_uuid: base_tree_uuid.clone() }.into())
				};
				let params = traph.get_params(gvm_eqn.get_variables())?;
				let save = gvm_eqn.eval_save(&params)?;
				Ok(save)
			} else {
				Ok(false) // Should be only for messages sent on control tree
			}
		} else {
			Ok(false)
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
	fn port_connected(&mut self, port_no: PortNo, is_border: bool) -> Result<(), Error> {
		//println!("CellAgent {}: port {} is border {} connected", self.cell_id, *port_no, is_border);
		if is_border {
			//println!("CellAgent {}: port {} is a border port", self.cell_id, *port_no);
			// Create tree to talk to outside
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("true"));
			eqns.insert(GvmEqn::Xtnd("false"));
			eqns.insert(GvmEqn::Save("false"));
			let gvm_eqn = GvmEquation::new(eqns, Vec::new());
			let new_tree_id = self.my_tree_id.add_component("Noc").context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			let entry = self.update_traph(&new_tree_id, port_number, traph::PortStatus::Parent,
				Some(&gvm_eqn), &mut HashSet::new(), TableIndex(0), PathLength(CellNo(1)), None).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            let mut tree_map = HashMap::new();
            let base_tree = AllowedTree::new("Base");
            tree_map.insert(base_tree.clone(), self.my_tree_id.clone());
			self.tree_name_map.insert(new_tree_id.clone(),tree_map);
            self.border_port_tree_id_map.insert(port_number, new_tree_id.clone());
			let port_no_mask = Mask::new(port_number);
			let tree_name_msg = TreeNameMsg::new(&base_tree.get_name());
            let serialized = serde_json::to_string(&tree_name_msg).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
            self.ca_to_pe.send(CaToPePacket::Tcp((port_number, (TcpMsgType::TreeName, serialized)))).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			//println!("Cell {}: Sending on ports {}: {}", self.cell_id, port_no_mask, tree_name_msg);
            Ok(())
		} else {
			//println!("Cell {}: port {} connected", self.cell_id, *port_no);
			let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?);
			let path = Path::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
			let hops = PathLength(CellNo(1));
			let my_table_index = self.my_entry.get_index();
			let discover_msg = DiscoverMsg::new(&self.my_tree_id, my_table_index, &self.cell_id, hops, path);
			//println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, discover_msg);
			let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
			self.ca_to_pe.send(entry).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			self.send_msg(&self.connected_tree_id, &discover_msg, port_no_mask).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			let saved_discover  = self.get_saved_discover();
            self.forward_discover(&saved_discover, port_no_mask).context(CellagentError::Chain { func_name: "port_connected", comment: S(self.cell_id.clone()) })?;
			Ok(())
		}
	}
	fn port_disconnected(&self, port_no: PortNo) -> Result<(), Error> {
		//println!("Cell Agent {} got disconnected on port {}", self.cell_id, port_no);
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
		self.connected_tree_entry.lock().unwrap().and_with_mask(port_no_mask.not());
		let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
		self.ca_to_pe.send(entry)?;
		Ok(())	
	}		
	fn forward_discover(&mut self, saved_discover: &Vec<SavedDiscover>, mask: Mask) -> Result<(), Error> {
		//println!("Cell {}: forwarding {} discover msgs", self.cell_id, saved_msgs.len());
		for packets in saved_discover.iter() {
			self.send_packets(self.connected_tree_id.get_uuid(), packets, mask)?;
            //let msg = MsgType::get_msg(&packets)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	fn forward_saved(&self, base_tree_id: &TreeID, current_mask: Mask) -> Result<(), Error> {
        match self.get_saved_msgs(base_tree_id) {
            Some(saved_msgs) => {
                for &(old_mask, ref packets) in saved_msgs.iter() {
                    let update_mask = old_mask.not().and(current_mask);
                    //println!("Cell {}: forwarding message with masks current {}, entry {}, send {}", self.cell_id, current_mask, old_mask, update_mask);
                    if !Mask::empty().equal(update_mask) {
                        let msg = MsgType::get_msg(&packets)?;
                        //println!("Cell {}: forward {}", self.cell_id, msg.get_msg_type());
                        let uuid = packets[0].get_tree_uuid();
                        self.send_packets(uuid, packets, update_mask)?;
                        //println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
                    }
                }
            }
            None => {}
        }
		Ok(())	
	}
    pub fn send_msg<T>(&self, tree_id: &TreeID, msg: &T, user_mask: Mask) -> Result<Vec<Packet>, Error>
        where T: Message + ::std::marker::Sized + serde::Serialize
    {
        let packets = msg.to_packets(tree_id).context(CellagentError::Chain { func_name: "send_msg", comment: S(self.cell_id.clone()) })?;
        self.send_packets(tree_id.get_uuid(), &packets, user_mask).context(CellagentError::Chain { func_name: "send_msg", comment: S(self.cell_id.clone()) })?;
        Ok(packets)
    }
	fn send_packets(&self, tree_uuid: Uuid, packets: &Vec<Packet>, user_mask: Mask) -> Result<(), Error> {
		let f = "send_packets";
		let base_tree_uuid = match self.tree_map.lock().unwrap().get(&tree_uuid).cloned() {
            Some(id) => id,
            None => return Err(CellagentError::Tree { func_name: f, cell_id: self.cell_id.clone(), tree_uuid: tree_uuid }.into())
        };
        let index = match self.traphs.lock().unwrap().get(&base_tree_uuid) {
            Some(traph) => traph.get_table_index(&tree_uuid).context(CellagentError::Chain { func_name: "send_packets", comment: S("")})?,
            None => return Err(CellagentError::NoTraph { cell_id: self.cell_id.clone(), func_name: f, tree_uuid: tree_uuid }.into())
        };
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packet);
			let msg = CaToPePacket::Packet((index, user_mask, *packet));
			self.ca_to_pe.send(msg)?;
			//println!("CellAgent {}: sent packet {} on tree {} to packet engine with index {}", self.cell_id, packet_count, tree_id, index);
		}
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
    #[fail(display = "CellAgentError::BorderMsgType {}: Message type {} is not accepted from a border port on cell {}", func_name, msg_type, cell_id)]
    BorderMsgType { func_name: &'static str, cell_id: CellID, msg_type: MsgType },
    #[fail(display = "CellagentError::ManifestVms {}: No VMs in manifest for cell {}", func_name, cell_id)]
    ManifestVms { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellagentError::Message {}: Malformed request {:?} from border port on cell {}", func_name, msg, cell_id)]
    Message { func_name: &'static str, cell_id: CellID, msg: HashMap<String, String> },
    #[fail(display = "CellAgentError::NoTraph {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    NoTraph { cell_id: CellID, func_name: &'static str, tree_uuid: Uuid },
    #[fail(display = "CellagentError::SavedMsgType {}: Message type {} does not support saving", func_name, msg_type)]
    SavedMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "CellAgentError::Size {}: No more room in routing table for cell {}", func_name, cell_id)]
    Size { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellAgentError::StackTree {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)]
    StackTree { func_name: &'static str, tree_id: TreeID, cell_id: CellID },
    #[fail(display = "CellagentError::TcpMessageType {}: Unsupported request {:?} from border port on cell {}", func_name, msg, cell_id)]
    TcpMessageType { func_name: &'static str, cell_id: CellID, msg: TcpMsgType },
    #[fail(display = "CellAgentError::TenantMask {}: Cell {} has no tenant mask", func_name, cell_id)]
    TenantMask { func_name: &'static str, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeMap {}: Cell {} has no tree map entry for {}", func_name, cell_id, tree_name)]
    TreeMap { func_name: &'static str, cell_id: CellID, tree_name: AllowedTree },
    #[fail(display = "CellAgentError::Tree {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid },
    #[fail(display = "CellAgentError::TreeIndex {}: No tree associated with index {:?} on cell {}", func_name, index, cell_id)]
    TreeIndex { func_name: &'static str, index: TableIndex, cell_id: CellID }
}
