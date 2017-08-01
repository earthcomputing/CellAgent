use std::fmt;
use std::collections::HashSet;

use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use name::{CellID, TreeID};
use routing_table_entry::{RoutingTableEntry};
use utility::{Path, PortNumber};

#[derive(Debug, Clone)]
pub struct Traph {
	cell_id: CellID,
	tree_id: TreeID,
	my_index: TableIndex,
	table_entry: RoutingTableEntry,
	elements: Vec<TraphElement>,
}

impl Traph {
	pub fn new(cell_id: CellID, tree_id: TreeID, index: TableIndex) -> Result<Traph> {
		let mut elements = Vec::new();
		for i in 1..MAX_PORTS { 
			elements.push(TraphElement::default(PortNumber::new(i, MAX_PORTS).chain_err(|| ErrorKind::TraphError)?)); 
		}
		let entry = RoutingTableEntry::default(index).chain_err(|| ErrorKind::TraphError)?;
		Ok(Traph { cell_id: cell_id, tree_id: tree_id, my_index: index, 
				table_entry: entry, elements: elements })
	}
//	pub fn get_tree_id(&self) -> TreeID { self.tree_id.clone() }
	pub fn get_port_status(&self, port_number: PortNumber) -> PortStatus { 
		let port_no = port_number.get_port_no();
		match self.elements.get(port_no as usize) {
			Some(e) => e.get_status(),
			None => PortStatus::Pruned
		}
	}
	pub fn get_parent_element(&self) -> Result<&TraphElement> {
		for element in &self.elements {
			match element.get_status() {
				PortStatus::Parent => return Ok(element),
				_ => ()
			}
		}
		Err(ErrorKind::Parent(self.tree_id.clone(), self.cell_id.clone()).into())
	}
	pub fn get_hops(&self) -> Result<PathLength> {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Parent { return Ok(element.get_hops()); }
		}
		Err(ErrorKind::NoParent(self.tree_id.clone()).into())	
	}
	pub fn is_leaf(&self) -> bool {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Child { return false; }
		}
		true
	}
	pub fn get_table_entry(&self) -> RoutingTableEntry { self.table_entry }
	pub fn get_table_index(&self) -> TableIndex { self.table_entry.get_index() }
	pub fn new_element(&mut self, port_number: PortNumber, port_status: PortStatus, 
			other_index: TableIndex, children: &HashSet<PortNumber>, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry> {
		let port_no = port_number.get_port_no();
		match port_status {
			PortStatus::Parent => self.table_entry.set_parent(port_number),
			PortStatus::Child => {
				let mut children = HashSet::new();
				children.insert(port_number);
				self.table_entry.add_children(&children)
			},
			_ => ()
		};
		self.table_entry.add_other_index(port_number, other_index);
		self.table_entry.add_children(children);
		self.table_entry.set_inuse();
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[port_no as usize] = element;
		Ok(self.table_entry)
	}
//	fn get_all_hops(&self) -> BTreeSet<PathLength> {
//		let mut set = BTreeSet::new();
//		//self.elements.iter().map(|e| set.insert(e.get_hops()));
//		for e in self.elements.iter() {
//			set.insert(e.get_hops());
//		}
//		set
//	}
	pub  fn get_other_indices(&self) -> [TableIndex; MAX_PORTS as usize] {
		let mut indices = [0; MAX_PORTS as usize];
		// Not sure why map gives warning about unused result
		//self.elements.iter().map(|e| indices[e.get_port_no() as usize] = e.get_other_index());
		for e in self.elements.iter() {
			indices[e.get_port_no() as usize] = e.get_other_index();
		}
		indices
	}
//	pub fn set_connected(&mut self, port_no: PortNumber) -> Result<(), TraphError> { 
//		self.set_connected_state(port_no, true); 
//		Ok(())
//	}
//	pub fn set_disconnected(&mut self, port_no: PortNumber) -> Result<(), TraphError> { 
//		self.set_connected_state(port_no, false); 
//		Ok(())
//	}
//	fn set_connected_state(&mut self, port_no: PortNumber, state: bool) -> Result<(),TraphError> {
//		if state { self.elements[port_no.get_port_no() as usize].set_connected(); }
//		else     { self.elements[port_no.get_port_no() as usize].set_disconnected(); }
//		Ok(())
//	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Cell {}: Traph for TreeID {}\nTable Entry Index {}", 
			self.cell_id, self.tree_id, self.table_entry.get_index());
		s = s + &format!("\nPort Other Connected Broken Status Hops Path");
		// Can't replace with map() because s gets moved into closure 
		for element in self.elements.iter() { 
			if element.is_connected() { s = s + &format!("\n{}",element); } 
		}
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PortStatus {
	Parent,
	Child,
	Pruned
}
impl fmt::Display for PortStatus {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PortStatus::Parent => write!(f, "Parent"),
			PortStatus::Child  => write!(f, "Child "),
			PortStatus::Pruned => write!(f, "Pruned")
		}
	}
}
// Errors
error_chain! {
	links {
		Name(::name::Error, ::name::ErrorKind);
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		RoutingtableEntry(::routing_table_entry::Error, ::routing_table_entry::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { TraphError
		Lookup(port_number: PortNumber) {
			display("No traph entry for port {}", port_number)
		}
		NoParent(tree_id: TreeID) {
			display("No parent for tree {}", tree_id)
		}
		Parent(tree_id: TreeID, cell_id: CellID) {
			display("No parent for tree {} on cell {}", tree_id, cell_id)
		}
	}
}
#[derive(Debug, Copy, Clone)]
pub struct TraphElement {
	port_no: PortNo,
	other_index: TableIndex,
	is_connected: bool,
	is_broken: bool,
	status: PortStatus,
	hops: PathLength,
	path: Option<Path> 
}
impl TraphElement {
	fn new(is_connected: bool, port_no: PortNo, other_index: TableIndex, 
			status: PortStatus, hops: PathLength, path: Option<Path>) -> TraphElement {
		TraphElement { port_no: port_no,  other_index: other_index, 
			is_connected: is_connected, is_broken: false, status: status, 
			hops: hops, path: path } 
	}
	fn default(port_number: PortNumber) -> TraphElement {
		let port_no = port_number.get_port_no();
		TraphElement::new(false, port_no, 0 as TableIndex, PortStatus::Pruned, 
					0 as PathLength, None)
	}
	fn get_port_no(&self) -> PortNo { self.port_no }
	pub fn get_hops(&self) -> PathLength { self.hops }
	pub fn get_path(&self) -> Option<Path> { self.path }
	fn get_status(&self) -> PortStatus { self.status }
	fn get_other_index(&self) -> TableIndex { self.other_index }
	fn is_connected(&self) -> bool { self.is_connected }
	fn set_connected(&mut self) { self.is_connected = true; }
//	fn set_disconnected(&mut self) { self.is_connected = false; }
	fn set_status(&mut self, status: PortStatus) { self.status = status; }	
}
impl fmt::Display for TraphElement {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("{:4} {:5} {:9} {:6} {:6} {:4}", 
			self.port_no, self.other_index, self.is_connected, self.is_broken, self.status, self.hops);
		match self.path {
			Some(p) => s = s + &format!(" {:4}", p.get_port_no()),
			None    => s = s + &format!(" None")
		}
		write!(f, "{}", s)
	}
}