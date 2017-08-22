use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use uuid::Uuid;

use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use gvm_equation::GvmEquation;
use name::{Name, CellID, TreeID};
use routing_table_entry::{RoutingTableEntry};
use traph_element::TraphElement;
use tree::Tree;
use utility::{Path, PortNumber};

type StackedTrees = HashMap<Uuid, Tree>;

#[derive(Debug, Clone)]
pub struct Traph {
	cell_id: CellID, // For debugging
	black_tree_id: TreeID,
	stacked_trees: Arc<Mutex<StackedTrees>>,
	elements: Vec<TraphElement>,
}
impl Traph {
	pub fn new(cell_id: &CellID, black_tree_id: &TreeID, index: TableIndex) -> Result<Traph> {
		let mut elements = Vec::new();
		for i in 1..MAX_PORTS.v { 
			elements.push(TraphElement::default(PortNumber::new(PortNo{v:i as u8}, MAX_PORTS).chain_err(|| ErrorKind::TraphError)?)); 
		}
		let gvm_eqn = GvmEquation::new("true", "true", "true", Vec::new());
		let entry = RoutingTableEntry::default(index).chain_err(|| ErrorKind::TraphError)?;
		let black_tree = Tree::new(black_tree_id, black_tree_id, Some(gvm_eqn), entry);
		let stacked_trees = Arc::new(Mutex::new(HashMap::new()));
		{
			let mut locked = stacked_trees.lock().unwrap();
			locked.insert(black_tree_id.get_uuid(), black_tree);
		}
		Ok(Traph { cell_id: cell_id.clone(), black_tree_id: black_tree_id.clone(),
				stacked_trees: stacked_trees, elements: elements })
	}
	pub fn get_black_tree_id(&self) -> &TreeID { &self.black_tree_id }
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
		Err(ErrorKind::Parent(self.black_tree_id.clone()).into())
	}
	pub fn get_hops(&self) -> Result<PathLength> {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Parent { return Ok(element.get_hops()); }
		}
		Err(ErrorKind::NoParent(self.black_tree_id.clone()).into())	
	}
	pub fn is_leaf(&self) -> bool {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Child { return false; }
		}
		true
	}
	pub fn get_table_entry(&self, stacked_trees_locked: &MutexGuard<StackedTrees>, tree_uuid: &Uuid) -> Result<RoutingTableEntry> { 
		match stacked_trees_locked.get(tree_uuid) {
			Some(tree) => Ok(tree.get_table_entry()),
			None => Err(ErrorKind::Tree(tree_uuid.clone()).into())
		}
	}
	pub fn get_table_index(&self, tree_uuid: &Uuid) -> Result<TableIndex> {
		let locked = self.stacked_trees.lock().unwrap(); 
		let table_entry = self.get_table_entry(&locked, tree_uuid)?;
		Ok(table_entry.get_index())
	}
	pub fn new_element(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: PortStatus, 
			other_index: TableIndex, children: &HashSet<PortNumber>, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry> {
		let port_no = port_number.get_port_no();
		let mut locked = self.stacked_trees.lock().unwrap();
		let mut tree = match locked.get(&tree_id.get_uuid()) {
			Some(tree) => tree.clone(),
			None => return Err(ErrorKind::Tree(tree_id.get_uuid()).into())
		};
		let mut table_entry = tree.get_table_entry();
		match port_status {
			PortStatus::Parent => table_entry.set_parent(port_number),
			PortStatus::Child => {
				let mut children = HashSet::new();
				children.insert(port_number);
				table_entry.add_children(&children)
			},
			_ => ()
		};
		table_entry.add_other_index(port_number, other_index);
		table_entry.add_children(children);
		table_entry.set_inuse();
		table_entry.set_tree_id(tree_id);
		tree.set_table_entry(table_entry);
		locked.insert(tree_id.get_uuid(), tree);
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[*port_no as usize] = element;
		Ok(table_entry)
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Traph for Black TreeID {}", 
			self.black_tree_id);
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
			display("Traph: No traph entry for port {}", port_number)
		}
		NoParent(tree_id: TreeID) {
			display("Traph: No parent for tree {}", tree_id)
		}
		Parent(tree_id: TreeID) {
			display("Traph: No parent for tree {}", tree_id)
		}
		Tree(tree_uuid: Uuid) {
			display("Traph: No tree with UUID {}", tree_uuid)
		}
	}
}
