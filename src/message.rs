use std::fmt;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use cellagent::{CellAgent};
use config::{PathLength, PortNo, TableIndex};
use name::{Name, CellID, TreeID};
use packet::Packetizer;
use traph;
use utility::{DEFAULT_USER_MASK, Mask, Path, PortNumber};

static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> usize { MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) } 
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

pub trait Message: fmt::Display {
	fn get_header(&self) -> MsgHeader;
	fn get_payload(&self) -> Box<MsgPayload>;
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn process(&mut self, cell_agent: &mut CellAgent, port_no: PortNo) -> Result<(), ProcessMsgError>;
}
pub trait MsgPayload {}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
// Header may not contain '{' or '}' or a separate object, such as TreeID
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
	//pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for MsgHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Message {} {} '{}'", self.msg_count, self.msg_type, self.direction);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(tree_id: TreeID, my_index: TableIndex, sending_cell_id: CellID, 
			hops: PathLength, path: Path) -> DiscoverMsg {
		let header = MsgHeader::new(MsgType::Discover, MsgDirection::Leafward);
		//println!("DiscoverMsg: msg_count {}", header.get_count());
		let payload = DiscoverPayload::new(tree_id, my_index, sending_cell_id, hops, path);
		DiscoverMsg { header: header, payload: payload }
	}
	pub fn update_discover_msg(&mut self, cell_id: CellID) {
		let hops = self.update_hops();
		let path = self.update_path();
		self.payload.set_hops(hops);
		self.payload.set_path(path);
		self.payload.set_sending_cell(cell_id);
	}
	fn update_hops(&self) -> PathLength { self.payload.get_hops() + 1 }
	fn update_path(&self) -> Path { self.payload.get_path() } // No change per hop
}
#[deny(unused_must_use)]
impl Message for DiscoverMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&mut self, ca: &mut CellAgent, port_no: u8) -> Result<(), ProcessMsgError> {
		let new_tree_id = self.payload.get_tree_id();
		let port_number = PortNumber::new(port_no, ca.get_no_ports())?;
		let hops = self.payload.get_hops();
		let path = self.payload.get_path();
		let senders_index = self.payload.get_index();
		let children = &Vec::new();
		//println!("DiscoverMsg: tree_id {}, port_number {}", tree_id, port_number);
		let exists = ca.exists(&new_tree_id);  // Have I seen this tree before?
		let status = if exists { traph::PortStatus::Pruned } else { traph::PortStatus::Parent };
		let entry = ca.update_traph(&new_tree_id, port_number, status,
				children, senders_index, hops, Some(path))?;
		//println!("DiscoverMsg {}: entry {}", ca.get_id(), entry);
		if exists { 
			println!("DiscoverMsg {}: exists {}", ca.get_id(), self);
			return Ok(()); // Don't forward if traph exists for this tree - Simple quenching
		} 
		let my_index = entry.get_index();
		// Send DiscoverD to sender
		let discoverd_msg = DiscoverDMsg::new(new_tree_id.clone(), my_index);
		let packets = Packetizer::packetize(&discoverd_msg, senders_index)?;
		println!("DiscoverMsg {}: sending discoverd for tree {} packet {} {}",ca.get_id(), new_tree_id, packets[0].get_count(), discoverd_msg);
		let mask = Mask::new(port_number);
		ca.send_msg(&ca.get_connected_ports_tree_id(), packets, mask)?;
		// Forward Discover on all except port_no with updated hops and path
		self.update_discover_msg(ca.get_id());
		let control_tree_index = 0;
		let packets = Packetizer::packetize(self, control_tree_index)?;
		let user_mask = DEFAULT_USER_MASK.all_but_port(PortNumber::new(port_no, ca.get_no_ports())?);
		ca.add_discover_msg(self.clone());
		println!("DiscoverMsg {}: forwarding packet {} on connected ports {}", ca.get_id(), packets[0].get_count(), self);
		ca.send_msg(&ca.get_connected_ports_tree_id(), packets, user_mask)?;
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
pub struct DiscoverPayload {
	tree_id: TreeID,
	index: TableIndex,
	sending_cell_id: CellID,
	hops: PathLength,
	path: Path,
}
impl DiscoverPayload {
	fn new(tree_id: TreeID, index: TableIndex, sending_cell_id: CellID, 
			hops: PathLength, path: Path) -> DiscoverPayload {
		DiscoverPayload { tree_id: tree_id, index: index, sending_cell_id: sending_cell_id, 
			hops: hops, path: path }
	}
	fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	//fn get_sending_cell(&self) -> CellID { self.sending_cell_id.clone() }
	fn get_hops(&self) -> PathLength { self.hops }
	fn get_path(&self) -> Path { self.path }
	fn get_index(&self) -> TableIndex { self.index }
	fn set_hops(&mut self, hops: PathLength) { self.hops = hops; }
	fn set_path(&mut self, path: Path) { self.path = path; }
	fn set_sending_cell(&mut self, sending_cell_id: CellID) { self.sending_cell_id = sending_cell_id; }
}
impl MsgPayload for DiscoverPayload {}
impl fmt::Display for DiscoverPayload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Tree {}, sending cell {}, index {}, hops {}, path {}", self.tree_id, 
			self.sending_cell_id, self.index, self.hops, self.path);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDMsg {
	header: MsgHeader,
	payload: DiscoverDPayload
}
impl DiscoverDMsg {
	pub fn new(tree_id: TreeID, index: TableIndex) -> DiscoverDMsg {
		// Note that direction is leafward so we can use the connected ports tree
		// If we send rootward, then the first recipient forwards the DiscoverD
		let header = MsgHeader::new(MsgType::DiscoverD, MsgDirection::Leafward);
		let payload = DiscoverDPayload::new(tree_id, index);
		DiscoverDMsg { header: header, payload: payload }
	}
}
#[deny(unused_must_use)]
impl Message for DiscoverDMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&mut self, ca: &mut CellAgent, port_no: u8) 
			-> Result<(), ProcessMsgError> {
		let tree_id = self.payload.get_tree_id().stringify();
		let my_index = self.payload.get_table_index();
		println!("DiscoverDMsg {}: process msg {} processing {} {} {}", ca.get_id(), self.get_header().get_count(), port_no, my_index, tree_id);
		ca.add_child(&tree_id, port_no, my_index)?;
		Ok(())
	}
}
impl fmt::Display for DiscoverDMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {
	tree_id: TreeID,
	my_index: TableIndex,
}
#[deny(unused_must_use)]
impl DiscoverDPayload {
	fn new(tree_id: TreeID, index: TableIndex) -> DiscoverDPayload {
		DiscoverDPayload { tree_id: tree_id, my_index: index }
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
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
use utility::{PortNumberError, UtilityError};
#[derive(Debug)]
pub enum MessageError {
	Packetizer(PacketizerError),
	PortNumber(PortNumberError),
	Utility(UtilityError),
}
impl Error for MessageError {
	fn description(&self) -> &str {
		match *self {
			MessageError::Packetizer(ref err) => err.description(),
			MessageError::PortNumber(ref err) => err.description(),
			MessageError::Utility(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			MessageError::Packetizer(ref err) => Some(err),
			MessageError::PortNumber(ref err) => Some(err),
			MessageError::Utility(ref err) => Some(err),
		}
	}
}
impl fmt::Display for MessageError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MessageError::Packetizer(ref err) => write!(f, "Packetizer Error caused by {}", err),
			MessageError::PortNumber(ref err) => write!(f, "Port Number Error caused by {}", err),
			MessageError::Utility(ref err) => write!(f, "Utility Error caused by {}", err),
		}
	}
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
