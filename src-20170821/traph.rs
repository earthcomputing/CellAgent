use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use uuid::Uuid;

use config::{PathLength, PortNo, TableIndex};
use name::{Name, TreeID};
use routing_table_entry::{RoutingTableEntry};
use tree::{Tree};
use traph_element::TraphElement;
use utility::{Path, PortNumber};

pub type StackedTrees = HashMap<Uuid, Tree>;

#[derive(Debug, Clone)]
pub struct Traph {
	id: TreeID,  // Black tree ID
	stacked_trees: Arc<Mutex<StackedTrees>>,
	elements: Vec<TraphElement>,
}
impl Traph {
	pub fn new(id: &TreeID, index: TableIndex, no_ports: PortNo) -> Result<Traph> {
		let mut elements = Vec::new();
		for i in 1..no_ports.v { 
			elements.push(TraphElement::default(PortNumber::new(PortNo{v:i as u8}, no_ports).chain_err(|| ErrorKind::TraphError)?)); 
		}
		Ok(Traph { id: id.clone(), stacked_trees: Arc::new(Mutex::new(HashMap::new())), elements: elements })
	}
	pub fn get_id(&self) -> &TreeID { &self.id }
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
		Err(ErrorKind::NoParent(self.id.clone()).into())
	}
	pub fn get_hops(&self) -> Result<PathLength> {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Parent { return Ok(element.get_hops()); }
		}
		Err(ErrorKind::NoParent(self.id.clone()).into())	
	}
	pub fn is_leaf(&self) -> bool {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Child { return false; }
		}
		true
	}
	pub fn add_stacked_tree(&mut self, tree: Tree) { self.stacked_trees.lock().unwrap().insert(tree.get_uuid().clone(), tree); }
	pub fn lock_stacked_trees(&self, tree_id: &TreeID) -> Result<MutexGuard<StackedTrees>> {
		match self.stacked_trees.lock() {
			Ok(locked) => Ok(locked),
			Err(err) => Err(ErrorKind::Tree(tree_id.clone()).into())
		}
	}
	pub fn new_element(&mut self, traph_id: &TreeID, stacked_trees: &mut Arc<Mutex<HashMap<Uuid,Tree>>>, 
			port_number: PortNumber, port_status: PortStatus, other_index: TableIndex, 
			children: &HashSet<PortNumber>, hops: PathLength, path: Option<Path>) -> Result<()> {
		let port_no = port_number.get_port_no();
		let mut locked = stacked_trees.lock().unwrap();
		match port_status {
			PortStatus::Parent => {let _ = locked.values_mut().map(|mut tree| tree.set_parent(port_number));},
			PortStatus::Child => {
				let mut children = HashSet::new();
				children.insert(port_number);
				let _ = locked.values_mut().map(|tree| tree.add_children(&children));
			},
			_ => ()
		};
		let _ = locked.values_mut().map(|tree| tree.add_other_index(port_number, other_index));
		let _ = locked.values_mut().map(|tree| tree.add_children(children));
		let _ = locked.values_mut().map(|tree| tree.set_inuse());
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[port_no.v as usize] = element;
		Ok(())
	}
	fn update_parent(&self, stacked_trees: &mut Vec<Tree>, parent: PortNumber) {
		for tree in stacked_trees {
			tree.set_parent(parent);
		}
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Black Tree ID {} {}", self.id, self.id.get_uuid());
		s = s + &format!("\nPort Other Connected Broken Status Hops Path");
		// Can't replace with map() because s gets moved into closure 
		for element in self.elements.iter() { 
			if element.is_connected() { s = s + &format!("\n{}",element); } 
		}
		s = s + &format!("\n Stacked TreeIDs {:?}", self.stacked_trees.lock().unwrap().values());
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
			display("Traph: No traph entry for port {}", port_number)
		}
		NoParent(tree_id: TreeID) {
			display("Traph: No parent for traph {}", tree_id)
		}
		Tree(tree_id: TreeID) {
			display("No tree with id {}", tree_id)
		}
	}
}
