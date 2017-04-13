use std::fmt;
use std::str;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{SendError, RecvError};
use std::hash::Hash;
use std::collections::HashMap;
use serde;
use crossbeam::Scope;
use config::{MAX_ENTRIES, MAX_PORTS, Mask, TableIndex, PortNo};
use nalcell::{PortNumber, PortNumberError, EntryCaToPe, StatusCaFromPort, RecvrCaToPort, RecvrSendError,
		PacketRecv, PacketSendError, PacketCaToPe, PacketCaFromPe, PacketPortToPe,
		TenantMaskCaToPe, TenantMaskSendError};
use message::{Message, DiscoverMsg};
use name::{Name, CellID, TreeID};
use packet::{Packet, Packetizer, PacketizerError, UnpacketizeError};
use packet_engine::PacketEngine;
use port;
use routing_table::RoutingTableError;
use routing_table_entry::RoutingTableEntry;
use traph;
use traph::{Traph, TraphError};
use utility::{int_to_mask, mask_from_port_nos, UnimplementedError};

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";
const BASE_TENANT_MASK: Mask = 255;   // All ports
const DEFAULT_USER_MASK: Mask = 254;  // All ports except port 0

pub const DEFAULT_OTHER_INDICES: [TableIndex; MAX_PORTS as usize] = [0; MAX_PORTS as usize];

#[derive(Debug, Clone)]
pub struct CellAgent {
	cell_id: CellID,
	my_tree_id: TreeID,
	no_ports: PortNo,
	control_tree_id: TreeID,
	connected_ports_tree_id: TreeID,
	free_indices: Arc<Mutex<Vec<TableIndex>>>,
	trees: Arc<Mutex<HashMap<TableIndex,TreeID>>>,
	traphs: Arc<Mutex<HashMap<TreeID,Traph>>>,
	tenant_masks: Vec<Mask>,
	entry_ca_to_pe: EntryCaToPe,
	packet_port_to_pe: PacketPortToPe,
	packet_ca_to_pe: PacketCaToPe,
	tenant_ca_to_pe: TenantMaskCaToPe,
	recvr_ca_to_ports: Vec<RecvrCaToPort>,
	packet_engine: PacketEngine,
}
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, no_ports: PortNo, packet_port_to_pe: PacketPortToPe, 
			packet_engine: PacketEngine, packet_ca_to_pe: PacketCaToPe, packet_ca_from_pe: PacketCaFromPe, 
			entry_ca_to_pe: EntryCaToPe, recv_status_from_port: StatusCaFromPort, recvr_ca_to_ports: Vec<RecvrCaToPort>, 
			packet_ports_from_pe: HashMap<PortNo,PacketRecv>,
			tenant_ca_to_pe: TenantMaskCaToPe) -> Result<CellAgent, CellAgentError> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let my_tree_id = try!(TreeID::new(cell_id.get_name()));
		let control_tree_id = try!(TreeID::new(CONTROL_TREE_NAME));
		let connected_tree_id = try!(TreeID::new(CONNECTED_PORTS_TREE_NAME));
		let mut free_indices = Vec::new();
		let mut trees = HashMap::new(); // For getting TreeID from table index
		for i in 0..MAX_ENTRIES { 
			trees.insert(i as TableIndex, control_tree_id.clone());
			free_indices.push(i as TableIndex); // O reserved for control tree, 1 for connected tree
		}
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		let mut ca = CellAgent { cell_id: cell_id.clone(), my_tree_id: my_tree_id.clone(), 
			no_ports: no_ports, traphs: traphs, control_tree_id: control_tree_id.clone(), 
			connected_ports_tree_id: connected_tree_id.clone(), free_indices: Arc::new(Mutex::new(free_indices)),
			tenant_masks: tenant_masks, trees: Arc::new(Mutex::new(trees)), 
			packet_ca_to_pe: packet_ca_to_pe.clone(), entry_ca_to_pe: entry_ca_to_pe,
			packet_port_to_pe: packet_port_to_pe, packet_engine: packet_engine,
			recvr_ca_to_ports: recvr_ca_to_ports, tenant_ca_to_pe: tenant_ca_to_pe};
		// Set up predefined trees - Must be first two in this order
		let port_number_0 = try!(PortNumber::new(0, no_ports));
		let control_entry = try!(ca.update_traph(control_tree_id, port_number_0, 
				traph::PortStatus::Parent, vec![port_number_0], 0, 0, None));
		let connected_entry = try!(ca.update_traph(connected_tree_id.clone(), port_number_0, 
				traph::PortStatus::Parent, vec![port_number_0], 0, 0, None));
		// Create my tree
		let my_entry = try!(ca.update_traph(my_tree_id.clone(), port_number_0, 
				traph::PortStatus::Parent, Vec::new(), 0, 0, None)); 
		try!(ca.port_status(scope, connected_tree_id, connected_entry, my_entry, recv_status_from_port, 
				packet_ports_from_pe));
		try!(ca.recv_packets(scope, packet_ca_from_pe));
		Ok(ca)
	}
	pub fn get_no_ports(&self) -> PortNo { self.no_ports }	
	pub fn get_id(&self) -> CellID { self.cell_id.clone() }
	pub fn get_control_tree_id(&self) -> &TreeID { &self.control_tree_id }
	pub fn get_connected_ports_tree_id(&self) -> TreeID { self.connected_ports_tree_id.clone() }
	pub fn exists(&self, tree_id: &TreeID) -> bool { 
		(*self.traphs.lock().unwrap()).contains_key(tree_id)
	}
	pub fn update_traph(&mut self, tree_id: TreeID, port_number: PortNumber, port_status: traph::PortStatus,
				children: Vec<PortNumber>, other_index: TableIndex, hops: usize, path: Option<PortNumber>) 
			-> Result<RoutingTableEntry, CellAgentError> {
		let mask = try!(mask_from_port_nos(children));
		let mut traphs = self.traphs.lock().unwrap();
		let traph = traphs.remove(&tree_id);  // Avoids lifetime problem
		let (index, mut other_indices) = match traph {
			Some(t) => (t.get_table_index(), t.get_other_indices()),         // Tree exists 		
			None => (try!(self.clone().use_index()), DEFAULT_OTHER_INDICES)  // Need to create tree
		};
		let port_no = port_number.get_port_no();
		other_indices[port_no as usize] = other_index;
		let entry = RoutingTableEntry::new(index, true, port_number, mask, other_indices);
		let mut traph = try!(Traph::new(tree_id.clone(), entry));
		traph.add_element(port_number, index, other_index, port_status); 
		println!("CellAgent: {} Tree {} {} {}", self.cell_id, tree_id, entry, traph);
		traphs.insert(tree_id.clone(), traph);
		self.trees.lock().unwrap().insert(index, tree_id);
		try!(self.entry_ca_to_pe.send(entry));
		Ok(entry)
	}
	fn port_status(&mut self, scope: &Scope, connected_tree_id: TreeID, connected_tree_entry: RoutingTableEntry,
			my_entry: RoutingTableEntry, status_ca_from_port: StatusCaFromPort, 
			mut packet_ports_from_pe: HashMap<PortNo,PacketRecv>) -> Result<(), CellAgentError>{
		let tree_id = self.my_tree_id.clone();
		let my_table_index = my_entry.get_index();
		let entry_ca_to_pe = self.entry_ca_to_pe.clone();
		let mut connected_entry = connected_tree_entry.clone();	
		let tenant_mask = self.tenant_masks[0];//.last().expect("CellAgent: initiate_discover: No tenant mask");
		try!(self.tenant_ca_to_pe.send(tenant_mask));
		let ca = self.clone();
		scope.spawn( move || -> Result<(), CellAgentError> {
			loop {
				let (port_no, status) = try!(status_ca_from_port.recv());
				let port_number = try!(PortNumber::new(port_no, ca.get_no_ports())); // Validates port_no
				let port_no_mask = try!(int_to_mask(port_no));
				match status {
					port::PortStatus::Connected => {
						if let Some(packet_port_from_pe) = packet_ports_from_pe.remove(&port_no) {
							if let Some(recvr) = ca.recvr_ca_to_ports.get(port_no as usize) {
								try!(recvr.send(packet_port_from_pe));	
							} else {
								return Err(CellAgentError::Recvr(RecvrError::new(port_no)));
							}				
						} else {
							return Err(CellAgentError::PortTaken(PortTakenError::new(port_no)))
						};
						connected_entry.or_with_mask(port_no_mask); 
						try!(ca.entry_ca_to_pe.send(connected_entry));
						let msg = DiscoverMsg::new(connected_tree_id.clone(), tree_id.clone(), 
									ca.cell_id.clone(), my_table_index, 0, port_number);
						try!(ca.send_msg(msg, DEFAULT_USER_MASK));
					},
					port::PortStatus::Disconnected => {
						connected_entry.and_with_mask(!port_no_mask);
						try!(entry_ca_to_pe.send(connected_entry));
					}
				}
 			}
		});
		Ok(())
	}				
	fn send_msg<T>(&self, msg: T, user_mask: Mask) -> Result<(), CellAgentError> 
			where T: Message + Hash + serde::Serialize + fmt::Display {
		let tree_id = msg.get_header().get_tree_id();
		let index;
		if let Some(traph) = self.traphs.lock().unwrap().get(&tree_id) {
			index = traph.get_table_index();			
			let packets = try!(Packetizer::packetize(&msg, [false;4]));
			for packet in packets.iter() {
				try!(self.packet_ca_to_pe.send((index, user_mask, **packet)));
			}
			//println!("Cell Agent {} sent packets", self.cell_id);
		} else {
			return Err(CellAgentError::Tree(TreeError::new(&tree_id)));
		};
		Ok(())
	}
	fn recv_packets(&self, scope: &Scope, packet_ca_from_pe: PacketCaFromPe) -> Result<(), CellAgentError> {
		let mut packet_assembler: HashMap<u64, Vec<Box<Packet>>> = HashMap::new();
		let ca = self.clone();
		scope.spawn( move || -> Result<(), CellAgentError> {
			loop {
				let (port_no, index, packet) = try!(packet_ca_from_pe.recv());
				//println!("Cell Agent {} got packet", ca.cell_id);
				let header = packet.get_header();
				let uniquifier = header.get_uniquifier();
				let packets = packet_assembler.entry(uniquifier).or_insert(Vec::new());
				packets.push(Box::new(packet));
				if header.is_last_packet() {
					let msg = try!(Packetizer::unpacketize(packets));
					if let Some(tree_id) = ca.trees.lock().unwrap().get(&index) {
						let new_tree_id = msg.process(&mut ca.clone(), port_no, index);
					} else {
						return Err(CellAgentError::TreeIndex(TreeIndexError::new(index)));
					}
				}
			}	
		});
		Ok(())
	}
	fn use_index(&mut self) -> Result<TableIndex,CellAgentError> {
		match self.free_indices.lock().unwrap().pop() {
			Some(i) => Ok(i),
			None => Err(CellAgentError::Size(SizeError::new()))
		}
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
use name::NameError;
use utility::UtilityError;
#[derive(Debug)]
pub enum CellAgentError {
	Name(NameError),
	Size(SizeError),
	Tree(TreeError),
	TreeIndex(TreeIndexError),
	Traph(TraphError),
	Packetizer(PacketizerError),
	PortNumber(PortNumberError),
	PortTaken(PortTakenError),
	InvalidMsgType(InvalidMsgTypeError),
	MsgAssembly(MsgAssemblyError),
	BadPacket(BadPacketError),
	Utility(UtilityError),
	Routing(RoutingTableError),
	SendTableEntry(SendError<RoutingTableEntry>),
	SendCaPe(SendError<(u32,u16,Packet)>),
	SendPacket(PacketSendError),
	SendTenant(TenantMaskSendError),
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
			CellAgentError::BadPacket(ref err) => err.description(),
			CellAgentError::InvalidMsgType(ref err) => err.description(),
			CellAgentError::MsgAssembly(ref err) => err.description(),
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::Size(ref err) => err.description(),
			CellAgentError::Tree(ref err) => err.description(),
			CellAgentError::TreeIndex(ref err) => err.description(),
			CellAgentError::Traph(ref err) => err.description(),
			CellAgentError::Utility(ref err) => err.description(),
			CellAgentError::Routing(ref err) => err.description(),
			CellAgentError::SendTableEntry(ref err) => err.description(),
			CellAgentError::SendCaPe(ref err) => err.description(),
			CellAgentError::SendPacket(ref err) => err.description(),
			CellAgentError::SendTenant(ref err) => err.description(),
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
			CellAgentError::BadPacket(ref err) => Some(err),
			CellAgentError::InvalidMsgType(ref err) => Some(err),
			CellAgentError::MsgAssembly(ref err) => Some(err),
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::Size(ref err) => Some(err),
			CellAgentError::Tree(ref err) => Some(err),
			CellAgentError::TreeIndex(ref err) => Some(err),
			CellAgentError::Traph(ref err) => Some(err),
			CellAgentError::Utility(ref err) => Some(err),
			CellAgentError::Routing(ref err) => Some(err),
			CellAgentError::SendTableEntry(ref err) => Some(err),
			CellAgentError::SendCaPe(ref err) => Some(err),
			CellAgentError::SendPacket(ref err) => Some(err),
			CellAgentError::SendTenant(ref err) => Some(err),
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
			CellAgentError::BadPacket(ref err) => write!(f, "Cell Agent Bad Packet Error caused by {}", err),
			CellAgentError::InvalidMsgType(ref err) => write!(f, "Cell Agent Invalid Message Type Error caused by {}", err),
			CellAgentError::MsgAssembly(ref err) => write!(f, "Cell Agent Message Assembly Error caused by {}", err),
			CellAgentError::Name(ref err) => write!(f, "Cell Agent Name Error caused by {}", err),
			CellAgentError::Size(ref err) => write!(f, "Cell Agent Size Error caused by {}", err),
			CellAgentError::Tree(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::TreeIndex(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::Traph(ref err) => write!(f, "Cell Agent Traph Error caused by {}", err),
			CellAgentError::Utility(ref err) => write!(f, "Cell Agent Utility Error caused by {}", err),
			CellAgentError::Routing(ref err) => write!(f, "Cell Agent Routing Table Error caused by {}", err),
			CellAgentError::SendTableEntry(ref err) => write!(f, "Cell Agent Send Table Entry Error caused by {}", err),
			CellAgentError::SendCaPe(ref err) => write!(f, "Cell Agent Send Packet to Packet Engine Error caused by {}", err),
			CellAgentError::SendPacket(ref err) => write!(f, "Cell Agent Send Packet to Packet Engine Error caused by {}", err),
			CellAgentError::SendTenant(ref err) => write!(f, "Cell Agent Send Tenant Mask to Packet Engine Error caused by {}", err),
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
impl From<RoutingTableError> for CellAgentError{
	fn from(err: RoutingTableError) -> CellAgentError { CellAgentError::Routing(err) }
}
impl From<SendError<RoutingTableEntry>> for CellAgentError{
	fn from(err: SendError<RoutingTableEntry>) -> CellAgentError { CellAgentError::SendTableEntry(err) }
}
impl From<SendError<(u32,u16,Packet)>> for CellAgentError{
	fn from(err: SendError<(u32,u16,Packet)>) -> CellAgentError { CellAgentError::SendCaPe(err) }
}
impl From<SendError<Packet>> for CellAgentError{
	fn from(err: SendError<Packet>) -> CellAgentError { CellAgentError::SendPacket(err) }
}
impl From<SendError<u16>> for CellAgentError{
	fn from(err: SendError<u16>) -> CellAgentError { CellAgentError::SendTenant(err) }
}
impl From<RecvError> for CellAgentError{
	fn from(err: RecvError) -> CellAgentError { CellAgentError::Recv(err) }
}
impl From<RecvrSendError> for CellAgentError{
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
#[derive(Debug)]
pub struct InvalidMsgTypeError { msg: String }
impl InvalidMsgTypeError { 
	pub fn new() -> InvalidMsgTypeError {
		InvalidMsgTypeError { msg: format!("Problem with packet assembler") }
	}
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
	pub fn new() -> MsgAssemblyError {
		MsgAssemblyError { msg: format!("Problem with packet assembler") }
	}
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
pub struct BadPacketError { msg: String }
impl BadPacketError { 
	pub fn new() -> BadPacketError {
		BadPacketError { msg: format!("No packet created by packetizer") }
	}
}
impl Error for BadPacketError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for BadPacketError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<BadPacketError> for CellAgentError {
	fn from(err: BadPacketError) -> CellAgentError { CellAgentError::BadPacket(err) }
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
