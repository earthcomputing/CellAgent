use std::fmt;
use std::str;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crossbeam::Scope;
use config::{MAX_ENTRIES, PathLength, PortNo, TableIndex};
use nalcell::{EntryCaToPe, StatusCaFromPort, RecvrCaToPort, RecvrSendError,
		PacketRecv, PacketSendError, PacketCaToPe, PacketCaFromPe, PacketPortToPe};
use message::{Message, DiscoverMsg, MsgType};
use name::{Name, CellID, TreeID};
use packet::{Packet, Packetizer};
use packet_engine::PacketEngine;
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
	connected_ports_tree_id: TreeID,
	discover_msgs: Vec<DiscoverMsg>,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<HashMap<TableIndex,TreeID>>>,
	traphs: Arc<Mutex<HashMap<TreeID,Traph>>>,
	tenant_masks: Vec<Mask>,
	entry_ca_to_pe: EntryCaToPe,
	packet_port_to_pe: PacketPortToPe,
	packet_ca_to_pe: PacketCaToPe,
	recvr_ca_to_ports: Vec<RecvrCaToPort>,
	packet_engine: PacketEngine,
}
#[deny(unused_must_use)]
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, no_ports: PortNo, packet_port_to_pe: PacketPortToPe, 
			packet_engine: PacketEngine, packet_ca_to_pe: PacketCaToPe, packet_ca_from_pe: PacketCaFromPe, 
			entry_ca_to_pe: EntryCaToPe, recv_status_from_port: StatusCaFromPort, recvr_ca_to_ports: Vec<RecvrCaToPort>, 
			packet_ports_from_pe: HashMap<PortNo,PacketRecv>) 
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
			discover_msgs: Vec::new(),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), 
			packet_ca_to_pe: packet_ca_to_pe.clone(), entry_ca_to_pe: entry_ca_to_pe,
			packet_port_to_pe: packet_port_to_pe, packet_engine: packet_engine,
			recvr_ca_to_ports: recvr_ca_to_ports};
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = PortNumber::new(0, no_ports)?;
		let other_index = 0;
		let hops = 0;
		let mut control_entry = ca.update_traph(control_tree_id, port_number_0, 
				traph::PortStatus::Parent, vec![port_number_0], other_index, hops, None)?;
		let mut connected_entry = ca.update_traph(connected_tree_id.clone(), port_number_0, 
				traph::PortStatus::Parent, vec![port_number_0], other_index, hops, None)?;
		// Create my tree
		let mut my_entry = ca.update_traph(my_tree_id, port_number_0, 
				traph::PortStatus::Parent, vec![port_number_0], other_index, hops, None)?; 
		ca.port_status(scope, connected_entry, my_entry, recv_status_from_port, packet_ports_from_pe)?;
		ca.recv_packets(scope, packet_ca_from_pe)?;
		Ok(ca)
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_tree_id(&self, index: TableIndex) -> Result<TreeID, CellAgentError> {
		println!("CellAgent {}: trees table {:?}", self.cell_id, *self.trees.lock().unwrap());
		let tree_id = match self.trees.lock().unwrap().get(&index) {
			Some(t) => t.clone(),
			None => return Err(CellAgentError::TreeIndex(TreeIndexError::new(index)))
		};
		Ok(tree_id)
	}
	//pub fn get_tenant_mask(&self) -> Result<&Mask, CellAgentError> {
	//	if let Some(tenant_mask) = self.tenant_masks.last() {
	//		Ok(tenant_mask)
	//	} else {
	//		return Err(CellAgentError::TenantMask(TenantMaskError::new(self.get_id())))
	//	}
	//}
	//pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn add_discover_msg(&mut self, msg: DiscoverMsg) { 
		println!("CellAgent {}: added msg {} as entry {} for tree {}", self.cell_id, msg.get_count(), self.discover_msgs.len()+1, msg); 
		self.discover_msgs.push(msg);
	}
	pub fn get_connected_ports_tree_id(&self) -> TreeID { self.connected_ports_tree_id.clone() }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(tree_id)
	}
	fn use_index(&mut self) -> Result<TableIndex,CellAgentError> {
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(CellAgentError::Size(SizeError::new()))
		}
	}
	pub fn update_traph(&mut self, tree_id: TreeID, port_number: PortNumber, port_status: traph::PortStatus, 
				children: Vec<PortNumber>, other_index: TableIndex, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry, CellAgentError> {
		// Note that traphs is updated transactionally; I remove an entry, update it, then put it back.
		let mut traphs = self.traphs.lock().unwrap();
		let mut traph = match traphs.remove(&tree_id) { // Avoids lifetime problem
			Some(t) => t,
			None => Traph::new(tree_id.clone(), self.clone().use_index()?)?
		};
		let entry = traph.add_element(port_number, port_status, children, hops, path)?;
		// Here's the end of the transaction
		traphs.insert(tree_id.clone(), traph);
		{
			self.trees.lock().unwrap().insert(entry.get_index(), tree_id.clone());
		}
		println!("CellAgent {}: Tree {:10} {}", self.cell_id, tree_id, entry);
		self.entry_ca_to_pe.send((entry,None))?;
		Ok(entry)
	}
	pub fn add_child(&self, tree_id: &TreeID, port_no: PortNo, other_index: TableIndex)
			-> Result<(), CellAgentError> {
		let mut traphs = self.traphs.lock().unwrap();
		if let Some(mut traph) = traphs.remove(&tree_id) {
			let port_number = PortNumber::new(port_no, self.no_ports)?;
			let mut entry = traph.add_child(port_number)?;
			entry.set_other_index(port_number, other_index);
			println!("CellAgent {}: child {}   {}", self.cell_id, tree_id, entry);
			self.entry_ca_to_pe.send((entry,None))?;
		} else {
			return Err(CellAgentError::Tree(TreeError::new(tree_id)));
		}
		Ok(())
	}
	fn port_status(&mut self, scope: &Scope, connected_tree_entry: RoutingTableEntry,
			my_entry: RoutingTableEntry, status_ca_from_port: StatusCaFromPort, 
			mut packet_ports_from_pe: HashMap<PortNo,PacketRecv>) -> Result<(), CellAgentError>{
		let tree_id = self.my_tree_id.clone();
		let my_table_index = my_entry.get_index();
		let entry_ca_to_pe = self.entry_ca_to_pe.clone();
		let mut connected_entry = connected_tree_entry.clone();	
		let mut ca = self.clone();
		let no_ports = self.get_no_ports();
		scope.spawn( move || -> Result<(), CellAgentError> {
			//println!("CellAgent {}: waiting for status", ca.cell_id);	
			loop {
				let (port_no, status) = try!(status_ca_from_port.recv());
				//println!("CellAgent {}: got status on port {}", ca.cell_id, port_no);
				let path = Path::new(port_no, no_ports)?;
				let port_no_mask = Mask::new(port_no)?;
				match status {
					port::PortStatus::Connected => {
						println!("CellAgent {}: port {} connected", ca.cell_id, port_no);
						if let Some(packet_port_from_pe) = packet_ports_from_pe.remove(&port_no) {
							if let Some(recvr) = ca.recvr_ca_to_ports.get(port_no as usize) {
								//println!("CellAgent {}: sending recvr to port {}", ca.cell_id, port_no);
								recvr.send(packet_port_from_pe)?;	
							} else {
								println!("CellAgent {}: error sending recvr on port {}", ca.cell_id, port_no);
								return Err(CellAgentError::Recvr(RecvrError::new(port_no)));
							}				
						} else {
							println!("CellAgent {}: port {} already connected", ca.cell_id, port_no);
							return Err(CellAgentError::PortTaken(PortTakenError::new(port_no)))
						};
						connected_entry.or_with_mask(port_no_mask);
						//println!("CellAgent {}: connected entry {}", ca.cell_id, connected_entry);
						let hops = 1;
						let msg = DiscoverMsg::new(tree_id.clone(), ca.cell_id.clone(), hops, path);
						ca.discover_msgs.push(msg.clone());
						println!("CellAgent {}: {} old msgs", ca.cell_id, ca.discover_msgs.len());
						for msg in &ca.discover_msgs {
							if ca.discover_msgs.len() > 1 {
								println!("CellAgent {}: old msg {}", ca.cell_id, msg);
							}						
							let packets = Packetizer::packetize(msg, my_table_index, [false;4])?;
							println!("CellAgent {}: sending msg {} discover", ca.cell_id, msg.get_count());
							// Assumes Discover message fits in one packet and path doesn't change when forwarded
							ca.entry_ca_to_pe.send((connected_entry, Some((port_no_mask,*packets[0]))))?;
						}
						ca.discover_msgs.pop(); // This DiscoverMsg is different for each port
						//println!("CellAgent {}: sent packet {} on port {} as msg {}", ca.cell_id, packets[0].get_packet_count(), port_no, msg.get_count());
						//try!(ca.send_msg(&connected_tree_id, packets, port_no_mask));
					},
					port::PortStatus::Disconnected => {
						println!("Cell Agent {} got disconnected on port {}", ca.cell_id, port_no);
						connected_entry.and_with_mask(port_no_mask.not());
						entry_ca_to_pe.send((connected_entry,None))?;
					}
				}
 			}
		});
		Ok(())
	}				
	pub fn send_msg(&self, tree_id: &TreeID, packets: Vec<Box<Packet>>, user_mask: Mask) -> Result<(), CellAgentError> 
			 {
		let index;
		{
			if let Some(traph) = self.traphs.lock().unwrap().get(&tree_id) {
				index = traph.get_table_index();			
			} else {
				return Err(CellAgentError::Tree(TreeError::new(&tree_id)));
			};
		}
		for packet in packets.iter() {
			//println!("CellAgent {}: Sending packet {}", self.cell_id, packets[0].get_packet_count());
			let packet_count = packet.get_packet_count();
			self.packet_ca_to_pe.send((packet_count, index, user_mask, **packet))?;
			//println!("CellAgent {}: sent packet {} on tree {} to packet engine with index {}", self.cell_id, packet_count, tree_id, index);
		}
		Ok(())
	}
	fn recv_packets(&self, scope: &Scope, packet_ca_from_pe: PacketCaFromPe) -> Result<(), CellAgentError> {
		let mut packet_assembler: HashMap<u64, Vec<Box<Packet>>> = HashMap::new();
		let mut ca = self.clone();
		scope.spawn( move || -> Result<(), CellAgentError> {
			loop {
				let (_, port_no, packet) = packet_ca_from_pe.recv()?;
				//let (packet_count, port_no, packet) = packet_ca_from_pe.recv()?;
				let header = packet.get_header();
				let my_index = header.get_other_index();
				//let is_rootcast = header.is_rootcast();
				//println!("CellAgent {}: got packet {} rootward? {} is_last? {}", ca.cell_id, packet_count, is_rootcast, header.is_last_packet());
				let uniquifier = header.get_uniquifier();
				let packets = packet_assembler.entry(uniquifier).or_insert(Vec::new());
				packets.push(Box::new(packet));
				if header.is_last_packet() {
					//println!("CellAgent {}: unpacketizing packet {}", ca.cell_id, packet_count);
					let msg = Packetizer::unpacketize(packets)?;
					println!("CellAgent {}: got msg {} on port {} {}", ca.cell_id, msg.get_count(), port_no, msg);							
					msg.process(&mut ca, port_no, my_index)?;
				}
			}	
		});
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
	SendTableEntry(SendError<(RoutingTableEntry,Option<(Mask,Packet)>)>),
	SendCaPe(SendError<(usize,u32,Mask,Packet)>),
	SendPacket(PacketSendError),
	Recvr(RecvrError),
	RecvrSend(RecvrSendError),
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
			CellAgentError::SendPacket(ref err) => err.description(),
			CellAgentError::Recvr(ref err) => err.description(),
			CellAgentError::RecvrSend(ref err) => err.description(),
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
			CellAgentError::SendPacket(ref err) => Some(err),
			CellAgentError::Recvr(ref err) => Some(err),
			CellAgentError::RecvrSend(ref err) => Some(err),
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
			CellAgentError::SendPacket(ref err) => write!(f, "Cell Agent Send Packet to Packet Engine Error caused by {}", err),
			CellAgentError::Recvr(ref err) => write!(f, "Cell Agent Receiver Error caused by {}", err),
			CellAgentError::RecvrSend(ref err) => write!(f, "Cell Agent Send Receiver Error caused by {}", err),
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
impl From<SendError<(RoutingTableEntry,Option<(Mask,Packet)>)>> for CellAgentError {
	fn from(err: SendError<(RoutingTableEntry, Option<(Mask,Packet)>)>) 
			-> CellAgentError { CellAgentError::SendTableEntry(err) }
}
impl From<SendError<(usize,u32,Mask,Packet)>> for CellAgentError {
	fn from(err: SendError<(usize,u32,Mask,Packet)>) -> CellAgentError { CellAgentError::SendCaPe(err) }
}
impl From<SendError<(usize,Packet)>> for CellAgentError {
	fn from(err: SendError<(usize,Packet)>) -> CellAgentError { CellAgentError::SendPacket(err) }
}
impl From<RecvError> for CellAgentError {
	fn from(err: RecvError) -> CellAgentError { CellAgentError::Recv(err) }
}
impl From<RecvrSendError> for CellAgentError {
	fn from(err: RecvrSendError) -> CellAgentError { CellAgentError::RecvrSend(err) }
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
	pub fn new(tree_id: &TreeID) -> TreeError {
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
impl From<RecvrError> for CellAgentError {
	fn from(err: RecvrError) -> CellAgentError { CellAgentError::Recvr(err) }
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
