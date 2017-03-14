use std::fmt;
use cellagent::CellAgent;
use name::{CellID, TreeID};

pub trait Message {
	fn get_header(&self) -> MsgHeader;
	//fn get_payload(&self) -> MsgPayload;
	fn process(&self, port_no: u8, cell_agent: &CellAgent);
}
pub trait MsgPayload {}
#[derive(Debug, Copy, Clone)]
pub enum MsgDirection {
	Rootward,
	Leafward
}
#[derive(Debug, Clone)]
pub struct MsgHeader {
	tree_id: TreeID,
	direction: MsgDirection,
	sending_port: u8
}
impl MsgHeader {
	pub fn new(tree_id: TreeID, direction: MsgDirection) -> MsgHeader {
		MsgHeader { tree_id: tree_id, direction: direction, sending_port: 0 }
	}
	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
	pub fn set_tree_id(&mut self, tree_id: TreeID) { self.tree_id = tree_id; }
	pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
#[derive(Debug, Clone)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(connected_ports_id: TreeID, tree_id: TreeID, sending_node_id: CellID, hops: usize, path: u8) -> DiscoverMsg {
		let header = MsgHeader::new(connected_ports_id, MsgDirection::Leafward);
		let payload = DiscoverPayload::new(tree_id, sending_node_id, hops, path);
		DiscoverMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> MsgHeader { self.header.clone() }
	//fn get_payload(&self) -> MsgPayload { self.payload }
	fn process(&self, port_no: u8, cell_agent: &CellAgent) { println!("message::process	not implemented"); }
}
#[derive(Debug, Clone)]
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
impl MsgPayload for DiscoverPayload {}