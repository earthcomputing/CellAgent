use std::fmt;
use std::collections::HashMap;
use name::{TreeID,PortID};

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	//reply_tree_id: TreeID, // Until I figure out what this is
	table_index: usize,
	elements: HashMap<PortID, TraphElement>,
}
impl Traph {
	fn new(tree_id: TreeID, table_index: usize) -> Traph {
		Traph { tree_id: tree_id, table_index: table_index, elements: HashMap::new() }
	}
}
#[derive(Debug, Copy, Clone)]
enum TraphStatus {
	Parent,
	Child,
	Pruned
}

#[derive(Debug, Clone)]
struct TraphElement {
	port_id: PortID,
	is_connected: bool,
	is_broken: bool,
	status: TraphStatus,
	hops: usize,
	path: Option<TreeID> // or Option<PortID>
}