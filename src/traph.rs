use std::fmt;
use std::collections::HashSet;

use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use name::{Name, TreeID};
use routing_table_entry::{RoutingTableEntry};
use traph_element::TraphElement;
use utility::{Path, PortNumber};

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	my_index: TableIndex,
	table_entry: RoutingTableEntry,
	elements: Vec<TraphElement>,
}
impl Traph {
	pub fn new(tree_id: &TreeID, index: TableIndex) -> Result<Traph> {
		let mut elements = Vec::new();
		for i in 1..MAX_PORTS.v { 
			elements.push(TraphElement::default(PortNumber::new(PortNo{v:i as u8}, MAX_PORTS).chain_err(|| ErrorKind::TraphError)?)); 
		}
		let entry = RoutingTableEntry::default(index).chain_err(|| ErrorKind::TraphError)?;
		Ok(Traph { tree_id: tree_id.clone(), my_index: index,
				table_entry: entry, elements: elements })
	}
	pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
	pub fn get_port_status(&self, port_number: PortNumber) -> PortStatus { 
		let port_no = port_number.get_port_no();
		match self.elements.get(port_no.v as usize) {
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
		Err(ErrorKind::Parent(self.tree_id.clone()).into())
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
	pub fn new_element(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: PortStatus, 
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
		self.table_entry.set_tree_id(tree_id);
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[port_no.v as usize] = element;
		Ok(self.table_entry)
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Traph for TreeID {} {}\nTable Entry Index {}", 
			self.tree_id, self.tree_id.get_uuid(), self.table_entry.get_index().0);
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
		Parent(tree_id: TreeID) {
			display("No parent for tree {}", tree_id)
		}
	}
}
