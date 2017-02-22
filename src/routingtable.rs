use std::fmt;
use std::fmt::Display;
use std::collections::HashMap;
use std::cell::{Cell};
use config::{MAX_ENTRIES, MAX_PORTS};
use name::{Name,CellID,PortID,TreeID};
use port::Port;
use traph::Traph;
use utility::chars_to_string;

const CONTROLPORT: &'static str = "Control";
const LOGICALPORTS: &'static str = "Logical";
const CONNECTEDPORTS: &'static str = "Connected";
const PARENT_TREE_ID_SUFFIX: &'static str = "-P";
const DEFAULT_TREE_ID: &'static str = "Default";

#[derive(Debug, Clone)]
pub struct RoutingTable {
	control_tree_id: TreeID,
	logical_ports_tree_id: TreeID,
	connected_ports_tree_id: TreeID,
	free_indices: Vec<usize>,
	entries: Vec<Cell<RoutingTableEntry>>,
	tree_ids: HashMap<TreeID,Traph>,
	connected_ports: Vec<PortID>
}
impl RoutingTable {
	pub fn new() -> Result<RoutingTable,RoutingTableError> {
		let mut free_indices = Vec::new();
		for i in 0..MAX_ENTRIES { free_indices.push(i); }
		free_indices.reverse();
		let default_tree_id = try!(TreeID::new(DEFAULT_TREE_ID));
		let default_entry = RoutingTableEntry::new(default_tree_id, 0, false);
		let mut entries = Vec::new();
		for i in 1..MAX_ENTRIES {
			let mut entry = default_entry; // NB: RoutingTableEntry is Copy
			entry.index = i;
			entries.push(Cell::new(entry));
		}
		let control_tree_id = try!(TreeID::new(CONTROLPORT));
		let logical_ports_tree_id = try!(TreeID::new(LOGICALPORTS));
		let connected_ports_tree_id = try!(TreeID::new(CONNECTEDPORTS));
		let mut routing_table = RoutingTable { control_tree_id: control_tree_id,
			logical_ports_tree_id: logical_ports_tree_id, 
			connected_ports_tree_id: connected_ports_tree_id, free_indices: free_indices,
			entries: entries, tree_ids: HashMap::new(), connected_ports: Vec::new() };
		Ok((routing_table))
	}
	fn use_index(&mut self) -> Result<usize,RoutingTableError> {
		match self.free_indices.pop() {
			Some(i) => Ok(i),
			None => Err(RoutingTableError::Size(SizeError::new()))
		}
	}
	pub fn add_entry(&mut self, tree_id: TreeID, port: &Port, hops: usize, path: Option<TreeID>) -> Result<(),RoutingTableError>{
		let index = try!(self.use_index());
		//let traph = Traph::new(tree_id, index, port, hops, path);
		self.entries[index] = Cell::new(RoutingTableEntry::new(tree_id, index, true));
		Ok(())
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum RoutingTableError {
	Name(NameError),
	Size(SizeError)
}
impl Error for RoutingTableError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableError::Name(ref err) => err.description(),
			RoutingTableError::Size(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableError::Name(ref err) => Some(err),
			RoutingTableError::Size(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableError::Name(_) => write!(f, "Routing Table Name Error caused by"),
			RoutingTableError::Size(_) => write!(f, "Routing Table Size Error caused by"),
		}
	}
}
impl From<NameError> for RoutingTableError {
	fn from(err: NameError) -> RoutingTableError { RoutingTableError::Name(err) }
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
impl From<SizeError> for RoutingTableError {
	fn from(err: SizeError) -> RoutingTableError { RoutingTableError::Size(err) }
}

const TREE_HASH_SIZE: usize = 64;
#[derive(Copy)]
struct RoutingTableEntry {
	index: usize,
	inuse: bool,
	mask: u16,
	tree_hash: [char; TREE_HASH_SIZE],
	indices: [usize; MAX_ENTRIES]
}
impl RoutingTableEntry {
	fn new(tree_id: TreeID, table_index: usize, inuse: bool) -> RoutingTableEntry {
		let tree_id_hash = RoutingTableEntry::hash(&tree_id);
		RoutingTableEntry { tree_hash: tree_id_hash, index: table_index,
			inuse: false, mask: 0, indices: [0; MAX_ENTRIES] }
	}
	fn hash<T: Display>(x: &T) -> [char; TREE_HASH_SIZE] { 
		let s = format!("H{}", x.to_string());
		let mut chars = [' '; TREE_HASH_SIZE];
		let mut i = 0;
		for c in s.chars() {
			chars[i] = c;
			i = i + 1;
		}
		chars
	}
	fn to_string(&self) -> String {
		let mut s = format!("{:4}", self.index);
		if self.inuse { s = s + &format!(" Yes  ") }
		else          { s = s + &format!(" No   ") }
		s = s + &format!("{:15}", chars_to_string(&self.tree_hash));
		s = s + &format!(" {:016.b}", self.mask);
		s = s + &format!(" {:?}", self.indices.to_vec());
		s
	}
}
impl Clone for RoutingTableEntry {
	fn clone(&self) -> RoutingTableEntry {
		RoutingTableEntry { index: self.index, inuse: self.inuse, mask: self.mask,
			tree_hash: self.tree_hash, indices: self.indices }
	}
}
impl fmt::Display for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
impl fmt::Debug for RoutingTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
