use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::hash_map::Entry;

use uuid::Uuid;

use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use name::{Name, CellID, TreeID};
use routing_table_entry::{RoutingTableEntry};
use traph_element::TraphElement;
use tree::Tree;
use utility::{Mask, Path, PortNumber};

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
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("true"));
		let gvm_eqn = GvmEquation::new(eqns, Vec::new());
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
	pub fn get_tree_entry(&self, tree_uuid: &Uuid) -> Result<RoutingTableEntry> {
		let locked = self.stacked_trees.lock().unwrap();
		if let Some(tree) = locked.get(tree_uuid) {
			Ok(tree.get_table_entry())
		} else {
			Err(ErrorKind::Tree(self.cell_id.clone(), "get_tree_entry".to_string(), tree_uuid.clone()).into())
		}
	}
	pub fn get_black_tree_entry(&self) -> Result<RoutingTableEntry> { self.get_tree_entry(&self.black_tree_id.get_uuid()) }
	pub fn get_stacked_trees(&self) -> &Arc<Mutex<StackedTrees>> { &self.stacked_trees }
	pub fn has_tree(&self, tree_id: &TreeID) -> bool {
		let stacked_trees = self.stacked_trees.lock().unwrap();
		stacked_trees.contains_key(&tree_id.get_uuid())
	}
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
		println!("+++ Cell {}: {}", self.cell_id, self);
		Err(ErrorKind::ParentElement(self.cell_id.clone(), "get_parent_element".to_string(), self.black_tree_id.clone()).into())
	}
	pub fn get_hops(&self) -> Result<PathLength> {
		let element = self.get_parent_element()?;
		return Ok(element.get_hops()); 
	}
	pub fn is_leaf(&self) -> bool {
		for element in self.elements.clone() {
			if element.get_status() == PortStatus::Child { return false; }
		}
		true
	}
	pub fn count_connected(&self) -> usize {
		let mut i = 0;
		for element in &self.elements {
			if element.is_connected() { i += 1; }
		}
		i
	}
	pub fn get_table_entry(&self, stacked_trees_locked: &MutexGuard<StackedTrees>, tree_uuid: &Uuid) -> Result<RoutingTableEntry> { 
		match stacked_trees_locked.get(tree_uuid) {
			Some(tree) => Ok(tree.get_table_entry()),
			None => Err(ErrorKind::Tree(self.cell_id.clone(), "get_table_entry".to_string(), tree_uuid.clone()).into())
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
		// I get lifetime errors if I put this block in a function
		let mut tree = match locked.get(&tree_id.get_uuid()) {
			Some(tree) => tree.clone(),
			None => return Err(ErrorKind::Tree(self.cell_id.clone(), "new_element".to_string(), tree_id.get_uuid()).into())
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
	pub fn update_stacked_entries(&self, base_tree_entry: RoutingTableEntry) -> Result<Vec<RoutingTableEntry>> {		
		let locked = self.stacked_trees.lock().unwrap();
		let mut updated_entries = Vec::new();
		for stacked_tree in locked.values() {
			if stacked_tree.get_id() != stacked_tree.get_black_tree_id() {
				let mut stacked_entry = stacked_tree.get_table_entry();
				let port_number = PortNumber::new(base_tree_entry.get_parent(), PortNo{v: base_tree_entry.get_other_indices().len() as u8})?;
				stacked_entry.set_parent(port_number);
				stacked_entry.set_mask(base_tree_entry.get_mask());
				if let Some(gvm_eqn) = stacked_tree.get_gvm_eqn() {
					let params = self.get_params(gvm_eqn.get_variables())?;
					if !gvm_eqn.eval_recv(&params)? { 
						let mask = stacked_entry.get_mask().and(Mask::all_but_zero(PortNo{v:stacked_entry.get_other_indices().len() as u8}));
						stacked_entry.set_mask(mask);
					}
				}
				stacked_entry.set_other_indices(base_tree_entry.get_other_indices());	
				updated_entries.push(stacked_entry);
			}
		}
		Ok(updated_entries)		
	}
	pub fn stack_tree(&mut self, tree: &Tree) {
		self.stacked_trees.lock().unwrap().insert(tree.get_uuid(), tree.clone());
	}
	pub fn get_params(&self, vars: &Vec<GvmVariable>) -> Result<Vec<GvmVariable>> {
		let mut variables = Vec::new();
		for var in vars {
			match var.get_value().as_ref() {
				"hops" => {
					let hops = *(self.get_hops()?);
					variables.push(GvmVariable::new(GvmVariableType::CellNo, *hops));
				},
				_ => ()
			}
		}
		Ok(variables)
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Traph {}", 
			self.black_tree_id);
		s = s + &format!("\n  Stacked Trees");
		let locked = self.stacked_trees.lock().unwrap();
		for tree in locked.values() {
			s = s + &format!("\n  {}", tree);
		}
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
		Gvm(::gvm_equation::Error, ::gvm_equation::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		RoutingtableEntry(::routing_table_entry::Error, ::routing_table_entry::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { TraphError
		Lookup(cell_id: CellID, func_name: String, port_number: PortNumber) {
			display("{}: Traph on cell {}: No traph entry for port {}", func_name, cell_id, port_number)
		}
		ParentElement(cell_id: CellID, func_name: String, tree_id: TreeID) {
			display("{}: Traph on cell {}: No parent element for tree {}", func_name, cell_id, tree_id)
		}
		Tree(cell_id: CellID, func_name: String, tree_uuid: Uuid) {
			display("{}: Traph on cell {}: No tree with UUID {}", func_name, cell_id, tree_uuid)
		}
	}
}
