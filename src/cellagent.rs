use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use uuid::Uuid;

use config::{BASE_TREE_NAME, CONNECTED_PORTS_TREE_NAME, CONTROL_TREE_NAME, MAX_ENTRIES, CellNo, CellType, PathLength, PortNo, TableIndex};
use gvm_equation::{GvmEquation, GvmEqn};
use message::{Message, MsgType, DiscoverMsg, StackTreeMsg, TreeNameMsg};
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
use utility::{BASE_TENANT_MASK, Mask, Path, PortNumber, S, write_err};
use vm::VirtualMachine;

use failure::{Error, Fail, ResultExt};

type BorderTreeIDMap = HashMap<PortNumber, TreeID>;
pub type Traphs = HashMap<Uuid, Traph>;
pub type Trees = HashMap<TableIndex, TreeID>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeIDMap = HashMap<Uuid, TreeID>;
pub type TreeNameMap = HashMap<TreeID, HashMap<AllowedTree, TreeID>>;
pub type SavedMsg = Vec<Packet>;

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
	saved_msgs: Arc<Mutex<Vec<SavedMsg>>>,
	saved_discover: Arc<Mutex<Vec<SavedMsg>>>,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<Trees>>,
	traphs: Arc<Mutex<Traphs>>,
	tree_map: Arc<Mutex<TreeMap>>,
	tree_name_map: TreeNameMap,
    border_port_tree_id_map: BorderTreeIDMap,
	tree_id_map: Arc<Mutex<TreeIDMap>>, // For debugging
	tenant_masks: Vec<Mask>,
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
			control_tree_id: control_tree_id, connected_tree_id: connected_tree_id,	
			no_ports: no_ports, traphs: traphs, vm_id_no: 0, tree_id_map: Arc::new(Mutex::new(HashMap::new())),
			free_indices: Arc::new(Mutex::new(free_indices)), tree_map: Arc::new(Mutex::new(HashMap::new())),
			tree_name_map: HashMap::new(), ca_to_vms: HashMap::new(), border_port_tree_id_map: HashMap::new(),
			saved_msgs: Arc::new(Mutex::new(Vec::new())), saved_discover: Arc::new(Mutex::new(Vec::new())),
			my_entry: RoutingTableEntry::default(TableIndex(0))?, 
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
	pub fn get_saved_msgs(&self) -> Vec<SavedMsg> {
		self.saved_msgs.lock().unwrap().to_vec()
	}
	pub fn get_saved_discover(&self) -> Vec<SavedMsg> {
		self.saved_msgs.lock().unwrap().to_vec()
	}
	pub fn add_saved_msg(&mut self, packets: &SavedMsg) -> Vec<SavedMsg> {
		{ 
			let mut saved_msgs = self.saved_msgs.lock().unwrap();
			//let msg = MsgType::get_msg(&packets).unwrap();
			//println!("Cell {}: save generic {}", self.cell_id, msg);
			saved_msgs.push(packets.clone());
		}
		self.get_saved_msgs()
	}
	pub fn add_saved_discover(&mut self, packets: &SavedMsg) -> Vec<SavedMsg> {
		{
			let mut saved_discover = self.saved_msgs.lock().unwrap();
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
	pub fn update_traph(&mut self, black_tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				gvm_eqn: Option<&GvmEquation>, children: &mut HashSet<PortNumber>, 
				other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry, Error> {
		let entry = {
			let mut traphs = self.traphs.lock().unwrap();
			let traph = match traphs.entry(black_tree_id.get_uuid()) { // Avoids lifetime problem
				Entry::Occupied(t) => t.into_mut(),
				Entry::Vacant(v) => {
					//println!("Cell {}: update traph {} {}", self.cell_id, black_tree_id, black_tree_id.get_uuid());
					self.tree_map.lock().unwrap().insert(black_tree_id.get_uuid(), black_tree_id.get_uuid());
					self.tree_id_map.lock().unwrap().insert(black_tree_id.get_uuid(), black_tree_id.clone());
					let index = self.clone().use_index()?;
					let t = Traph::new(&self.cell_id, &black_tree_id, index, gvm_eqn)?;
					v.insert(t)
				}
			};
			let (gvm_recv, gvm_send, gvm_xtnd, gvm_save) = match gvm_eqn {
				Some(eqn) => {
					let variables = traph.get_params(eqn.get_variables())?;
					let recv = eqn.eval_recv(&variables)?;
					let send = eqn.eval_send(&variables)?;
					let xtnd = eqn.eval_xtnd(&variables)?; 
					let save = eqn.eval_save(&variables)?; 
					(recv, send, xtnd, save)
				},
				None => (false, false, false, false),
			};
			let (hops, path) = match port_status {
				traph::PortStatus::Child => {
					let element = traph.get_parent_element()?;
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
			let mut entry = traph.new_element(black_tree_id, port_number, port_status, other_index, children, hops, path)?; 
			if gvm_send { entry.enable_send() } else { entry.disable_send() }
			//println!("CellAgent {}: entry {}", self.cell_id, entry); 
			// Need traph even if cell only forwards on this tree
			self.trees.lock().unwrap().insert(entry.get_index(), black_tree_id.clone());
			self.ca_to_pe.send(CaToPePacket::Entry(entry))?;
			// Update entries for stacked trees
			let entries = traph.update_stacked_entries(entry)?;
			for entry in entries {
				//println!("Cell {}: sending entry {}", self.cell_id, entry);
				self.ca_to_pe.send(CaToPePacket::Entry(entry))?;			
			}
			entry
		};
		let saved_msgs  = self.get_saved_msgs();
		self.forward_saved(&saved_msgs, Mask::new(port_number))?;		
		Ok(entry)
	}
	pub fn get_traph(&self, black_tree_id: &TreeID) -> Result<Traph, CellagentError> {
		let mut locked = self.traphs.lock().unwrap();
		let uuid = black_tree_id.get_uuid();
		match locked.entry(uuid) {
			Entry::Occupied(o) => Ok(o.into_mut().clone()),
			Entry::Vacant(_) => Err(CellagentError::NoTraph { cell_id: self.cell_id.clone(), func_name: "stack_tree", tree_uuid: uuid })
		}		
	}
	pub fn deploy(&mut self, port_no: PortNo, msg_tree_id: &TreeID, manifest: &Manifest) -> Result<(), Error> {
		println!("Cell {}: got manifest on tree {} {}", self.cell_id, msg_tree_id, manifest);
		let deployment_tree = AllowedTree::new(manifest.get_deployment_tree_name());
		let mut tree_name_map = match self.tree_name_map.get(  msg_tree_id).cloned() {
			Some(map) => map,
			None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(map)", tree_name: S(msg_tree_id.get_name()) }.into())
		};
        //let mut tree_name_map = result?;
        let deployment_tree_id = match tree_name_map.get(&deployment_tree).cloned() {
            Some(id) => id,
            None => return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(id)", tree_name: S(deployment_tree.get_name()) }.into())
        };
        let new_tree_name = manifest.get_new_tree_name();
        let up_tree_id = msg_tree_id.add_component(manifest.get_id()).context("UptreeID")?;
        println!("Deploy {} uptree {} on tree {}", new_tree_name, up_tree_id, deployment_tree_id);
        tree_name_map.insert(AllowedTree::new(new_tree_name), up_tree_id.clone());
        self.stack_uptree(&up_tree_id, &deployment_tree_id, port_no, manifest.get_gvm())?;
		self.tree_name_map.insert(up_tree_id.clone(), tree_name_map.clone());
        //for k in self.tree_name_map.keys() { println!("{} {:?}", k, self.tree_name_map.get(k).unwrap()); }
        //println!("tree_name_map {:?}", self.tree_name_map);
		for vm_spec in manifest.get_vms() {
			let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
			let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
            let vm_id = VmID::new(&self.cell_id, &vm_spec.get_id())?;
            let allowed_trees = vm_spec.get_allowed_trees();
            let mut trees = HashSet::new();
            trees.insert(AllowedTree::new(CONTROL_TREE_NAME));
            for allowed_tree in allowed_trees {
                if tree_name_map.contains_key(allowed_tree) {
                    trees.insert(allowed_tree.to_owned())
                } else {
                    return Err(CellagentError::TreeMap { cell_id: self.cell_id.clone(), func_name: "deploy(vm)", tree_name: S(allowed_tree.get_name()) }.into());
                };
            }
            let container_specs = vm_spec.get_containers();
            let mut vm = VirtualMachine::new(&vm_id, vm_to_ca, allowed_trees);
            vm.initialize(&up_tree_id, vm_from_ca,&trees, container_specs)?;
            self.ca_to_vms.insert(vm_id, ca_to_vm,);
            self.listen_uptree(&up_tree_id, vm.get_id(), &trees, ca_from_vm)?;
		}
		Ok(())
	}
    fn stack_uptree(&mut self, up_tree_id: &TreeID, deployment_tree_id: &TreeID, port_no: PortNo, gvm_eqn: &GvmEquation) -> Result<(), Error> {
        let ref my_tree_id = self.my_tree_id.clone(); // Need to clone because self is borrowed mut
        let msg= StackTreeMsg::new(up_tree_id, &self.my_tree_id, gvm_eqn);
        let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "stack_uptree", comment: ""})?;
        let port_no_mask = Mask::all_but_zero(self.no_ports).and(Mask::new(port_number));
        self.send_msg(&deployment_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "stack_uptree", comment: ""})?;
        self.stack_tree(&up_tree_id, &my_tree_id, my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "stack_uptree", comment: ""})?;
        let mut tree_name_map = HashMap::new();
        tree_name_map.insert(AllowedTree::new(up_tree_id.get_name()),up_tree_id.clone());
        self.tree_name_map.insert(up_tree_id.clone(), tree_name_map);
        Ok(())
    }
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
        let msg =  StackTreeMsg::new(&new_tree_id, &self.my_tree_id, &gvm_eqn);
        self.send_msg(target_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "create_tree", comment: ""})?;
        self.stack_tree(&new_tree_id, &my_tree_id, my_tree_id, &gvm_eqn).context(CellagentError::Chain { func_name: "create_tree", comment: ""})?;
        Ok(())
	}	
	pub fn stack_tree(&mut self, new_tree_id: &TreeID, parent_tree_id: &TreeID,
			black_tree_id: &TreeID, gvm_eqn: &GvmEquation) -> Result<(), Error> {
		let mut traph = self.get_traph(black_tree_id)?;
		if traph.has_tree(new_tree_id) { return Ok(()); } // Check for redundant StackTreeMsg
		let parent_entry = traph.get_tree_entry(&parent_tree_id.get_uuid())?;
		let mut entry = parent_entry.clone();
		let index = self.use_index()?; 
		entry.set_table_index(index);
		entry.set_uuid(&new_tree_id.get_uuid());
		let params = traph.get_params(gvm_eqn.get_variables())?;
		if !gvm_eqn.eval_recv(&params)? { 
			let mask = entry.get_mask().and(Mask::all_but_zero(self.no_ports));
			entry.set_mask(mask);
		}
		if !gvm_eqn.eval_xtnd(&params)? { entry.clear_children(); }
		if gvm_eqn.eval_send(&params)? { entry.enable_send(); } else { entry.disable_send(); }
		let tree = Tree::new(&new_tree_id, black_tree_id, Some(&gvm_eqn), entry);
		//println!("Cell {}: stack tree {} {}", self.cell_id, new_tree_id, new_tree_id.get_uuid());
		traph.stack_tree(&tree);
		self.tree_map.lock().unwrap().insert(new_tree_id.get_uuid(), black_tree_id.get_uuid());
		self.tree_id_map.lock().unwrap().insert(new_tree_id.get_uuid(), new_tree_id.clone());
		self.ca_to_pe.send(CaToPePacket::Entry(entry))?;
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
		loop {
			//println!("CellAgent {}: waiting for status or packet", ca.cell_id);
			match ca_from_pe.recv()? {
				PeToCaPacket::Status((port_no, is_border, status)) => match status {
					port::PortStatus::Connected => self.port_connected(port_no, is_border)?,
					port::PortStatus::Disconnected => self.port_disconnected(port_no)?
				},
				PeToCaPacket::Packet((port_no, index, packet)) => {
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
						let mut msg = MsgType::get_msg(&packets)?;
                        msg.process_ca(self, &tree_id, port_no)?;
						let save = self.gvm_eval_save(tree_uuid, msg)?;
						if save && (tree_uuid != self.connected_tree_id.get_uuid()) { self.add_saved_msg(packets); }
					} else {
						let assembler = PacketAssembler::create(msg_id, packets);
						self.packet_assemblers.insert(msg_id, assembler);
					}
				},
                PeToCaPacket::Tcp((port_no, (msg_type, serialized))) => {
                    let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "listen_pe_loop", comment: "PortNumber" })?;
                    let tree_map = match self.border_port_tree_id_map.get(&port_number) {
                        Some(tree_id) => match self.tree_name_map.get(&tree_id) {
                            Some(map) => map,
                            None => return Err(CellagentError::TreeMap { func_name: "listen_pe_loop 1", cell_id: self.cell_id.clone(), tree_name: S(tree_id.get_name()) }.into())
                        },
                        None => return Err(CellagentError::Border { func_name: "listen_pe_loop 2", cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    };
                    match msg_type {
                        MsgType::StackTree => {
                            let msg = serde_json::from_str::<HashMap<String, String>>(&serialized).context(CellagentError::Chain { func_name: "listen_pe_loop", comment: "deserialize" })?;
                            println!("Cellagent {}: msg {:?}", self.cell_id, msg);
                            let new_tree_name = self.get_param(&msg,"new_tree_name")?;
                            let base_tree_name = self.get_param(&msg, "base_tree_name")?;
                            let base_tree_id = match tree_map.get(&AllowedTree::new(&base_tree_name)) {
                                Some(id) => id,
                                None => return Err(CellagentError::TreeMap { func_name: "listen_pe_loop 3", cell_id: self.cell_id.clone(), tree_name: S(base_tree_name) }.into())
                            };
                            let ref new_tree_id = self.my_tree_id.add_component(&new_tree_name).context(CellagentError::Chain { func_name: "listen_pe_loop", comment: "new_tree_id" })?;
                            let gvm_eqn_serialized = self.get_param(&msg, "gvm_eqn")?;
                            let ref gvm_eqn = serde_json::from_str::<GvmEquation>(&gvm_eqn_serialized).context(CellagentError::Chain { func_name: "listen_pe_loop", comment: "gvm" })?;
                            let stack_tree_msg = StackTreeMsg::new(new_tree_id, base_tree_id, gvm_eqn);
                        },
                        _ => return return Err(CellagentError::MsgType { func_name: "listen_pe_loop 4", cell_id: self.cell_id.clone(), msg_type: msg_type }.into())
                    }
                }
			}
		}
	}
    fn get_param(&self, msg: &HashMap<String, String>, param: &str) -> Result<String, Error> {
        //println!("Cellagent {}: get_param {}", self.cell_id, param);
        match msg.get(&S(param)).cloned() {
            Some(p) => Ok(p),
            None => Err(CellagentError::Message { func_name: "listen_pe_loop", cell_id: self.cell_id.clone(), msg: msg.clone() }.into())
        }
    }
	fn gvm_eval_save(&self, tree_uuid: Uuid, msg: Box<Message>) -> Result<bool, Error> {
		let f = "gvm_eval_save";
		if let Ok(gvm_eqn) = msg.get_gvm_eqn() {
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
			let new_tree_id = self.my_tree_id.add_component("Noc").context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			let port_number = PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			let entry = self.update_traph(&new_tree_id, port_number, traph::PortStatus::Parent,
				Some(&gvm_eqn), &mut HashSet::new(), TableIndex(0), PathLength(CellNo(1)), None).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
            let base_name = "Base";
            let base = AllowedTree::new(base_name);
            let mut tree_map = HashMap::new();
            tree_map.insert(base, self.my_tree_id.clone());
			self.tree_name_map.insert(new_tree_id.clone(),tree_map);
            self.border_port_tree_id_map.insert(port_number, new_tree_id.clone());
            println!("Cell {}: tree_name_map 1 {:?}", self.cell_id, self.tree_name_map);
			let port_no_mask = Mask::new(port_number);
			let tree_name_msg = TreeNameMsg::new(&base_name);
            let serialized = serde_json::to_string(&tree_name_msg).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
            self.ca_to_pe.send(CaToPePacket::Tcp((port_number, (MsgType::TreeName, serialized)))).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			//println!("Cell {}: Sending on ports {}: {}", self.cell_id, port_no_mask, tree_name_msg);
            Ok(())
		} else {
			//println!("Cell {}: port {} connected", self.cell_id, *port_no);
			let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?);
			let path = Path::new(port_no, self.no_ports).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
			let hops = PathLength(CellNo(1));
			let my_table_index = self.my_entry.get_index();
			let msg = DiscoverMsg::new(&self.my_tree_id, my_table_index, &self.cell_id, hops, path);
			//println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, msg);
			let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
			self.ca_to_pe.send(entry).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			self.send_msg(&self.connected_tree_id, &msg, port_no_mask).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
			let saved_msgs  = self.get_saved_msgs();
			self.forward_discover(&saved_msgs, port_no_mask).context(CellagentError::Chain { func_name: "port_connected", comment: "" })?;
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
	fn forward_discover(&mut self, saved_msgs: &Vec<SavedMsg>, mask: Mask) -> Result<(), Error> {
		//println!("Cell {}: forwarding {} discover msgs", self.cell_id, saved_msgs.len());
		for packets in saved_msgs.iter() {
			self.send_packets(self.connected_tree_id.get_uuid(), packets, mask)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	fn forward_saved(&mut self, saved_msgs: &Vec<SavedMsg>, mask: Mask) -> Result<(), Error> {
		//println!("Cell {}: forwarding {} messages", self.cell_id, saved_msgs.len());
		for packets in saved_msgs.iter() {
			//let msg = MsgType::get_msg(&packets)?;
			//println!("Cell {}: forward {}", self.cell_id, msg);
			let uuid = packets[0].get_tree_uuid();
			self.send_packets(uuid, packets, mask)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
    pub fn send_msg<T>(&self, tree_id: &TreeID, msg: &T, user_mask: Mask) -> Result<Vec<Packet>, Error>
        where T: Message + ::std::marker::Sized + serde::Serialize
    {
        let packets = msg.to_packets(tree_id).context(CellagentError::Chain { func_name: "send_msg", comment: "" })?;
        self.send_packets(tree_id.get_uuid(), &packets, user_mask).context(CellagentError::Chain { func_name: "send_msg", comment: "" })?;
        Ok(packets)
    }
	fn send_packets(&self, tree_uuid: Uuid, packets: &Vec<Packet>, user_mask: Mask) -> Result<(), Error> {
		let f = "send_msg";
        let index = match self.traphs.lock().unwrap().get(&tree_uuid) {
            Some(traph) => traph.get_table_index(&tree_uuid)?,
            None => return Err(CellagentError::Tree { cell_id: self.cell_id.clone(), func_name: f, tree_uuid: tree_uuid }.into())
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
    Chain { func_name: &'static str, comment: &'static str },
    #[fail(display = "CellagentError::Border {}: Port {} is not a border port on cell {}", func_name, port_no, cell_id)]
    Border { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "CellagentError::ManifestVms {}: No VMs in manifest for cell {}", func_name, cell_id)]
    ManifestVms { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellagentError::Message {}: Malformed request {:?} from border port on cell {}", func_name, msg, cell_id)]
    Message { func_name: &'static str, cell_id: CellID, msg: HashMap<String, String> },
    #[fail(display = "CellAgentError::MsgType {}: Message type {} is not accepted from a border port on cell {}", func_name, cell_id, msg_type)]
    MsgType { func_name: &'static str, cell_id: CellID, msg_type: MsgType },
    #[fail(display = "CellAgentError::NoTraph {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    NoTraph { cell_id: CellID, func_name: &'static str, tree_uuid: Uuid },
    #[fail(display = "CellAgentError::Size {}: No more room in routing table for cell {}", func_name, cell_id)]
    Size { cell_id: CellID, func_name: &'static str },
    #[fail(display = "CellAgentError::StackTree {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)]
    StackTree { func_name: &'static str, tree_id: TreeID, cell_id: CellID },
    #[fail(display = "CellAgentError::TenantMask {}: Cell {} has no tenant mask", func_name, cell_id)]
    TenantMask { func_name: &'static str, cell_id: CellID },
    #[fail(display = "CellAgentError::TreeMap {}: Cell {} has no tree map entry for {}", func_name, cell_id, tree_name)]
    TreeMap { func_name: &'static str, cell_id: CellID, tree_name: String },
    #[fail(display = "CellAgentError::Tree {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid },
    #[fail(display = "CellAgentError::TreeIndex {}: No tree associated with index {:?} on cell {}", func_name, index, cell_id)]
    TreeIndex { func_name: &'static str, index: TableIndex, cell_id: CellID }
}
