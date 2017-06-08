use std::fmt;
use std::str;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crossbeam::Scope;
use config::{MAX_ENTRIES, PathLength, PortNo, TableIndex};
use nalcell::{CaToPe, CaFromPe, CaToPeMsg};
use message::{DiscoverMsg};
use name::{Name, CellID, TreeID};
use packet::{Packet, Packetizer};
use port;
use routing_table_entry::{RoutingTableEntry, RoutingTableEntryError};
use traph;
use traph::{Traph};
use utility::{BASE_TENANT_MASK, Mask, Path, PortNumber, PortNumberError};

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";

#[derive(Debug, Clone)]
pub struct CellAgent {
	cell_id: CellID,
	my_tree_id: TreeID,
	no_ports: PortNo,
	control_tree_id: TreeID,
	my_entry: RoutingTableEntry,
	connected_tree_entry: Arc<Mutex<RoutingTableEntry>>,
	connected_ports_tree_id: TreeID,
	discover_msgs: Arc<Mutex<Vec<DiscoverMsg>>>,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<HashMap<TableIndex,String>>>,
	traphs: Arc<Mutex<HashMap<String,Traph>>>,
	tenant_masks: Vec<Mask>,
	ca_to_pe: CaToPe,
}
#[deny(unused_must_use)]
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, no_ports: PortNo, ca_from_pe: CaFromPe, ca_to_pe: CaToPe ) 
				-> Result<CellAgent, CellAgentError> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let my_tree_id = TreeID::new(cell_id.get_name())?;
		let control_tree_id = TreeID::new(CONTROL_TREE_NAME)?;
		let connected_tree_id = TreeID::new(CONNECTED_PORTS_TREE_NAME)?;
		let mut free_indices = Vec::new();
		let trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..MAX_ENTRIES { 
			free_indices.push(i as TableIndex); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		let mut ca = CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id.clone(), 
			no_ports: no_ports, traphs: traphs, control_tree_id: control_tree_id.clone(), 
			connected_ports_tree_id: connected_tree_id.clone(), free_indices: Arc::new(Mutex::new(free_indices)),
			discover_msgs: Arc::new(Mutex::new(Vec::new())), my_entry: RoutingTableEntry::default(0)?, 
			connected_tree_entry: Arc::new(Mutex::new(RoutingTableEntry::default(0)?)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), 
			ca_to_pe: ca_to_pe};
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = PortNumber::new(0, no_ports)?;
		let other_index = 0;
		let hops = 0;
		let path = None;
		let children = vec![port_number_0];
		ca.update_traph(&control_tree_id, port_number_0, 
				traph::PortStatus::Parent, &children, other_index, hops, path)?;
		let connected_tree_entry = ca.update_traph(&connected_tree_id, port_number_0, 
				traph::PortStatus::Parent, &Vec::new(), other_index, hops, path)?;
		ca.connected_tree_entry = Arc::new(Mutex::new(connected_tree_entry));
		// Create my tree
		let my_entry = ca.update_traph(&my_tree_id, port_number_0, 
				traph::PortStatus::Parent, &children, other_index, hops, path)?; 
		ca.my_entry = my_entry;
		ca.listen(scope, ca_from_pe)?;
		Ok(ca)
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_tree_id(&self, index: TableIndex) -> Result<String, CellAgentError> {
		let trees = self.trees.lock().unwrap();
		let tree_id = match trees.get(&index) {
			Some(t) => t.clone(),
			None => {
				println!("--- CellAgent {}: index {} in trees table {:?}", self.cell_id, index, *trees);
				return Err(CellAgentError::TreeIndex(TreeIndexError::new(index)))}
			
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
	pub fn get_connected_ports_tree_id(&self) -> TreeID { self.connected_ports_tree_id.clone() }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(tree_id.get_name())
	}
	fn use_index(&mut self) -> Result<TableIndex,CellAgentError> {
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(CellAgentError::Size(SizeError::new()))
		}
	}
	pub fn update_traph(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: traph::PortStatus, 
				children: &Vec<PortNumber>, other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry, CellAgentError> {
		// Note that traphs is updated transactionally; I remove an entry, update it, then put it back.
		let mut traphs = self.traphs.lock().unwrap();
		let mut traph = match traphs.remove(tree_id.get_name()) { // Avoids lifetime problem
			Some(t) => t,
			None => Traph::new(tree_id.clone(), self.clone().use_index()?)?
		};
		let traph_status = traph.get_port_status(port_number);
		let port_status = match traph_status {
			traph::PortStatus::Pruned => port_status,
			_ => traph_status  // Don't replace if Parent or Child
		};
		let entry = traph.new_element(port_number, port_status, other_index, children, hops, path)?;
		// Here's the end of the transaction
		traphs.insert(tree_id.stringify(), traph);
		{
			self.trees.lock().unwrap().insert(entry.get_index(), tree_id.stringify());
		}
		match self.ca_to_pe.send((Some(entry),None)){
			Ok(_) => (),
			Err(err) => {
				println!("CellAgent {}: update_traph EntryCaToPe error {}", self.cell_id, err);
				return Err(CellAgentError::SendCaPe(err));
			}
		};
		Ok(entry)
	}
	pub fn add_child(&self, tree_id: &String, port_no: PortNo, other_index: TableIndex)
			-> Result<(), CellAgentError> {
		let mut traphs = self.traphs.lock().unwrap();
		if let Some(mut traph) = traphs.remove(tree_id) { // Avoids a lifetime error
			let port_number = PortNumber::new(port_no, self.no_ports)?;
			let entry = traph.add_child(port_number, other_index)?;
			traphs.insert(tree_id.clone(),traph);
			println!("CellAgent {}: child {}   {}", self.cell_id, tree_id, entry);
			match self.ca_to_pe.send((Some(entry),None)) {
				Ok(_) => (),
				Err(err) => {
					println!("CellAgent {}: add_child EntryCaToPe error {}", self.cell_id, err);
					return Err(CellAgentError::SendCaPe(err));
				}
			};
		} else {
			println!("CellAgent {}: add_child tree {} does not exist in traphs {:?}", self.cell_id, tree_id, traphs.keys());
			return Err(CellAgentError::Tree(TreeError::new(tree_id)));
		}
		Ok(())
	}
	fn listen(&mut self, scope: &Scope, ca_from_pe: CaFromPe) -> Result<(), CellAgentError>{
		let mut ca = self.clone();
		scope.spawn( move || -> Result<(), CellAgentError> { 
			match ca.listen_loop(ca_from_pe) {
				Ok(val) => Ok(val),
				Err(err) => {
					println!("--- CellAgent {}: {}", ca.cell_id, err);
					Err(err)
				}
			}
		});
		Ok(())
	}
	fn listen_loop(&mut self, ca_from_pe: CaFromPe) -> Result<(), CellAgentError> {
		loop {
			//println!("CellAgent {}: waiting for status or packet", ca.cell_id);
			let (opt_status, opt_packet) = ca_from_pe.recv()?; 
			match opt_status {
				Some((port_no, status)) => {
					//println!("CellAgent {}: got status on port {}", ca.cell_id, port_no);
					match status {
						port::PortStatus::Connected => self.port_connected(port_no)?,
						port::PortStatus::Disconnected => self.port_disconnected(port_no)?
					};
				},
				None => match opt_packet {
					Some((port_no, index, packet)) => self.process_packets(port_no, index, packet)?,
					None => ()
				}
			};
		}
	}
	fn process_packets(&mut self, port_no: PortNo, my_index: TableIndex, packet: Packet) 
				-> Result<(), CellAgentError> {
		let mut packet_assembler: HashMap<u64, Vec<Box<Packet>>> = HashMap::new();
		let header = packet.get_header();
		let uniquifier = header.get_uniquifier();
		let packets = packet_assembler.entry(uniquifier).or_insert(Vec::new());
		packets.push(Box::new(packet));
		if header.is_last_packet() {
			let mut msg = Packetizer::unpacketize(packets)?;
			println!("CellAgent {}: port {} got packet {} msg {} ", self.cell_id, port_no, packets[0].get_count(), msg);							
			msg.process(&mut self.clone(), port_no)?;
// Need to update entry for my index with other_index
		}
		Ok(())
	}
	fn port_connected(&mut self, port_no: PortNo) -> Result<(), CellAgentError> {
		println!("CellAgent {}: port {} connected", self.cell_id, port_no);
		let tree_id = self.my_tree_id.clone();
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
		let path = Path::new(port_no, self.no_ports)?;
		self.connected_tree_entry.lock().unwrap().or_with_mask(port_no_mask);
		let hops = 1;
		let my_table_index = self.my_entry.get_index();
		let msg = DiscoverMsg::new(tree_id.clone(), my_table_index, self.cell_id.clone(), hops, path);
		let other_index = 0;
		let packets = Packetizer::packetize(&msg, other_index)?;
		println!("CellAgent {}: sending packet {} on port {} {} ", self.cell_id, packets[0].get_count(), port_no, msg);
		let index = (*self.connected_tree_entry.lock().unwrap()).get_index();
		for packet in packets {
			self.ca_to_pe.send((Some(*self.connected_tree_entry.lock().unwrap()), 
			                    Some((index, port_no_mask, *packet))))?;
		}
		let discover_msgs  = self.get_discover_msgs();
		//println!("CellAgent {}: {} discover msgs", ca.cell_id, discover_msgs.len());
		self.forward_discover(&discover_msgs, port_no_mask)?;
		Ok(())		
	}
	fn port_disconnected(&self, port_no: PortNo) -> Result<(), CellAgentError> {
		println!("Cell Agent {} got disconnected on port {}", self.cell_id, port_no);
		let port_no_mask = Mask::new(PortNumber::new(port_no, self.no_ports)?);
		self.connected_tree_entry.lock().unwrap().and_with_mask(port_no_mask.not());
		self.ca_to_pe.send((Some(*self.connected_tree_entry.lock().unwrap()),None))?;	
		Ok(())	
	}		
	pub fn forward_discover(&mut self, discover_msgs: &Vec<DiscoverMsg>, mask: Mask) -> Result<(), CellAgentError> {
		let my_table_index = self.my_entry.get_index();
		for msg in discover_msgs.iter() {
			let packets = Packetizer::packetize(msg, my_table_index)?;
			self.send_msg(&self.connected_ports_tree_id, packets, mask)?;
			println!("CellAgent {}: forward on ports {:?} {}", self.cell_id, mask.get_port_nos(), msg);
		}
		Ok(())	
	}
	pub fn send_msg(&self, tree_id: &TreeID, packets: Vec<Box<Packet>>, user_mask: Mask) 
			-> Result<(), CellAgentError> {
		let index = {
			if let Some(traph) = self.traphs.lock().unwrap().get(tree_id.get_name()) {
				traph.get_table_index()			
			} else {
				return Err(CellAgentError::Tree(TreeError::new(tree_id.get_name())));
			}
		};
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packets[0].get_packet_count());
			self.ca_to_pe.send((None, Some((index, user_mask, **packet))))?;
			//println!("CellAgent {}: sent packet {} on tree {} to packet engine with index {}", self.cell_id, packet_count, tree_id, index);
		}
		Ok(())
	}
}
impl fmt::Display for CellAgent { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!(" Cell Agent");
		for (_, traph) in self.traphs.lock().unwrap().iter() {
			s = s + &format!("{}", traph);
		}
		write!(f, "{}", s) }
}
// Errors
use std::error::Error;
use std::sync::mpsc::{SendError, RecvError};
use message::ProcessMsgError;
use name::NameError;
use packet::{PacketizerError};
use routing_table::RoutingTableError;
use traph::TraphError;
use utility::{MaskError, UtilityError};
#[derive(Debug)]
pub enum CellAgentError {
	Name(NameError),
	Size(SizeError),
	Tree(TreeError),
	Mask(MaskError),
	TenantMask(TenantMaskError),
	TreeIndex(TreeIndexError),
	Traph(TraphError),
	Packetizer(PacketizerError),
	PortNumber(PortNumberError),
	PortTaken(PortTakenError),
	ProcessMsg(ProcessMsgError),
	InvalidMsgType(InvalidMsgTypeError),
	MsgAssembly(MsgAssemblyError),
//	BadPacket(BadPacketError),
	Utility(UtilityError),
	Routing(RoutingTableError),
	RoutingTableEntry(RoutingTableEntryError),
	SendTableEntry(SendError<CaToPeMsg>),
	SendCaPe(SendError<CaToPeMsg>),
	Recv(RecvError),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Packetizer(ref err) => err.description(),
			CellAgentError::PortNumber(ref err) => err.description(),
			CellAgentError::PortTaken(ref err) => err.description(),
			CellAgentError::ProcessMsg(ref err) => err.description(),
//			CellAgentError::BadPacket(ref err) => err.description(),
			CellAgentError::InvalidMsgType(ref err) => err.description(),
			CellAgentError::MsgAssembly(ref err) => err.description(),
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::Size(ref err) => err.description(),
			CellAgentError::Tree(ref err) => err.description(),
			CellAgentError::Mask(ref err) => err.description(),
			CellAgentError::TenantMask(ref err) => err.description(),
			CellAgentError::TreeIndex(ref err) => err.description(),
			CellAgentError::Traph(ref err) => err.description(),
			CellAgentError::Utility(ref err) => err.description(),
			CellAgentError::Routing(ref err) => err.description(),
			CellAgentError::RoutingTableEntry(ref err) => err.description(),
			CellAgentError::SendTableEntry(ref err) => err.description(),
			CellAgentError::SendCaPe(ref err) => err.description(),
			CellAgentError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Packetizer(ref err) => Some(err),
			CellAgentError::PortNumber(ref err) => Some(err),
			CellAgentError::PortTaken(ref err) => Some(err),
			CellAgentError::ProcessMsg(ref err) => Some(err),
//			CellAgentError::BadPacket(ref err) => Some(err),
			CellAgentError::InvalidMsgType(ref err) => Some(err),
			CellAgentError::MsgAssembly(ref err) => Some(err),
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::Size(ref err) => Some(err),
			CellAgentError::Tree(ref err) => Some(err),
			CellAgentError::Mask(ref err) => Some(err),
			CellAgentError::TenantMask(ref err) => Some(err),
			CellAgentError::TreeIndex(ref err) => Some(err),
			CellAgentError::Traph(ref err) => Some(err),
			CellAgentError::Utility(ref err) => Some(err),
			CellAgentError::Routing(ref err) => Some(err),
			CellAgentError::RoutingTableEntry(ref err) => Some(err),
			CellAgentError::SendTableEntry(ref err) => Some(err),
			CellAgentError::SendCaPe(ref err) => Some(err),
			CellAgentError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Packetizer(ref err) => write!(f, "Cell Agent Packetizer Error caused by {}", err),
			CellAgentError::PortNumber(ref err) => write!(f, "Cell Agent PortNumber Error caused by {}", err),
			CellAgentError::PortTaken(ref err) => write!(f, "Cell Agent PortNumber Error caused by {}", err),
			CellAgentError::ProcessMsg(ref err) => write!(f, "Cell Agent ProcessMsg Error caused by {}", err),
//			CellAgentError::BadPacket(ref err) => write!(f, "Cell Agent Bad Packet Error caused by {}", err),
			CellAgentError::InvalidMsgType(ref err) => write!(f, "Cell Agent Invalid Message Type Error caused by {}", err),
			CellAgentError::MsgAssembly(ref err) => write!(f, "Cell Agent Message Assembly Error caused by {}", err),
			CellAgentError::Name(ref err) => write!(f, "Cell Agent Name Error caused by {}", err),
			CellAgentError::Size(ref err) => write!(f, "Cell Agent Size Error caused by {}", err),
			CellAgentError::Tree(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::Mask(ref err) => write!(f, "Cell Agent Mask Error caused by {}", err),
			CellAgentError::TenantMask(ref err) => write!(f, "Cell Agent Tenant Mask Error caused by {}", err),
			CellAgentError::TreeIndex(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::Traph(ref err) => write!(f, "Cell Agent Traph Error caused by {}", err),
			CellAgentError::Utility(ref err) => write!(f, "Cell Agent Utility Error caused by {}", err),
			CellAgentError::Routing(ref err) => write!(f, "Cell Agent Routing Table Error caused by {}", err),
			CellAgentError::RoutingTableEntry(ref err) => write!(f, "Cell Agent Routing Table Entry Error caused by {}", err),
			CellAgentError::SendTableEntry(ref err) => write!(f, "Cell Agent Send Table Entry Error caused by {}", err),
			CellAgentError::SendCaPe(ref err) => write!(f, "Cell Agent Send Packet to Packet Engine Error caused by {}", err),
			CellAgentError::Recv(ref err) => write!(f, "Cell Agent Receive Error caused by {}", err),
		}
	}
}
impl From<NameError> for CellAgentError {
	fn from(err: NameError) -> CellAgentError { CellAgentError::Name(err) }
}
impl From<TraphError> for CellAgentError {
	fn from(err: TraphError) -> CellAgentError { CellAgentError::Traph(err) }
}
impl From<UtilityError> for CellAgentError {
	fn from(err: UtilityError) -> CellAgentError { CellAgentError::Utility(err) }
}
impl From<RoutingTableError> for CellAgentError {
	fn from(err: RoutingTableError) -> CellAgentError { CellAgentError::Routing(err) }
}
impl From<RoutingTableEntryError> for CellAgentError {
	fn from(err: RoutingTableEntryError) -> CellAgentError { CellAgentError::RoutingTableEntry(err) }
}
impl From<SendError<CaToPeMsg>> for CellAgentError {
	fn from(err: SendError<CaToPeMsg>) -> CellAgentError { CellAgentError::SendTableEntry(err) }
}
impl From<RecvError> for CellAgentError {
	fn from(err: RecvError) -> CellAgentError { CellAgentError::Recv(err) }
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError { 
	pub fn new() -> SizeError {
		SizeError { msg: format!("No more room in routing table") }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<SizeError> for CellAgentError {
	fn from(err: SizeError) -> CellAgentError { CellAgentError::Size(err) }
}
impl From<MaskError> for CellAgentError {
	fn from(err: MaskError) -> CellAgentError { CellAgentError::Mask(err) }
}
#[derive(Debug)]
pub struct TenantMaskError { msg: String }
impl TenantMaskError { 
//	pub fn new(cell_id: CellID) -> TenantMaskError {
//		TenantMaskError { msg: format!("Cell {} has no tenant mask", cell_id) }
//	}
}
impl Error for TenantMaskError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for TenantMaskError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<TenantMaskError> for CellAgentError {
	fn from(err: TenantMaskError) -> CellAgentError { CellAgentError::TenantMask(err) }
}
#[derive(Debug)]
pub struct InvalidMsgTypeError { msg: String }
impl InvalidMsgTypeError { 
//	pub fn new() -> InvalidMsgTypeError {
//		InvalidMsgTypeError { msg: format!("Problem with packet assembler") }
//	}
}
impl Error for InvalidMsgTypeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for InvalidMsgTypeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<InvalidMsgTypeError> for CellAgentError {
	fn from(err: InvalidMsgTypeError) -> CellAgentError { CellAgentError::InvalidMsgType(err) }
}
#[derive(Debug)]
pub struct MsgAssemblyError { msg: String }
impl MsgAssemblyError { 
//	pub fn new() -> MsgAssemblyError {
//		MsgAssemblyError { msg: format!("Problem with packet assembler") }
//	}
}
impl Error for MsgAssemblyError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for MsgAssemblyError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<MsgAssemblyError> for CellAgentError {
	fn from(err: MsgAssemblyError) -> CellAgentError { CellAgentError::MsgAssembly(err) }
}
#[derive(Debug)]
pub struct TreeError { msg: String }
impl TreeError { 
	pub fn new(tree_id: &str) -> TreeError {
		TreeError { msg: format!("TreeID {} does not exist", tree_id) }
	}
}
impl Error for TreeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for TreeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<TreeError> for CellAgentError {
	fn from(err: TreeError) -> CellAgentError { CellAgentError::Tree(err) }
}
#[derive(Debug)]
pub struct TreeIndexError { msg: String }
impl TreeIndexError { 
	pub fn new(index: TableIndex) -> TreeIndexError {
		TreeIndexError { msg: format!("No tree associated with index {}", index) }
	}
}
impl Error for TreeIndexError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for TreeIndexError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<TreeIndexError> for CellAgentError {
	fn from(err: TreeIndexError) -> CellAgentError { CellAgentError::TreeIndex(err) }
}
#[derive(Debug)]
pub struct PortTakenError { msg: String }
impl PortTakenError { 
	pub fn new(port_no: PortNo) -> PortTakenError {
		PortTakenError { msg: format!("Receiver for port {} has been previously assigned", port_no) }
	}
}
impl Error for PortTakenError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortTakenError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PortTakenError> for CellAgentError {
	fn from(err: PortTakenError) -> CellAgentError { CellAgentError::PortTaken(err) }
}
#[derive(Debug)]
pub struct RecvrError { msg: String }
impl RecvrError { 
	pub fn new(port_no: PortNo) -> RecvrError {
		RecvrError { msg: format!("No receiver for port {} ", port_no) }
	}
}
impl Error for RecvrError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for RecvrError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PacketizerError> for CellAgentError {
	fn from(err: PacketizerError) -> CellAgentError { CellAgentError::Packetizer(err) }
}
impl From<PortNumberError> for CellAgentError {
	fn from(err: PortNumberError) -> CellAgentError { CellAgentError::PortNumber(err) }
}
impl From<ProcessMsgError> for CellAgentError {
	fn from(err: ProcessMsgError) -> CellAgentError { CellAgentError::ProcessMsg(ProcessMsgError::new(&err)) }
}
