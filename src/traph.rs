use std::fmt;
use std::collections::HashMap;
use name::{TreeID,PortID};

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	parent_port_no: u8, // Until I figure out what this is
	table_index: usize,
	elements: HashMap<PortID, TraphElement>,
}
impl Traph {
	pub fn new(tree_id: TreeID, table_index: usize, parent: u8,
			hops: usize, path: Option<&TreeID>) -> Traph {
		Traph { tree_id: tree_id, parent_port_no: 0, table_index: table_index, elements: HashMap::new() }
	}
	pub fn get_table_index(&self) -> usize { self.table_index } 
}
#[derive(Debug, Copy, Clone)]
enum TraphStatus {
	Parent,
	Child,
	Pruned
}

#[derive(Debug, Clone)]
struct TraphElement {
	port_index: u8,
	is_connected: bool,
	is_broken: bool,
	status: TraphStatus,
	hops: usize,
	path: Option<TreeID> // or Option<PortID>
}