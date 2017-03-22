use std::fmt;
use cellagent::CellAgent;
use name::{CellID, TreeID};

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
	fn process(&self, port_no: u8, cell_agent: &CellAgent);
	fn stringify(&self) -> String;
}
pub trait MsgPayload {
	fn get_tree_id(&self) -> TreeID;
	fn stringify(&self) -> String;
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
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct MsgHeader {
	tree_id: TreeID,
	direction: MsgDirection,
	sending_port: u8,
}
impl MsgHeader {
	pub fn new(tree_id: TreeID, direction: MsgDirection) -> MsgHeader {
		MsgHeader { tree_id: tree_id, direction: direction, sending_port: 0 }
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
	pub fn get_sending_port(&self) -> u8 { self.sending_port }
	pub fn set_tree_id(&mut self, tree_id: TreeID) { self.tree_id = tree_id; }
	pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
	pub fn set_sending_port(&mut self, sending_port: u8) { self.sending_port = sending_port; }
	pub fn stringify(&self) -> String {
		format!("Message {} on Tree '{}'", self.direction, self.tree_id)
	}
}
impl fmt::Display for MsgHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(connected_ports_id: TreeID, tree_id: TreeID, sending_node_id: CellID, hops: usize, path: u8) -> DiscoverMsg {
		let payload = DiscoverPayload::new(tree_id, sending_node_id, hops, path);
		let header = MsgHeader::new(connected_ports_id, MsgDirection::Leafward);
		DiscoverMsg { header: header, payload: payload }
	}
	pub fn get_header(&self) -> MsgHeader { self.header.clone() }
	pub fn get_payload(&self) -> DiscoverPayload { self.payload.clone() }
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	fn get_payload(&self) -> Box<MsgPayload> { Box::new(self.payload.clone()) }
	fn process(&self, port_no: u8, cell_agent: &CellAgent) { println!("message::process	not implemented"); }
	fn stringify(&self) -> String {
		format!("{}: {}", self.header, self.payload)
	}
}
impl fmt::Display for DiscoverMsg { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
struct DiscoverPayload {
	tree_id: TreeID,
	sending_node_id: CellID,
	hops: usize,
	path: u8,
}
impl DiscoverPayload {
	fn new(tree_id: TreeID, sending_node_id: CellID, hops: usize, path: u8) -> DiscoverPayload {
		DiscoverPayload { tree_id: tree_id, sending_node_id: sending_node_id, hops: hops, path: path }
	}
}
impl fmt::Display for DiscoverPayload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl MsgPayload for DiscoverPayload {
	fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	fn stringify(&self) -> String {
		format!("Tree {}, sending cell {}, hops {}, path {}", self.tree_id, self.sending_node_id,
				self.hops, self.path)
	}
}
