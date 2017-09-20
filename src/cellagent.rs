use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use uuid::Uuid;

use config::{MAX_ENTRIES, CellNo, CellType, PathLength, PortNo, TableIndex};
use container::Service;
use gvm_equation::{GvmEquation, GvmEqn};
use message::{DiscoverMsg, StackTreeMsg, Message, MsgType};
use message_types::{CaToPe, CaFromPe, CaToVm, VmFromCa, VmToCa, CaFromVm, CaToPePacket, PeToCaPacket};
use name::{Name, CellID, TreeID, UpTraphID, VmID};
use packet::{Packet, PacketAssembler, PacketAssemblers};
use port;
use routing_table_entry::{RoutingTableEntry};
use traph;
use traph::{Traph};
use tree::Tree;
use utility::{BASE_TENANT_MASK, Mask, Path, PortNumber, S};
use vm::VirtualMachine;

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";

pub type Traphs = HashMap<Uuid, Traph>;
pub type Trees = HashMap<TableIndex, TreeID>;
pub type TreeMap = HashMap<Uuid, Uuid>;
pub type TreeIDMap = HashMap<Uuid, TreeID>;
pub type SavedMsg = Vec<Packet>;

#[derive(Debug, Clone)]
pub struct CellAgent {
	cell_id: CellID,
	cell_type: CellType,
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
	tree_id_map: Arc<Mutex<TreeIDMap>>, // For debugging
	tenant_masks: Vec<Mask>,
	ca_to_pe: CaToPe,
	vm_id_no: usize,
	up_traphs_senders: HashMap<UpTraphID,Vec<CaToVm>>,
	up_traphs_clist: HashMap<TreeID, TreeID>,
	packet_assemblers: PacketAssemblers,
}
impl CellAgent {
	pub fn new(cell_id: &CellID, cell_type: CellType, no_ports: PortNo, ca_to_pe: CaToPe ) 
				-> Result<CellAgent> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let my_tree_id = TreeID::new(cell_id.get_name())?;
		my_tree_id.append2file()?;		
		let control_tree_id = TreeID::new(&(cell_id.get_name().to_string() + ":" + CONTROL_TREE_NAME))?;
		let connected_tree_id = TreeID::new(&(cell_id.get_name().to_string() + ":" + CONNECTED_PORTS_TREE_NAME))?;
		connected_tree_id.append2file()?;
		let mut free_indices = Vec::new();
		let trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..(*MAX_ENTRIES) { 
			free_indices.push(TableIndex(i)); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		Ok(CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id, cell_type: cell_type,
			control_tree_id: control_tree_id, connected_tree_id: connected_tree_id,	
			no_ports: no_ports, traphs: traphs, vm_id_no: 0, tree_id_map: Arc::new(Mutex::new(HashMap::new())),
			free_indices: Arc::new(Mutex::new(free_indices)), tree_map: Arc::new(Mutex::new(HashMap::new())),
			saved_msgs: Arc::new(Mutex::new(Vec::new())), saved_discover: Arc::new(Mutex::new(Vec::new())),
			my_entry: RoutingTableEntry::default(TableIndex(0))?, 
			connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(TableIndex(0))?)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), up_traphs_senders: HashMap::new(),
			up_traphs_clist: HashMap::new(), ca_to_pe: ca_to_pe, packet_assemblers: PacketAssemblers::new()})
		}
	pub fn initialize(&mut self, cell_type: CellType, ca_from_pe: CaFromPe) -> Result<()> {
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
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path)?;
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("false"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_equation = GvmEquation::new(eqns, Vec::new());
		let connected_tree_entry = self.update_traph(&connected_tree_id, port_number_0, 
			traph::PortStatus::Parent, Some(gvm_equation),
			&mut HashSet::new(), other_index, hops, path)?;
		self.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
		// Create my tree
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("false"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_equation = GvmEquation::new(eqns, Vec::new());
		self.my_entry = self.update_traph(&my_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path)?; 
		self.listen(ca_from_pe)?;
		Ok(())
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_traphs(&self) -> &Arc<Mutex<Traphs>> { &self.traphs }
	pub fn get_tree_id(&self, TableIndex(index): TableIndex) -> Result<TreeID> {
		let f = "get_tree_id";
		let trees = self.trees.lock().unwrap();
		match trees.get(&TableIndex(index)) {
			Some(t) => Ok(t.clone()),
			None => Err(ErrorKind::TreeIndex(self.cell_id.clone(), S(f), TableIndex(index)).into())
		}
	}
	pub fn get_hops(&self, tree_id: &TreeID) -> Result<PathLength> {
		let f = "get_hops";
		if let Some(traph) = self.traphs.lock().unwrap().get(&tree_id.get_uuid()) {
			Ok(traph.get_hops()?)
		} else {
			Err(ErrorKind::Tree(self.cell_id.clone(), S(f), tree_id.get_uuid()).into())
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
	pub fn get_tenant_mask(&self) -> Result<&Mask> {
		let f = "get_tenant_mask";
		if let Some(tenant_mask) = self.tenant_masks.last() {
			Ok(tenant_mask)
		} else {
			return Err(ErrorKind::TenantMask(self.get_id(), S(f)).into())
		}
	}
	//pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn get_connected_ports_tree_id(&self) -> &TreeID { &self.connected_tree_id }
	pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(&tree_id.get_uuid())
	}
	fn use_index(&mut self) -> Result<TableIndex> {
		let f = "use_index";
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(ErrorKind::Size(self.cell_id.clone(), S(f)).into())
		}
	}
//	fn free_index(&mut self, index: TableIndex) {
//		self.free_indices.lock().unwrap().push(index);
//	}
	pub fn create_vms(&mut self, service_sets: Vec<Vec<Service>>) -> Result<()> {
		let name = format!("Up:{}+{}", self.cell_id, self.up_traphs_clist.len());
		let up_traph_id = UpTraphID::new(&name)?;
		let mut ca_to_vms = Vec::new();
		let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();
		let ca_id = TreeID::new("CellAgent")?;
		let base_id = TreeID::new("BaseTree")?;
		self.up_traphs_clist.insert(ca_id.clone(), self.control_tree_id.clone());
		self.up_traphs_clist.insert(base_id.clone(), self.my_tree_id.clone());
		let mut tree_ids = HashMap::new();
		tree_ids.insert("CellAgent",ca_id);
		tree_ids.insert("BaseTree", base_id);
		for mut services in service_sets {
			self.vm_id_no = self.vm_id_no + 1;
			let vm_id = VmID::new(&self.cell_id, self.vm_id_no)?;
			let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
			let mut vm = VirtualMachine::new(vm_id.clone());
			vm.initialize(&mut services, &up_traph_id, &tree_ids,
				&vm_to_ca, vm_from_ca)?;
			ca_to_vms.push(ca_to_vm);
		}
		self.up_traphs_senders.insert(up_traph_id.clone(), ca_to_vms);
		self.listen_uptraph(up_traph_id.clone(), ca_from_vm)?;
		Ok(())
	}
	fn listen_uptraph(&self, up_traph_id: UpTraphID, ca_from_vm: CaFromVm) -> Result<()> {
		let ca = self.clone();
		::std::thread::spawn( move || -> Result<()> {
		loop {
			let msg = ca_from_vm.recv()?;
			println!("CellAgent {}: got vm msg {}", ca.cell_id, msg);
		}	
		});
		Ok(())
	}
	pub fn update_traph(&mut self, black_tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				gvm_equation: Option<GvmEquation>, children: &mut HashSet<PortNumber>, 
				other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry> {
		let entry = {
			let mut traphs = self.traphs.lock().unwrap();
			let mut traph = match traphs.entry(black_tree_id.get_uuid()) { // Avoids lifetime problem
				Entry::Occupied(t) => t.into_mut(),
				Entry::Vacant(v) => {
					//println!("Cell {}: update traph {} {}", self.cell_id, black_tree_id, black_tree_id.get_uuid());
					self.tree_map.lock().unwrap().insert(black_tree_id.get_uuid(), black_tree_id.get_uuid());
					self.tree_id_map.lock().unwrap().insert(black_tree_id.get_uuid(), black_tree_id.clone());
					let index = self.clone().use_index()?;
					let t = Traph::new(&self.cell_id, &black_tree_id, index)?;
					v.insert(t)
				}
			};
			let (gvm_recv, gvm_send, gvm_xtnd, gvm_save) = match gvm_equation {
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
	pub fn get_traph(&self, black_tree_id: &TreeID) -> Result<Traph> {
		let mut locked = self.traphs.lock().unwrap();
		let uuid = black_tree_id.get_uuid();
		match locked.entry(uuid) {
			Entry::Occupied(o) => Ok(o.into_mut().clone()),
			Entry::Vacant(_) => Err(ErrorKind::NoTraph(self.cell_id.clone(), "stack_tree".to_string(), uuid).into())
		}		
	}
	pub fn stack_tree(&mut self, tree_id: &TreeID, parent_tree_uuid: &Uuid, 
			black_tree_id: &TreeID, gvm_eqn: &GvmEquation) -> Result<()> {
		let mut traph = self.get_traph(black_tree_id)?;
		if traph.has_tree(tree_id) { return Ok(()); } // Check for redundant StackTreeMsg
		let parent_entry = traph.get_tree_entry(&parent_tree_uuid)?;
		let mut entry = parent_entry.clone();
		let index = self.use_index()?; 
		entry.set_table_index(index);
		entry.set_uuid(&tree_id.get_uuid());
		let params = traph.get_params(gvm_eqn.get_variables())?;
		if !gvm_eqn.eval_recv(&params)? { 
			let mask = entry.get_mask().and(Mask::all_but_zero(self.no_ports));
			entry.set_mask(mask);
		}
		if !gvm_eqn.eval_xtnd(&params)? { entry.clear_children(); }
		if gvm_eqn.eval_send(&params)? { entry.enable_send(); } else { entry.disable_send(); }
		let tree = Tree::new(&tree_id, black_tree_id, Some(gvm_eqn.clone()), entry);
		//println!("Cell {}: stack tree {} {}", self.cell_id, tree_id, tree_id.get_uuid());
		traph.stack_tree(&tree);
		self.tree_map.lock().unwrap().insert(tree_id.get_uuid(), black_tree_id.get_uuid());
		self.tree_id_map.lock().unwrap().insert(tree_id.get_uuid(), tree_id.clone());
		self.ca_to_pe.send(CaToPePacket::Entry(entry))?;
		Ok(())
	}
	fn listen(&mut self, ca_from_pe: CaFromPe) -> Result<()>{
		let mut ca = self.clone();
		::std::thread::spawn( move || { 
			let _ = ca.listen_loop(ca_from_pe).map_err(|e| ca.write_err(e));
		});
		Ok(())
	}
	fn listen_loop(&mut self, ca_from_pe: CaFromPe) -> Result<()> {
		loop {
			//println!("CellAgent {}: waiting for status or packet", ca.cell_id);
			match ca_from_pe.recv()? {
				PeToCaPacket::Status(port_no, is_border, status) => match status {
					port::PortStatus::Connected => self.port_connected(port_no, is_border)?,
					port::PortStatus::Disconnected => self.port_disconnected(port_no)?
				},
				PeToCaPacket::Packet(port_no, packet) => {
					let msg_id = packet.get_header().get_msg_id();
					let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
					let tree_uuid = packet.get_tree_uuid();
					let (last_packet, packets) = packet_assembler.add(packet);
					if last_packet {
						let mut msg = match MsgType::get_msg(&packets) {
							Ok(m) => m,
							Err(err) => return Err(map_message_errors(err))
						};
						match msg.process(self, tree_uuid, port_no) {
							Ok(_) => (),
							Err(err) => return Err(map_message_errors(err))
						};
						let save = self.gvm_eval_save(tree_uuid, msg)?;
						if save && (tree_uuid != self.connected_tree_id.get_uuid()) { self.add_saved_msg(packets); }
					} else {
						let assembler = PacketAssembler::create(msg_id, packets);
						self.packet_assemblers.insert(msg_id, assembler);
					}
				}
			}
		}
	}
	fn gvm_eval_save(&self, tree_uuid: Uuid, msg: Box<Message>) -> Result<bool> {
		let f = "gvm_eval_save";
		if let Some(gvm_eqn) = msg.get_payload().get_gvm_eqn() {
			//let locked = self.tree_id_map.lock().unwrap();
			//println!("Cell {}: tree_uuid {}", self.cell_id, tree_uuid);
			if let Some(black_tree_uuid) = self.tree_map.lock().unwrap().get(&tree_uuid) {
				//println!("Cell {}: black_tree_uuid {}", self.cell_id, black_tree_uuid);
				let mut locked = self.traphs.lock().unwrap();
				let traph = match locked.entry(*black_tree_uuid) {
					Entry::Occupied(o) => o.into_mut(),
					Entry::Vacant(_) => return Err(ErrorKind::Tree(self.cell_id.clone(), S(f), *black_tree_uuid).into())
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
	fn port_connected(&mut self, port_no: PortNo, is_border: bool) -> Result<()> {
		//println!("CellAgent {}: port {} is border {} connected", self.cell_id, port_no, is_border);
		if is_border {
			//println!("CellAgent {}: port {} is a border port", self.cell_id, *port_no);
			let tree_id = self.my_tree_id.add_component("Outside")?;
			let tree_id = TreeID::new(tree_id.get_name())?;
			tree_id.append2file()?;
			let ref my_tree_id = self.my_tree_id.clone(); // Need because self borrowed mut
			let msg = match StackTreeMsg::new(&tree_id, &self.my_tree_id) {
				Ok(m) => m,
				Err(err) => return Err(map_message_errors(err))
			};
			let packets = match msg.to_packets(&self.my_tree_id) {
				Ok(p) => p,
				Err(err) => return Err(map_message_errors(err))
			};
			let port_no_mask = Mask::all_but_zero(self.no_ports);
			let my_index = self.my_entry.get_index();
			for packet in packets {
				self.ca_to_pe.send(CaToPePacket::Packet((my_index, port_no_mask, packet)))?;			
			}
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("false"));
			eqns.insert(GvmEqn::Xtnd("true"));
			eqns.insert(GvmEqn::Save("true"));
			let gvm_eqn = GvmEquation::new(eqns, Vec::new());
			self.stack_tree(&tree_id, &my_tree_id.get_uuid(), my_tree_id, &gvm_eqn)?;
		} else {
			//println!("Cell {}: port {} connected", self.cell_id, *port_no);
			let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
			let path = Path::new(port_no, self.no_ports)?;
			self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
			let hops = PathLength(CellNo(1));
			let my_table_index = self.my_entry.get_index();
			let msg = DiscoverMsg::new(&self.my_tree_id, my_table_index, &self.cell_id, hops, path);
			let packets = match msg.to_packets(&self.control_tree_id) {
				Ok(p) => p,
				Err(err) => return Err(map_message_errors(err))
			};
			//println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, msg);
			let connected_tree_index = (*self.connected_tree_entry.lock().unwrap()).get_index();
			for packet in packets {
				let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
				self.ca_to_pe.send(entry)?;
				let packet_msg = CaToPePacket::Packet((connected_tree_index, port_no_mask, packet));
				self.ca_to_pe.send(packet_msg)?;
			}
			let saved_msgs  = self.get_saved_msgs();
			self.forward_discover(&saved_msgs, port_no_mask)?;		
		}
		Ok(())		
	}
	fn port_disconnected(&self, port_no: PortNo) -> Result<()> {
		//println!("Cell Agent {} got disconnected on port {}", self.cell_id, port_no);
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
		self.connected_tree_entry.lock().unwrap().and_with_mask(port_no_mask.not());
		let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
		self.ca_to_pe.send(entry)?;
		Ok(())	
	}		
	pub fn forward_discover(&mut self, saved_msgs: &Vec<SavedMsg>, mask: Mask) -> Result<()> {
		//println!("Cell {}: forwarding {} discover msgs", self.cell_id, saved_msgs.len());
		for packets in saved_msgs.iter() {
			self.send_msg(self.connected_tree_id.get_uuid(), packets, mask)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	pub fn forward_saved(&mut self, saved_msgs: &Vec<SavedMsg>, mask: Mask) -> Result<()> {
		//println!("Cell {}: forwarding {} messages", self.cell_id, saved_msgs.len());
		for packets in saved_msgs.iter() {
			//let msg = MsgType::get_msg(&packets)?;
			//println!("Cell {}: forward {}", self.cell_id, msg);
			let uuid = packets[0].get_tree_uuid();
			self.send_msg(uuid, packets, mask)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	pub fn send_msg(&self, tree_uuid: Uuid, packets: &Vec<Packet>, user_mask: Mask) 
			-> Result<()> {
		let f = "send_msg";
		let index = {
			if let Some(traph) = self.traphs.lock().unwrap().get(&tree_uuid) {
				traph.get_table_index(&tree_uuid)?			
			} else {
				return Err(ErrorKind::Tree(self.cell_id.clone(), S(f), tree_uuid).into());
			}
		};
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packet);
			let msg = CaToPePacket::Packet((index, user_mask, *packet));
			self.ca_to_pe.send(msg)?;
			//println!("CellAgent {}: sent packet {} on tree {} to packet engine with index {}", self.cell_id, packet_count, tree_id, index);
		}
		Ok(())
	}
	fn write_err(&self, e: Error) -> Result<()> {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "CellAgent {}: {}", self.cell_id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
		Err(e)
	}
}
impl fmt::Display for CellAgent { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!(" Cell Agent");
		for (_, traph) in self.traphs.lock().unwrap().iter() {
			s = s + &format!("\n{}", traph);
		}
		write!(f, "{}", s) }
}
// Errors
fn map_message_errors(err: ::message::Error) -> ::cellagent::Error {
	::cellagent::ErrorKind::Message(Box::new(err)).into()
}
error_chain! {
	foreign_links {
		CaToPePacket(::message_types::CaPeError);
		Io(::std::io::Error);
		Recv(::std::sync::mpsc::RecvError);
	}
	links {
		GvmEqn(::gvm_equation::Error, ::gvm_equation::ErrorKind);
//		Message(Box<::message::Error>, Box<::message::ErrorKind>);
		Name(::name::Error, ::name::ErrorKind);
		RoutingTableEntry(::routing_table_entry::Error, ::routing_table_entry::ErrorKind);
		Traph(::traph::Error, ::traph::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
		Vm(::vm::Error, ::vm::ErrorKind);
	}
	errors { 
		Message(err: Box<::message::Error>)
		NoTraph(cell_id: CellID, func_name: String, tree_uuid: Uuid ) {
			display("Cellagent {}: A Traph with TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)
		}
		Size(cell_id: CellID, func_name: String) {
			display("Cellagent {}: No more room in routing table for cell {}", func_name, cell_id)
		}
//		StackTree(cell_id: CellID, func_name: String, tree_id: TreeID) {
//			display("Cellagent {}: Problem stacking tree {} on cell {}", func_name, tree_id, cell_id)
//		}
		TenantMask(cell_id: CellID, func_name: String) {
			display("Cellagent {}: Cell {} has no tenant mask", func_name, cell_id)
		}
		Tree(cell_id: CellID, func_name: String, tree_uuid: Uuid ) {
			display("Cellagent {}: TreeID {} does not exist on cell {}", func_name, tree_uuid, cell_id)
		}
		TreeIndex(cell_id: CellID, func_name: String, index: TableIndex) {
			display("Cellagent {}: No tree associated with index {} on cell {}", func_name, index.0, cell_id)
		} 
	}
}
