use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use cellagent::{DEFAULT_OTHER_INDICES, CellAgent};
use config::{CellNo, PathLength, TableIndex};
use name::{CellID, TreeID};
use packet::Packetizer;
use traph;
use utility::{Mask, Path, PortNumber};

static message_count: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> usize { message_count.fetch_add(1, Ordering::SeqCst) } 
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgType {
	Discover,
	DiscoverD,
}
impl fmt::Display for MsgType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgType::Discover => write!(f, "Discover"),
			MsgType::DiscoverD => write!(f, "DiscoverD"),
		}
	}
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgDirection {
	Rootward,
	Leafward
}
impl fmt::Display for MsgDirection {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgDirection::Rootward => write!(f, "Rootward"),
			MsgDirection::Leafward => write!(f, "Leafward")
		}
	}
}

pub trait Message {
	fn get_header(&self) -> MsgHeader;
	fn get_payload(&self) -> Box<MsgPayload>;
	fn get_count(&self) -> usize { self.get_header().get_count() }
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn process(&self, cell_agent: &mut CellAgent, port_no: u8, index: TableIndex) 
			-> Result<(), ProcessMsgError>;
}
pub trait MsgPayload {}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct MsgHeader {
	msg_count: usize,
	msg_type: MsgType,
	direction: MsgDirection,
}
impl MsgHeader {
	pub fn new(msg_type: MsgType, direction: MsgDirection) -> MsgHeader {
		let msg_count = get_next_count();
		MsgHeader { msg_type: msg_type, direction: direction, msg_count: msg_count }
	}
	pub fn get_msg_type(&self) -> MsgType { self.msg_type }
	pub fn get_count(&self) -> usize { self.msg_count }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
	pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for MsgHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Message {} '{}'", self.msg_type, self.direction);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(tree_id: TreeID, sending_node_id: CellID, 
			my_index: TableIndex, hops: PathLength, path: Path) -> DiscoverMsg {
		let header = MsgHeader::new(MsgType::Discover, MsgDirection::Leafward);
		let payload = DiscoverPayload::new(tree_id, sending_node_id, my_index, hops, path);
		DiscoverMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&self, ca: &mut CellAgent, port_no: u8, index: u32) -> Result<(), ProcessMsgError> {
		let tree_id = try!(ca.get_tree_id(index));
		let new_tree_id = self.payload.get_tree_id();
		let port_number = try!(PortNumber::new(port_no, ca.get_no_ports()));
		//println!("Message {}: msg {} port {} {}", ca.get_id(), self.get_count(), port_no, self.payload);
		if ca.exists(&new_tree_id) { return Ok(()); } // Ignore if traph exists for this tree - Simple quenching
		let senders_index = self.payload.get_senders_index();
		let hops = self.payload.get_hops();
		let path = self.payload.get_path();
		//println!("Message: tree_id {}, port_number {}", tree_id, port_number);
		let entry = try!(ca.update_traph(new_tree_id.clone(), port_number, traph::PortStatus::Parent,
				Vec::new(), senders_index, hops, Some(path)));
		//println!("Message {}: entry {}", ca.get_id(), entry);
		let index = entry.get_index();
		// Send DiscoverD to sender
		//let discoverd_msg = DiscoverDMsg::new(ca.get_id(), index);
		//let port_mask = try!(Mask::new(port_no));
		//let packets = try!(Packetizer::packetize(&discoverd_msg, [false;4]));
		//println!("DiscoverMsg {}: Sending discoverD tree {}",ca.get_id(), new_tree_id);
		//try!(ca.send_msg(&tree_id, packets, port_mask));
		// Forward Discover on all except port_no
		//let discover_msg = DiscoverMsg::new(tree_id.clone(), ca.get_id(), index, hops+1, path);
		Ok(())
	}
}
impl fmt::Display for DiscoverMsg { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("{}: {}", self.header, self.payload);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
struct DiscoverPayload {
	tree_id: TreeID,
	sending_node_id: CellID,
	senders_index: TableIndex,
	hops: PathLength,
	path: Path,
}
impl DiscoverPayload {
	fn new(tree_id: TreeID, sending_node_id: CellID, senders_index: TableIndex,  
			hops: PathLength, path: Path) -> DiscoverPayload {
		DiscoverPayload { tree_id: tree_id, sending_node_id: sending_node_id, 
			senders_index: senders_index, hops: hops, path: path }
	}
	fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	fn get_sending_node(&self) -> CellID { self.sending_node_id.clone() }
	fn get_senders_index(&self) -> TableIndex { self.senders_index }
	fn get_hops(&self) -> PathLength { self.hops }
	fn get_path(&self) -> Path { self.path }
}
impl MsgPayload for DiscoverPayload {}
impl fmt::Display for DiscoverPayload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Tree {}, sending cell {}, senders_index {}, hops {}, path {}", self.tree_id, self.sending_node_id,
				self.senders_index, self.hops, self.path);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDMsg {
	header: MsgHeader,
	payload: DiscoverDPayload
}
impl DiscoverDMsg {
	fn new(sending_cell_id: CellID, index: TableIndex) -> DiscoverDMsg {
		let header = MsgHeader::new(MsgType::DiscoverD, MsgDirection::Rootward);
		let payload = DiscoverDPayload::new(index);
		DiscoverDMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverDMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&self, cell_agent: &mut CellAgent, port_no: u8, index: u32) 
			-> Result<(), ProcessMsgError> {
		println!("DiscoverDMsg: processing {} {} {}", cell_agent, port_no, index);
		Ok(())
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {
	my_index: TableIndex,
}
impl DiscoverDPayload {
	fn new(index: TableIndex) -> DiscoverDPayload {
		DiscoverDPayload { my_index: index }
	}
	pub fn get_table_index(&self) -> TableIndex { self.my_index }
}
impl MsgPayload for DiscoverDPayload {}
impl fmt::Display for DiscoverDPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "My table index {}", self.my_index)
	}
}
// Errors
use std::error::Error;
use cellagent::CellAgentError;
use packet::PacketizerError;
use utility::{PortError, PortNumberError, UtilityError};
#[derive(Debug)]
pub enum MessageError {
	CellAgent(CellAgentError),
	Packetizer(PacketizerError),
	PortNumber(PortNumberError),
	Utility(UtilityError),
}
impl Error for MessageError {
	fn description(&self) -> &str {
		match *self {
			MessageError::CellAgent(ref err) => err.description(),
			MessageError::Packetizer(ref err) => err.description(),
			MessageError::PortNumber(ref err) => err.description(),
			MessageError::Utility(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			MessageError::CellAgent(ref err) => Some(err),
			MessageError::Packetizer(ref err) => Some(err),
			MessageError::PortNumber(ref err) => Some(err),
			MessageError::Utility(ref err) => Some(err),
		}
	}
}
impl fmt::Display for MessageError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MessageError::CellAgent(ref err) => write!(f, "Cell Agent Error caused by {}", err),
			MessageError::Packetizer(ref err) => write!(f, "Packetizer Error caused by {}", err),
			MessageError::PortNumber(ref err) => write!(f, "Port Number Error caused by {}", err),
			MessageError::Utility(ref err) => write!(f, "Utility Error caused by {}", err),
		}
	}
}
impl From<CellAgentError> for MessageError {
	fn from(err: CellAgentError) -> MessageError { MessageError::CellAgent(err) }
}
impl From<PortNumberError> for MessageError {
	fn from(err: PortNumberError) -> MessageError { MessageError::PortNumber(err) }
}
impl From<PacketizerError> for MessageError {
	fn from(err: PacketizerError) -> MessageError { MessageError::Packetizer(err) }
}
impl From<UtilityError> for MessageError {
	fn from(err: UtilityError) -> MessageError { MessageError::Utility(err) }
}
#[derive(Debug)]
pub struct ProcessMsgError { msg: String }
impl ProcessMsgError { 
	pub fn new(err: &Error) -> ProcessMsgError {
		ProcessMsgError { msg: format!("Error {}", err) }
	}
}
impl Error for ProcessMsgError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for ProcessMsgError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PortNumberError> for ProcessMsgError {
	fn from(err: PortNumberError) -> ProcessMsgError {
		ProcessMsgError::new(&err)
	}
}
impl From<CellAgentError> for ProcessMsgError {
	fn from(err: CellAgentError) -> ProcessMsgError {
		ProcessMsgError::new(&err)
	}
}
impl From<UtilityError> for ProcessMsgError {
	fn from(err: UtilityError) -> ProcessMsgError {
		ProcessMsgError::new(&err)
	}
}
impl From<PacketizerError> for ProcessMsgError {
	fn from(err: PacketizerError) -> ProcessMsgError {
		ProcessMsgError::new(&err)
	}
}
