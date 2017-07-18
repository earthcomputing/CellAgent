use std::fmt;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use crossbeam::{Scope};
use serde_json;
use config::{MAX_ENTRIES, PathLength, PortNo, TableIndex};
use container::Service;
use gvm_equation::{GvmEquation, GvmVariables};
use message::{DiscoverMsg, DiscoverDMsg, StackTreeMsg, SetupVMsMsg, Message, MsgType};
use message_types::{CaToPe, CaFromPe, CaToVm, VmFromCa, VmToCa, CaFromVm, CaToPePacket, PeToCaPacket};
use name::{Name, CellID, TreeID, UpTreeID, VmID};
use packet::{Packet, Packetizer, PacketAssemblers};
use port;
use routing_table_entry::{RoutingTableEntry};
use traph;
use traph::{Traph};
use utility::{BASE_TENANT_MASK, Mask, Path, PortNumber};
use vm::VirtualMachine;

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";

pub type Traphs = Arc<Mutex<HashMap<String,Traph>>>;

#[derive(Debug, Clone)]
pub struct CellAgent {
	cell_id: CellID,
	no_ports: PortNo,
	my_tree_id: TreeID,
	control_tree_id: TreeID,
	connected_tree_id: TreeID,
	my_entry: RoutingTableEntry,
	connected_tree_entry: Arc<Mutex<RoutingTableEntry>>,
	discover_msgs: Arc<Mutex<Vec<DiscoverMsg>>>,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<HashMap<TableIndex,String>>>,
	traphs: Traphs,
	tenant_masks: Vec<Mask>,
	ca_to_pe: CaToPe,
	vm_id_no: usize,
	up_trees_senders: HashMap<UpTreeID,Vec<CaToVm>>,
	up_trees_clist: HashMap<TreeID, TreeID>,
	packet_assemblers: PacketAssemblers,
}
impl CellAgent {
	pub fn new(cell_id: &CellID, no_ports: PortNo, ca_to_pe: CaToPe ) 
				-> Result<CellAgent> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let my_tree_id = TreeID::new(cell_id.get_name()).chain_err(|| ErrorKind::CellagentError)?;
		let control_tree_id = TreeID::new(CONTROL_TREE_NAME).chain_err(|| ErrorKind::CellagentError)?;
		let connected_tree_id = TreeID::new(CONNECTED_PORTS_TREE_NAME).chain_err(|| ErrorKind::CellagentError)?;
		let mut free_indices = Vec::new();
		let trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..MAX_ENTRIES { 
			free_indices.push(i as TableIndex); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		Ok(CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id, 
			control_tree_id: control_tree_id, connected_tree_id: connected_tree_id,	
			no_ports: no_ports, traphs: traphs, vm_id_no: 0,
			free_indices: Arc::new(Mutex::new(free_indices)),
			discover_msgs: Arc::new(Mutex::new(Vec::new())), my_entry: RoutingTableEntry::default(0).chain_err(|| ErrorKind::CellagentError)?, 
			connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(0).chain_err(|| ErrorKind::CellagentError)?)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), up_trees_senders: HashMap::new(),
			up_trees_clist: HashMap::new(), ca_to_pe: ca_to_pe, packet_assemblers: PacketAssemblers::new()})
		}
	pub fn initialize(&mut self, scope: &Scope, ca_from_pe: CaFromPe) -> Result<()> {
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = PortNumber::new(0, self.no_ports).chain_err(|| ErrorKind::CellagentError)?;
		let other_index = 0;
		let hops = 0;
		let path = None;
		let control_tree_id = self.control_tree_id.clone();
		let connected_tree_id = self.connected_tree_id.clone();
		let my_tree_id = self.my_tree_id.clone();
		let gvm_equation = GvmEquation::new("true", GvmVariables::empty());
		self.update_traph(&control_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?;
		let gvm_equation = GvmEquation::new("false", GvmVariables::empty());
		let connected_tree_entry = self.update_traph(&connected_tree_id, port_number_0, 
			traph::PortStatus::Parent, Some(gvm_equation),
			&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?;
		self.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
		// Create my tree
		let gvm_equation = GvmEquation::new("true", GvmVariables::empty());
		self.my_entry = self.update_traph(&my_tree_id, port_number_0, 
				traph::PortStatus::Parent, Some(gvm_equation), 
				&mut HashSet::new(), other_index, hops, path).chain_err(|| ErrorKind::CellagentError)?; 
		self.listen(scope, ca_from_pe).chain_err(|| ErrorKind::CellagentError)?;
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
				println!("--- CellAgent {}: index {} in trees table {:?}", self.cell_id, index, *trees);
				return Err(ErrorKind::TreeIndex(self.cell_id.clone(), index).into())}
			
		};
		Ok(tree_id)
	}
	pub fn get_discover_msgs(&self) -> Vec<DiscoverMsg> {
		self.discover_msgs.lock().unwrap().to_vec()
	}
	pub fn add_discover_msg(&mut self, msg: DiscoverMsg) -> Vec<DiscoverMsg> {
		{ 
			let mut discover_msgs = self.discover_msgs.lock().unwrap();
			//println!("CellAgent {}: added msg {} as entry {} for tree {}", self.cell_id, msg.get_header().get_count(), discover_msgs.len()+1, msg); 
			discover_msgs.push(msg);
		}
		self.get_discover_msgs()
	}
	//pub fn get_tenant_mask(&self) -> Result<&Mask, CellAgentError> {
	//	if let Some(tenant_mask) = self.tenant_masks.last() {
	//		Ok(tenant_mask)
	//	} else {
	//		return Err(CellAgentError::TenantMask(TenantMaskError::new(self.get_id())))
	//	}
	//}
	//pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn get_connected_ports_tree_id(&self) -> TreeID { self.connected_tree_id.clone() }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(tree_id.get_name())
	}
	fn use_index(&mut self) -> Result<TableIndex> {
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(ErrorKind::Size(self.cell_id.clone()).into())
		}
	}
	pub fn create_vms(&mut self, service_sets: Vec<Vec<Service>>) -> Result<()> {
		let up_tree_id = UpTreeID::new(&format!("UpTree:{}+{}", self.cell_id, self.up_trees_clist.len())).chain_err(|| ErrorKind::CellagentError)?;
		let mut ca_to_vms = Vec::new();
		let (vm_to_ca, ca_from_vm): (VmToCa, CaFromVm) = channel();

		let ca_id = TreeID::new("CellAgent")?;
		let base_id = TreeID::new("BaseTree")?;
		self.up_trees_clist.insert(ca_id.clone(), self.control_tree_id.clone());
		self.up_trees_clist.insert(base_id.clone(), self.my_tree_id.clone());
		let tree_ids = vec![ca_id, base_id];
		for mut services in service_sets {
			self.vm_id_no = self.vm_id_no + 1;
			let vm_id = VmID::new(&self.cell_id, self.vm_id_no).chain_err(|| ErrorKind::CellagentError)?;
			let (ca_to_vm, vm_from_ca): (CaToVm, VmFromCa) = channel();
			let mut vm = VirtualMachine::new(vm_id);
			vm.initialize(&mut services, &up_tree_id, &tree_ids, 
				vm_to_ca.clone(), vm_from_ca).chain_err(|| ErrorKind::CellagentError)?;
			ca_to_vms.push(ca_to_vm);
		}
		self.up_trees_senders.insert(up_tree_id.clone(), ca_to_vms);
		self.listen_uptree(up_tree_id, ca_from_vm)?;
		Ok(())
	}
	fn listen_uptree(&self, up_tree_id: UpTreeID, ca_from_vm: CaFromVm) -> Result<()> {
		let ca = self.clone();
		::std::thread::spawn( move || -> Result<()> {
		loop {
			let msg = ca_from_vm.recv().chain_err(|| ErrorKind::CellagentError)?;
			println!("CellAgent {}: got vm msg {}", ca.cell_id, msg);
		}	
		});
		Ok(())
	}
	pub fn update_traph(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				gvm_equation: Option<GvmEquation>, children: &mut HashSet<PortNumber>, 
				other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry> {
// Note that traphs is updated transactionally; I remove an entry, update it, then put it back.
		let mut traphs = self.traphs.lock().unwrap();
		let mut traph = match traphs.remove(tree_id.get_name()) { // Avoids lifetime problem
			Some(t) => t,
			None => Traph::new(self.cell_id.clone(), tree_id.clone(), 
				self.clone().use_index().chain_err(|| ErrorKind::CellagentError)?).chain_err(|| ErrorKind::CellagentError)?
		};
		let (hops, path) = match port_status{
			traph::PortStatus::Child => {
				let element = traph.get_parent_element().chain_err(|| ErrorKind::CellagentError)?;
				// Need to coordinate the following with DiscoverMsg.update_discover_msg
				(element.get_hops()+1, element.get_path()) 
			},
			_ => (hops, path)
		};
		let traph_status = traph.get_port_status(port_number);
		let port_status = match traph_status {
			traph::PortStatus::Pruned => port_status,
			_ => traph_status  // Don't replace if Parent or Child
		};
		let gvm_result = match gvm_equation {
			Some(eqn) => eqn.evaluate(HashMap::new()).chain_err(|| ErrorKind::CellagentError)?,
			None => false,
		};
		if gvm_result { children.insert(PortNumber::new(0, self.no_ports)?); }
		let entry = traph.new_element(port_number, port_status, other_index, children, hops, path).chain_err(|| ErrorKind::CellagentError)?; 
// Here's the end of the transaction
		//println!("CellAgent {}: entry {}\n{}", self.cell_id, entry, traph); 
		traphs.insert(tree_id.stringify(), traph);
		{
			self.trees.lock().unwrap().insert(entry.get_index(), tree_id.stringify());
		}
		self.ca_to_pe.send(CaToPePacket::Entry(entry)).chain_err(|| ErrorKind::CellagentError)?;
		Ok(entry)
	}
	fn listen(&mut self, scope: &Scope, ca_from_pe: CaFromPe) -> Result<()>{
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
				PeToCaPacket::Status(port_no, status) => match status {
					port::PortStatus::Connected => self.port_connected(port_no).chain_err(|| ErrorKind::CellagentError)?,
					port::PortStatus::Disconnected => self.port_disconnected(port_no).chain_err(|| ErrorKind::CellagentError)?
				},
				PeToCaPacket::Packet(port_no, index, packet) => {
					if let Some(packets) = Packetizer::process_packet(&mut self.packet_assemblers, packet) {
						let (msg_type, serialized_msg) = MsgType::get_type_serialized(packets).chain_err(|| ErrorKind::CellagentError)?;
						if let Some(mut msg) = self.get_msg(msg_type, serialized_msg)? {
							//println!("CellAgent {}: got msg {}", self.cell_id, msg);
							msg.process(&mut self.clone(), port_no).chain_err(|| ErrorKind::CellagentError)?;
						};
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
			MsgType::SetupVM => Some(Box::new(serde_json::from_str::<SetupVMsMsg>(&serialized_msg).chain_err(|| ErrorKind::CellagentError)?)),
			_ => { // Not for cell agent
				let (up_tree_id, serialized) = serde_json::from_str::<(UpTreeID, String)>(&serialized_msg).chain_err(|| ErrorKind::CellagentError)?;
				if let Some(senders) = self.up_trees_senders.get(&up_tree_id) {
					for sender in senders.iter() { 
						sender.send(serialized.clone()).chain_err(|| ErrorKind::CellagentError)?;
					}
				} else {
					return Err(ErrorKind::UpTree(self.cell_id.clone(), up_tree_id.clone()).into());
				}
				None
			}
		})		
	}
	fn port_connected(&mut self, port_no: PortNo) -> Result<()> {
		//println!("CellAgent {}: port {} connected", self.cell_id, port_no);
		let tree_id = self.my_tree_id.clone();
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports).chain_err(|| ErrorKind::CellagentError)?);
		let path = Path::new(port_no, self.no_ports).chain_err(|| ErrorKind::CellagentError)?;
		self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
		let hops = 1;
		let my_table_index = self.my_entry.get_index();
		let msg = DiscoverMsg::new(&tree_id, my_table_index, &self.cell_id, hops, path);
		let other_index = 0;
		let direction = msg.get_header().get_direction();
		let bytes = Packetizer::serialize(&msg).chain_err(|| ErrorKind::CellagentError)?;
		let packets = Packetizer::packetize(bytes, direction, other_index).chain_err(|| ErrorKind::CellagentError)?;
		//println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, msg);
		let connected_tree_index = (*self.connected_tree_entry.lock().unwrap()).get_index();
		for packet in packets {
			let entry = CaToPePacket::Entry(*self.connected_tree_entry.lock().unwrap());
			self.ca_to_pe.send(entry).chain_err(|| ErrorKind::CellagentError)?;
			let packet_msg = CaToPePacket::Packet((connected_tree_index, port_no_mask, *packet));
			self.ca_to_pe.send(packet_msg).chain_err(|| ErrorKind::CellagentError)?;
		}
		let discover_msgs  = self.get_discover_msgs();
		//println!("CellAgent {}: {} discover msgs", ca.cell_id, discover_msgs.len());
		self.forward_discover(&discover_msgs, port_no_mask).chain_err(|| ErrorKind::CellagentError)?;
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
	pub fn forward_discover(&mut self, discover_msgs: &Vec<DiscoverMsg>, mask: Mask) -> Result<()> {
		let my_table_index = self.my_entry.get_index();
		let other_index = 0;
		for msg in discover_msgs.iter() {
			let direction = msg.get_header().get_direction();
			let bytes = Packetizer::serialize(msg).chain_err(|| ErrorKind::CellagentError)?;
			let packets = Packetizer::packetize(bytes, direction, other_index).chain_err(|| ErrorKind::CellagentError)?;
			self.send_msg(&self.connected_tree_id, packets, mask).chain_err(|| ErrorKind::CellagentError)?;
			//println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	pub fn send_msg(&self, tree_id: &TreeID, packets: Vec<Box<Packet>>, user_mask: Mask) 
			-> Result<()> {
		let index = {
			if let Some(traph) = self.traphs.lock().unwrap().get(tree_id.get_name()) {
				traph.get_table_index()			
			} else {
				return Err(ErrorKind::Tree(self.cell_id.clone(), tree_id.clone()).into());
			}
		};
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packet);
			let msg = CaToPePacket::Packet((index, user_mask, **packet));
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
		InvalidMsgType(cell_id: CellID) {
			description("Invalid message type")
			display("Invalid message type from packet assembler on cell {}", cell_id)
		}
		// Recursive type error if put in message.rs
		Message(cell_id: CellID, msg_no: usize) {
			description("Error processing message")
			display("Error processing message {} on cell {}", msg_no, cell_id)
		}		
		MessageAssembly(cell_id: CellID) {
			description("Problem assembling message")
			display("Problem assembling message on cell {}", cell_id)
		}
		PortTaken(cell_id: CellID, port_no: PortNo) {
			description("Port already assigned")
			display("Receiver for port {} has been previously assigned on cell {}", port_no, cell_id)
		}
		Recvr(cell_id: CellID, port_no: PortNo) {
			description("No channel receiver")
			display("No receiver for port {} on cell {}", port_no, cell_id)
		}
		Size(cell_id: CellID) {
			description("Routing table is full")
			display("No more room in routing table for cell {}", cell_id)
		}
		TenantMask(cell_id: CellID) {
			description("Tenant mask missing")
			display("Cell {} has no tenant mask", cell_id)
		}
		Tree(cell_id: CellID, tree_id: TreeID ) {
			description("Unknown tree")
			display("TreeID {} does not exist on cell {}", tree_id, cell_id)
		}
		TreeIndex(cell_id: CellID, index: TableIndex) {
			description("No tree for specified index")
			display("No tree associated with index {} on cell {}", index, cell_id)
		} 
		UpTree(cell_id: CellID, up_tree_id: UpTreeID) {
			description("UpTree not found")
			display("Cell {} has no uptree named {}", cell_id, up_tree_id)
		}
	}
}
