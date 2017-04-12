use std::fmt;
use name::{CellID, TreeID};
use nalcell::PortNumber;

#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgType {
	Discover,
	DiscoverD,
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
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn process(&self, cell_id: &CellID, port_no: u8) -> TreeID;
}
pub trait MsgPayload {
	fn get_tree_id(&self) -> TreeID;
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct MsgHeader {
	tree_id: TreeID,
	msg_type: MsgType,
	direction: MsgDirection,
}
impl MsgHeader {
	pub fn new(tree_id: TreeID, msg_type: MsgType, direction: MsgDirection) -> MsgHeader {
		MsgHeader { tree_id: tree_id, msg_type: msg_type, direction: direction }
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
	pub fn set_tree_id(&mut self, tree_id: TreeID) { self.tree_id = tree_id; }
	pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for MsgHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Message {} on Tree '{}'", self.direction, self.tree_id);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(connected_ports_id: TreeID, tree_id: TreeID, 
			sending_node_id: CellID, hops: usize, path: PortNumber) -> DiscoverMsg {
		let payload = DiscoverPayload::new(tree_id, sending_node_id, hops, path);
		let header = MsgHeader::new(connected_ports_id, MsgType::Discover, MsgDirection::Leafward);
		DiscoverMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&self, cell_id: &CellID, port_no: u8) -> TreeID { 
		println!("CellID {} port {} {}", cell_id, port_no, self.payload); 
		// Send DiscoverD to sender
		// Forward Discover on all except port_no
		// Return TreeID 
		self.get_payload().get_tree_id()
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
	hops: usize,
	path: PortNumber,
}
impl DiscoverPayload {
	fn new(tree_id: TreeID, sending_node_id: CellID, hops: usize, path: PortNumber) -> DiscoverPayload {
		DiscoverPayload { tree_id: tree_id, sending_node_id: sending_node_id, hops: hops, path: path }
	}
}
impl MsgPayload for DiscoverPayload {
	fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
}
impl fmt::Display for DiscoverPayload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Tree {}, sending cell {}, hops {}, path {}", self.tree_id, self.sending_node_id,
				self.hops, self.path);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDMsg {
	header: MsgHeader,
	payload: DiscoverDPayload
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {}