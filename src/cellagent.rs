use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use serde_json;
use uuid::Uuid;

use config::{MAX_ENTRIES, MsgID, CellNo, PathLength, PortNo, TableIndex};
use container::Service;
use gvm_equation::{GvmEquation, GvmVariable, GvmVariableType};
use message::{DiscoverMsg, DiscoverDMsg, StackTreeMsg, SetupVMsMsg, Message, MsgType};
use message_types::{CaToPe, CaFromPe, CaToVm, VmFromCa, VmToCa, CaFromVm, CaToPePacket, PeToCaPacket};
use name::{Name, CellID, TreeID, UpTraphID, VmID};
use nalcell::CellType;
use packet::{Packet, Packetizer, PacketAssembler, PacketAssemblers, Serializer};
use port;
use routing_table_entry::{RoutingTableEntry};
use traph;
use traph::{Traph};
use utility::{BASE_TENANT_MASK, Mask, Path, PortNumber};
use vm::VirtualMachine;

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";

pub type Traphs = Arc<Mutex<HashMap<Uuid,Traph>>>;
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
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<HashMap<TableIndex,String>>>,
	traphs: Traphs,
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
		let my_tree_id = TreeID::new(cell_id.get_name()).chain_err(|| ErrorKind::CellagentError)?;
		let control_tree_id = TreeID::new(CONTROL_TREE_NAME).chain_err(|| ErrorKind::CellagentError)?;
		let connected_tree_id = TreeID::new(CONNECTED_PORTS_TREE_NAME).chain_err(|| ErrorKind::CellagentError)?;
		let mut free_indices = Vec::new();
		let trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..MAX_ENTRIES.0 { 
			free_indices.push(TableIndex(i)); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		Ok(CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id, cell_type: cell_type,
			control_tree_id: control_tree_id, connected_tree_id: connected_tree_id,	
			no_ports: no_ports, traphs: traphs, vm_id_no: 0,
			free_indices: Arc::new(Mutex::new(free_indices)),
			saved_msgs: Arc::new(Mutex::new(Vec::new())), my_entry: RoutingTableEntry::default(TableIndex(0)).chain_err(|| ErrorKind::CellagentError)?, 
			connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(TableIndex(0)).chain_err(|| ErrorKind::CellagentError)?)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), up_traphs_senders: HashMap::new(),
			up_traphs_clist: HashMap::new(), ca_to_pe: ca_to_pe, packet_assemblers: PacketAssemblers::new()})
		}
	pub fn initialize(&mut self, cell_type: CellType, ca_from_pe: CaFromPe) -> Result<()> {
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = PortNumber::new(PortNo{v:0}, self.no_ports).chain_err(|| ErrorKind::CellagentError)?;
		let other_index = TableIndex(0);
		let hops = PathLength(CellNo(0));
		let path = None;
		let control_tree_id = self.control_tree_id.clone();
		let connected_tree_id = self.connected_tree_id.clone();
		let my_tree_id = self.my_tree_id.clone();
		let gvm_equation = GvmEquation::new("true", "true", "true", Vec::new());
		self.update_black_trees(&control_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?;
		let gvm_equation = GvmEquation::new("false", "true", "true", Vec::new());
		let connected_tree_entry = self.update_black_trees(&connected_tree_id, port_number_0, 
			traph::PortStatus::Parent, Some(gvm_equation),
			&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?;
		self.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
		// Create my tree
		let gvm_equation = GvmEquation::new("true", "true", "true", Vec::new());
		self.my_entry = self.update_black_trees(&my_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?; 
		self.listen(ca_from_pe).chain_err(|| ErrorKind::CellagentError)?;
		Ok(())
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_traphs(&self) -> &Traphs { &self.traphs }
	pub fn get_tree_id(&self, index: TableIndex) -> Result<String> {
		let trees = self.trees.lock().unwrap();
		let tree_id = match trees.get(&index) {
			Some(t) => t.clone(),
			None => {
				println!("--- CellAgent {}: index {} in trees table {:?}", self.cell_id, index.0, *trees);
				return Err(ErrorKind::TreeIndex(self.cell_id.clone(), index).into())}
			
		};
		Ok(tree_id)
	}
	pub fn get_traph(&self, tree_id: &TreeID) -> Result<Traph> {
		let mut traphs = self.traphs.lock().unwrap();
		if let Some(traph) = traphs.remove(&tree_id.get_uuid()) {
			traphs.insert(tree_id.get_uuid(), traph.clone());
			Ok(traph)
		} else {
			Err(ErrorKind::Tree(self.cell_id.clone(), tree_id.get_uuid()).into())
		}
	}
	pub fn get_hops(&self, tree_id: &TreeID) -> Result<PathLength> {
		let traph = self.get_traph(tree_id)?;
		let hops = traph.get_hops().chain_err(|| ErrorKind::CellagentError)?;
		Ok(hops)
	}
	pub fn get_saved_msgs(&self) -> Vec<SavedMsg> {
		self.saved_msgs.lock().unwrap().to_vec()
	}
	pub fn add_saved_msg(&mut self, msg: SavedMsg) -> Vec<SavedMsg> {
		{ 
			let mut saved_msgs = self.saved_msgs.lock().unwrap();
			//println!("CellAgent {}: added msg {} as entry {} for tree {}", self.cell_id, msg.get_header().get_count(), discover_msgs.len()+1, msg); 
			saved_msgs.push(msg);
		}
		self.get_saved_msgs()
	}
	//pub fn get_tenant_mask(&self) -> Result<&Mask, CellAgentError> {
	//	if let Some(tenant_mask) = self.tenant_masks.last() {
	//		Ok(tenant_mask)
	//	} else {
	//		return Err(CellAgentError::TenantMask(TenantMaskError::new(self.get_id())))
	//	}
	//}
	//pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn get_connected_ports_tree_id(&self) -> &TreeID { &self.connected_tree_id }
	pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(&tree_id.get_uuid())
	}
	fn use_index(&mut self) -> Result<TableIndex> {
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(ErrorKind::Size(self.cell_id.clone()).into())
		}
	}
	pub fn create_vms(&mut self, service_sets: Vec<Vec<Service>>) -> Result<()> {
		let up_traph_id = UpTraphID::new(&format!("Up:{}+{}", self.cell_id, self.up_traphs_clist.len())).chain_err(|| ErrorKind::CellagentError)?;
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
			let vm_id = VmID::new(&self.cell_id, self.vm_id_no).chain_err(|| ErrorKind::CellagentError)?;
			let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
			let mut vm = VirtualMachine::new(vm_id);
			vm.initialize(&mut services, &up_traph_id, &tree_ids, 
				&vm_to_ca, vm_from_ca).chain_err(|| ErrorKind::CellagentError)?;
			ca_to_vms.push(ca_to_vm);
		}
		self.up_traphs_senders.insert(up_traph_id.clone(), ca_to_vms);
		self.listen_uptraph(up_traph_id, ca_from_vm)?;
		Ok(())
	}
	fn listen_uptraph(&self, up_traph_id: UpTraphID, ca_from_vm: CaFromVm) -> Result<()> {
		let ca = self.clone();
		::std::thread::spawn( move || -> Result<()> {
		loop {
			let msg = ca_from_vm.recv().chain_err(|| ErrorKind::CellagentError)?;
			println!("CellAgent {}: got vm msg {}", ca.cell_id, msg);
		}	
		});
		Ok(())
	}
	pub fn update_black_trees(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				gvm_equation: Option<GvmEquation>, children: &mut HashSet<PortNumber>, 
				other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry> {
// Note that traphs is updated transactionally; I remove an entry, update it, then put it back.
		let mut traphs = self.traphs.lock().unwrap();
		let mut traph = match traphs.remove(&tree_id.get_uuid()) { // Avoids lifetime problem
			Some(t) => t,
			None => Traph::new(&tree_id, self.clone().use_index().chain_err(|| ErrorKind::CellagentError)?).chain_err(|| ErrorKind::CellagentError)?
		};
		let (hops, path) = match port_status{
			traph::PortStatus::Child => {
				let element = traph.get_parent_element().chain_err(|| ErrorKind::CellagentError)?;
				// Need to coordinate the following with DiscoverMsg.update_discover_msg
				(PathLength(CellNo((element.get_hops().0).0+1)), element.get_path()) 
			},
			_ => (hops, path)
		};
		let traph_status = traph.get_port_status(port_number);
		let port_status = match traph_status {
			traph::PortStatus::Pruned => port_status,
			_ => traph_status  // Don't replace if Parent or Child
		};
		let (gvm_recv, gvm_send) = match gvm_equation {
			Some(eqn) => {
				let variables = self.get_params(tree_id, eqn.get_variables())?;
				let recv = eqn.eval_recv(&variables).chain_err(|| ErrorKind::CellagentError)?;
				let send = eqn.eval_send(&variables).chain_err(|| ErrorKind::CellagentError)?;
				(recv, send)
			},
			None => (false, false),
		};
		if gvm_recv { children.insert(PortNumber::new(PortNo{v:0}, self.no_ports)?); }
		let entry = traph.new_element(tree_id, port_number, port_status, other_index, children, hops, path).chain_err(|| ErrorKind::CellagentError)?; 
// Here's the end of the transaction
		//println!("CellAgent {}: entry {}\n{}", self.cell_id, entry, traph); 
		if gvm_send {
			traphs.insert(tree_id.get_uuid(), traph);
			{
				self.trees.lock().unwrap().insert(entry.get_index(), tree_id.stringify());
			}
			self.ca_to_pe.send(CaToPePacket::Entry(entry)).chain_err(|| ErrorKind::CellagentError)?;
		}
		Ok(entry)
	}
	pub fn get_params(&self, tree_id: &TreeID, vars: &Vec<GvmVariable>) -> Result<Vec<GvmVariable>> {
		let mut variables = Vec::new();
		for var in vars {
			match var.get_value().as_ref() {
				"hops" => {
					let hops = (self.get_hops(tree_id)?.0).0;
					variables.push(GvmVariable::new(GvmVariableType::CellNo, hops));
				},
				_ => ()
			}
		}
		Ok(variables)
	}
	fn listen(&mut self, ca_from_pe: CaFromPe) -> Result<()>{
		let mut ca = self.clone();
		::std::thread::spawn( move || { 
			let _ = ca.listen_loop(ca_from_pe).chain_err(|| ErrorKind::CellagentError).map_err(|e| ca.write_err(e));
		});
		Ok(())
	}
	fn listen_loop(&mut self, ca_from_pe: CaFromPe) -> Result<()> {
		loop {
			//println!("CellAgent {}: waiting for status or packet", ca.cell_id);
			match ca_from_pe.recv().chain_err(|| ErrorKind::CellagentError)? {
				PeToCaPacket::Status(port_no, is_border, status) => match status {
					port::PortStatus::Connected => self.port_connected(port_no, is_border).chain_err(|| ErrorKind::CellagentError)?,
					port::PortStatus::Disconnected => self.port_disconnected(port_no).chain_err(|| ErrorKind::CellagentError)?
				},
				PeToCaPacket::Packet(port_no, packet) => {
					let msg_id = packet.get_header().get_msg_id();
					let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
					if let Some(packets) = packet_assembler.clone().add(packet) {
						let (msg_type, serialized_msg) = MsgType::get_type_serialized(packets).chain_err(|| ErrorKind::CellagentError)?;
						if let Some(mut msg) = self.get_msg(msg_type, serialized_msg)? {
							//println!("CellAgent {}: got msg {}", self.cell_id, msg);
							msg.process(&mut self.clone(), port_no).chain_err(|| ErrorKind::CellagentError)?;
						};
					} else {
						self.packet_assemblers.insert(msg_id, packet_assembler);
					}
				}
			}
		}
	}
	fn get_msg(&self, msg_type: MsgType, serialized_msg: String) -> Result<Option<Box<Message>>> {
		Ok(match msg_type {
			MsgType::Discover => Some(Box::new(serde_json::from_str::<DiscoverMsg>(&serialized_msg).chain_err(|| ErrorKind::CellagentError)?)),
			MsgType::DiscoverD => Some(Box::new(serde_json::from_str::<DiscoverDMsg>(&serialized_msg).chain_err(|| ErrorKind::CellagentError)?)),
			MsgType::StackTree => Some(Box::new(serde_json::from_str::<StackTreeMsg>(&serialized_msg).chain_err(|| ErrorKind::CellagentError)?)),
			_ => match self.cell_type {
				CellType::NalCell => return Err(ErrorKind::InvalidMsgType(self.cell_id.clone(), msg_type).into()),
				CellType::Vm => panic!("Message for VM"),
				CellType::Container => panic!("Message for Container"),
				_ => panic!("Message for service")
			}
		})		
	}
	fn port_connected(&mut self, port_no: PortNo, is_border: bool) -> Result<()> {
		//println!("CellAgent {}: port {} is border {} connected", self.cell_id, port_no, is_border);
		if is_border {
			println!("CellAgent {}: port {} is a border port", self.cell_id, port_no.v);
			let tree_id = self.my_tree_id.add_component("Outside").chain_err(|| ErrorKind::CellagentError)?;
			let msg = StackTreeMsg::new(&tree_id, &self.my_tree_id).chain_err(|| ErrorKind::CellagentError)?;
			let direction = msg.get_header().get_direction();
			let bytes = Serializer::serialize(&msg).chain_err(|| ErrorKind::CellagentError).chain_err(|| ErrorKind::CellagentError)?;
			let port_no_mask = Mask::all_but_zero();
			let my_index = self.my_entry.get_index();
			let packets = Packetizer::packetize(&self.my_tree_id, bytes, direction).chain_err(|| ErrorKind::CellagentError)?;
			for packet in packets {
				self.ca_to_pe.send(CaToPePacket::Packet((my_index, port_no_mask, packet))).chain_err(|| ErrorKind::CellagentError)?;			
			}
		} else {
			let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).chain_err(|| ErrorKind::CellagentError)?);
			let path = Path::new(port_no, self.no_ports).chain_err(|| ErrorKind::CellagentError)?;
			self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
			let hops = PathLength(CellNo(1));
			let my_table_index = self.my_entry.get_index();
			let msg = DiscoverMsg::new(&self.my_tree_id, my_table_index, &self.cell_id, hops, path);
			let direction = msg.get_header().get_direction();
			let bytes = Serializer::serialize(&msg).chain_err(|| ErrorKind::CellagentError)?;
			let packets = Packetizer::packetize(&self.control_tree_id, bytes, direction,).chain_err(|| ErrorKind::CellagentError)?;
			//println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, msg);
			let connected_tree_index = (*self.connected_tree_entry.lock().unwrap()).get_index();
			for packet in packets {
				let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
				self.ca_to_pe.send(entry).chain_err(|| ErrorKind::CellagentError)?;
				let packet_msg = CaToPePacket::Packet((connected_tree_index, port_no_mask, packet));
				self.ca_to_pe.send(packet_msg).chain_err(|| ErrorKind::CellagentError)?;
			}
			let saved_msgs  = self.get_saved_msgs();
			//println!("CellAgent {}: {} discover msgs", ca.cell_id, discover_msgs.len());
			self.forward_saved(&saved_msgs, port_no_mask).chain_err(|| ErrorKind::CellagentError)?;		
		}
		Ok(())		
	}
	fn port_disconnected(&self, port_no: PortNo) -> Result<()> {
		//println!("Cell Agent {} got disconnected on port {}", self.cell_id, port_no);
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).chain_err(|| ErrorKind::CellagentError)?);
		self.connected_tree_entry.lock().unwrap().and_with_mask(port_no_mask.not());
		let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
		self.ca_to_pe.send(entry).chain_err(|| ErrorKind::CellagentError)?;
		//self.ca_to_pe.send((Some(*self.connected_tree_entry.lock().unwrap()),None)).chain_err(|| ErrorKind::CellagentError)?;	
		Ok(())	
	}		
	pub fn forward_saved(&mut self, saved_msgs: &Vec<SavedMsg>, mask: Mask) -> Result<()> {
		for packets in saved_msgs.iter() {
			let packet_uuid = packets[0].get_uuid();
			// If packet is to this cell's control tree, send out on all connected ports
			let uuid = if packet_uuid == self.control_tree_id.get_uuid() {
				self.connected_tree_id.get_uuid()
			} else {  // Otherwise, send it on the tree it came in on
				packet_uuid
			};
			self.send_msg(uuid, packets, mask).chain_err(|| ErrorKind::CellagentError)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	pub fn send_msg(&self, tree_uuid: Uuid, packets: &Vec<Packet>, user_mask: Mask) 
			-> Result<()> {
		let index = {
			if let Some(traph) = self.traphs.lock().unwrap().get(&tree_uuid) {
				traph.get_table_index()			
			} else {
				return Err(ErrorKind::Tree(self.cell_id.clone(), tree_uuid).into());
			}
		};
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packet);
			let msg = CaToPePacket::Packet((index, user_mask, *packet));
			self.ca_to_pe.send(msg).chain_err(|| ErrorKind::CellagentError)?;
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
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		CaToPe(::message_types::CaPeError);
		CaToVm(::message_types::CaVmError);
	}
	links {
		//Message(::message::Error, ::message::ErrorKind); // Recursive type error if left in
		Gvm(::gvm_equation::Error, ::gvm_equation::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		Packetizer(::packet::Error, ::packet::ErrorKind);
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		RoutingTableEntry(::routing_table_entry::Error, ::routing_table_entry::ErrorKind);
		Traph(::traph::Error, ::traph::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { CellagentError
		InvalidMsgType(cell_id: CellID, msg_type: MsgType) {
			display("Invalid message type {} from packet assembler on cell {}", msg_type, cell_id)
		}
		// Recursive type error if put in message.rs
		Message(cell_id: CellID, msg_no: usize) {
			display("Error processing message {} on cell {}", msg_no, cell_id)
		}		
		MessageAssembly(cell_id: CellID) {
			display("Problem assembling message on cell {}", cell_id)
		}
		PortTaken(cell_id: CellID, port_no: PortNo) {
			display("Receiver for port {} has been previously assigned on cell {}", port_no.v, cell_id)
		}
		Recvr(cell_id: CellID, port_no: PortNo) {
			display("No receiver for port {} on cell {}", port_no.v, cell_id)
		}
		Size(cell_id: CellID) {
			display("No more room in routing table for cell {}", cell_id)
		}
		TenantMask(cell_id: CellID) {
			display("Cell {} has no tenant mask", cell_id)
		}
		Tree(cell_id: CellID, tree_uuid: Uuid ) {
			display("TreeID {} does not exist on cell {}", tree_uuid, cell_id)
		}
		TreeIndex(cell_id: CellID, index: TableIndex) {
			display("No tree associated with index {} on cell {}", index.0, cell_id)
		} 
		UnknownVariable(cell_id: CellID, var: GvmVariable) {
			display("Cell {}: Unknown gvm variable {}", cell_id, var)
		}
		UpTraph(cell_id: CellID, up_tree_id: UpTraphID) {
			display("Cell {} has no UpTraph named {}", cell_id, up_tree_id)
		}
	}
}
