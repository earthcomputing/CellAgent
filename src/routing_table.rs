use std::fmt;
use std::fmt::Display;
use std::collections::HashMap;
use std::cell::{Cell};
use config::{MAX_ENTRIES, MAX_PORTS};
use name::{PortID,TreeID};
use routing_table_entry::{RoutingTableEntry, RoutingTableEntryError};
use traph::Traph;

const CONTROLPORT: &'static str = "Control";
const CONNECTEDPORTS: &'static str = "Connected";
const DEFAULT_TREE_ID: &'static str = "Default";

#[derive(Debug, Clone)]
pub struct RoutingTable {
	control_tree_id: TreeID,
	connected_ports_tree_id: TreeID,
	free_indices: Vec<usize>,
	entries: Vec<Cell<RoutingTableEntry>>,
	tree_ids: HashMap<TreeID,Traph>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new() -> Result<RoutingTable,RoutingTableError> {
		let mut free_indices = Vec::new();
		for i in 0..MAX_ENTRIES { free_indices.push(i); }
		free_indices.reverse();
		let default_entry = RoutingTableEntry::new(0, false);
		let mut entries = Vec::new();
		for i in 1..MAX_ENTRIES {
			let mut entry = default_entry.clone(); 
			entry.set_index(i);
			entries.push(Cell::new(entry.clone()));
		}
		let control_tree_id = try!(TreeID::new(CONTROLPORT)).clone();
		let connected_ports_tree_id = try!(TreeID::new(CONNECTEDPORTS));
		let mut routing_table = RoutingTable { control_tree_id: control_tree_id.clone(),
			connected_ports_tree_id: connected_ports_tree_id.clone(), free_indices: free_indices,
			entries: entries, tree_ids: HashMap::new(), connected_ports: Vec::new() };
		try!(routing_table.add_entry(&control_tree_id, 0, 0, None)); 
		try!(routing_table.add_entry(&connected_ports_tree_id, 0, 0, None));
		Ok((routing_table))
	}
	fn use_index(&mut self) -> Result<usize,RoutingTableError> {
		match self.free_indices.pop() {
			Some(i) => Ok(i),
			None => Err(RoutingTableError::Size(SizeError::new()))
		}
	}
	pub fn add_entry(&mut self, tree_id: &TreeID, port_index: u8, hops: usize, path: Option<&TreeID>) -> Result<(),RoutingTableError>{
		let index = try!(self.use_index());
		let traph = Traph::new(tree_id.clone(), index, port_index, hops, path);
		self.entries[index] = Cell::new(RoutingTableEntry::new(index, true));
		self.tree_ids.insert(tree_id.clone(), traph);
		Ok(())
	}
	pub fn add_parent(&mut self, tree_id: &TreeID, parent_port_no: u8, other_index: usize) -> Result<(),RoutingTableError> {
		let mut traph = match self.tree_ids.get_mut(tree_id) {
			Some(t) => t,
			None => return Err(RoutingTableError::Traph(TraphError::new(tree_id)))
		};
		let index = traph.get_table_index();
		let mut entry = match self.entries.get_mut(index) {
			Some(e) => e.get(),
			None => return Err(RoutingTableError::Traph(TraphError::new(tree_id)))
		};
		self.entries[index] = Cell::new(try!(entry.update_parent(parent_port_no, other_index)));
		Ok(())
	}
	pub fn add_child(&mut self, tree_id: &TreeID, child_port_no: u8, other_index: usize) -> Result<(),RoutingTableError> {
		let mut traph = match self.tree_ids.get_mut(tree_id) {
			Some(t) => t,
			None => return Err(RoutingTableError::Traph(TraphError::new(tree_id)))
		};
		let index = traph.get_table_index();
		let entry = match self.entries.get_mut(index) {
			Some(e) => e.get(),
			None => return Err(RoutingTableError::Traph(TraphError::new(tree_id)))
		};
		self.entries[index] = Cell::new(try!(entry.update_children(child_port_no, other_index)));
		Ok(())
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum RoutingTableError {
	Name(NameError),
	Size(SizeError),
	Traph(TraphError),
	RoutingTableEntry(RoutingTableEntryError)
}
impl Error for RoutingTableError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableError::Name(ref err) => err.description(),
			RoutingTableError::Size(ref err) => err.description(),
			RoutingTableError::Traph(ref err) => err.description(),
			RoutingTableError::RoutingTableEntry(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableError::Name(ref err) => Some(err),
			RoutingTableError::Size(ref err) => Some(err),
			RoutingTableError::Traph(ref err) => Some(err),
			RoutingTableError::RoutingTableEntry(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableError::Name(ref err) => write!(f, "Routing Table Name Error caused by {}", err),
			RoutingTableError::Size(ref err) => write!(f, "Routing Table Size Error caused by {}", err),
			RoutingTableError::Traph(ref err) => write!(f, "Routing Table Traph Error caused by {}", err),
			RoutingTableError::RoutingTableEntry(ref err) => write!(f, "Routing Table Entry Error caused by {}", err),
		}
	}
}
impl From<NameError> for RoutingTableError {
	fn from(err: NameError) -> RoutingTableError { RoutingTableError::Name(err) }
}
impl From<RoutingTableEntryError> for RoutingTableError {
	fn from(err: RoutingTableEntryError) -> RoutingTableError { RoutingTableError::RoutingTableEntry(err) }
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
#[derive(Debug)]
pub struct TraphError { msg: String }
impl TraphError {
	pub fn new(tree_id: &TreeID) -> TraphError {
		TraphError { msg: format!("TreeID {} does not exist", tree_id) }
	}
}
impl Error for TraphError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for TraphError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<TraphError> for RoutingTableError {
	fn from(err: TraphError) -> RoutingTableError { RoutingTableError::Traph(err) }
}

