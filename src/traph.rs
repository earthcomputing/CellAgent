use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use failure::{Error, Fail, ResultExt};
use serde_json;
//use uuid::Uuid;

use config::{MAX_PORTS, PathLength, PortNo, TableIndex};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use name::{Name, CellID, TreeID};
use routing_table_entry::{RoutingTableEntry};
use traph_element::TraphElement;
use tree::Tree;
use utility::{Mask, Path, PortNumber, S};
use uuid_fake::Uuid;

type StackedTrees = HashMap<Uuid, Tree>;

#[derive(Debug, Clone)]
pub struct Traph {
	cell_id: CellID, // For debugging
	black_tree_id: TreeID,
	stacked_trees: Arc<Mutex<StackedTrees>>,
	elements: Vec<TraphElement>,
}
impl Traph {
	pub fn new(cell_id: &CellID, black_tree_id: &TreeID, index: TableIndex, gvm_eqn: Option<&GvmEquation>) -> Result<Traph, Error> {
		let mut elements = Vec::new();
		for i in 1..MAX_PORTS.v {
			let port_number = PortNumber::new(PortNo{v:i as u8}, MAX_PORTS).context(TraphError::Chain { func_name: "new", comment: S("")})?;
			elements.push(TraphElement::default(port_number));
		}
		let entry = RoutingTableEntry::default(index).context(TraphError::Chain { func_name: "new", comment: S("")})?;
		let black_tree = Tree::new(black_tree_id, black_tree_id, gvm_eqn, entry);
		let stacked_trees = Arc::new(Mutex::new(HashMap::new()));
		{
			let mut locked = stacked_trees.lock().unwrap();
			locked.insert(black_tree_id.get_uuid(), black_tree);
		}
		Ok(Traph { cell_id: cell_id.clone(), black_tree_id: black_tree_id.clone(),
				stacked_trees: stacked_trees, elements: elements })
	}
	pub fn get_black_tree_id(&self) -> &TreeID { &self.black_tree_id }
	pub fn get_tree_entry(&self, tree_uuid: &Uuid) -> Result<RoutingTableEntry, Error> {
		let locked = self.stacked_trees.lock().unwrap();
		if let Some(tree) = locked.get(tree_uuid) {
			Ok(tree.get_table_entry())
		} else {
            for stacked_tree in locked.iter() {
                println!("Traph for cell {}: stacked_trees {}", self.cell_id, stacked_tree.0);
            }
			Err(TraphError::Tree { cell_id: self.cell_id.clone(), func_name: "get_tree_entry", tree_uuid: *tree_uuid }.into())
		}
	}
	pub fn get_black_tree_entry(&self) -> Result<RoutingTableEntry, Error> {
        Ok(self.get_tree_entry(&self.black_tree_id.get_uuid()).context(TraphError::Chain { func_name: "get_black_tree_entry", comment: S("")})?)
    }
	pub fn get_stacked_trees(&self) -> &Arc<Mutex<StackedTrees>> { &self.stacked_trees }
	pub fn has_tree(&self, tree_id: &TreeID) -> bool {
		let stacked_trees = self.stacked_trees.lock().unwrap();
		stacked_trees.contains_key(&tree_id.get_uuid())
	}
	pub fn is_port_connected(&self, port_number: PortNumber) -> bool {
        let port_no = port_number.get_port_no();
        match self.elements.get(*port_no as usize) {
            Some(e) => e.is_connected(),
            None => false
        }
    }
	pub fn get_port_status(&self, port_number: PortNumber) -> PortStatus {
		let port_no = port_number.get_port_no();
		match self.elements.get(port_no.v as usize) {
			Some(e) => e.get_status(),
			None => PortStatus::Pruned
		}
	}
	pub fn get_parent_element(&self) -> Result<&TraphElement, TraphError> {
		for element in &self.elements {
			match element.get_status() {
				PortStatus::Parent => return Ok(element),
				_ => ()
			}
		}
		Err(TraphError::ParentElement { cell_id: self.cell_id.clone(), func_name: "get_parent_element", tree_id: self.black_tree_id.clone() }.into())
	}
	pub fn get_hops(&self) -> Result<PathLength, Error> {
		let element = self.get_parent_element().context(TraphError::Chain { func_name: "get_hops", comment: S("")})?;
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
	pub fn get_table_entry(&self, stacked_trees_locked: &MutexGuard<StackedTrees>, tree_uuid: &Uuid)
            -> Result<RoutingTableEntry, TraphError> {
		match stacked_trees_locked.get(tree_uuid) {
			Some(tree) => Ok(tree.get_table_entry()),
			None => Err(TraphError::Tree { cell_id: self.cell_id.clone(), func_name: "get_table_entry", tree_uuid: tree_uuid.clone() }.into())
		}
	}
	pub fn get_table_index(&self, tree_uuid: &Uuid) -> Result<TableIndex, Error> {
		let locked = self.stacked_trees.lock().unwrap(); 
		let table_entry = self.get_table_entry(&locked, tree_uuid).context(TraphError::Chain { func_name: "get_table_index", comment: S("")})?;
		Ok(table_entry.get_index())
	}
	pub fn new_element(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: PortStatus, 
			other_index: TableIndex, children: &HashSet<PortNumber>, hops: PathLength, path: Option<Path>) 
			-> Result<RoutingTableEntry, TraphError> {
		let port_no = port_number.get_port_no();
		let mut stacked_trees = self.stacked_trees.lock().unwrap();
		// I get lifetime errors if I put this block in a function
		let mut tree = match stacked_trees.get(&tree_id.get_uuid()).cloned() {
			Some(tree) => tree,
			None => return Err(TraphError::Tree { cell_id: self.cell_id.clone(), func_name: "new_element", tree_uuid: tree_id.get_uuid() }.into())
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
		stacked_trees.insert(tree_id.get_uuid(), tree);
		let element = TraphElement::new(true, port_no, other_index, port_status, hops, path);
		self.elements[*port_no as usize] = element;
		Ok(table_entry)
	}
	pub fn update_stacked_entries(&self, base_tree_entry: RoutingTableEntry) -> Result<Vec<RoutingTableEntry>, Error> {
		let locked = self.stacked_trees.lock().unwrap();
		let mut updated_entries = Vec::new();
		for stacked_tree in locked.values() {
			if stacked_tree.get_id() != stacked_tree.get_base_tree_id() {
				let mut stacked_entry = stacked_tree.get_table_entry();
				let port_no = PortNo{v: base_tree_entry.get_other_indices().len() as u8};
				let port_number = PortNumber::new(base_tree_entry.get_parent(), port_no).context(TraphError::Chain { func_name: "update_stacked_entries", comment: S("") })?;
				stacked_entry.set_parent(port_number);
				stacked_entry.set_mask(base_tree_entry.get_mask());
				if let Some(gvm_eqn) = stacked_tree.get_gvm_eqn() {
					let params = self.get_params(gvm_eqn.get_variables()).context(TraphError::Chain { func_name: "update_stacked_entries", comment: S("")})?;
					if !gvm_eqn.eval_recv(&params).context(TraphError::Chain { func_name: "update_stacked_entries", comment: S(self.cell_id.get_name()) + " recv"})? {
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
	pub fn stack_tree(&mut self, tree: Tree) {
		self.stacked_trees.lock().unwrap().insert(tree.get_uuid(), tree);
	}
	pub fn get_params(&self, vars: &Vec<GvmVariable>) -> Result<Vec<GvmVariable>, Error> {
		let mut variables = Vec::new();
		for var in vars {
			match var.get_var_name().as_ref() {
				"hops" => {
					let ref hops = self.get_hops().context(TraphError::Chain { func_name: "get_params", comment: S("")})?;
                    let str_hops = serde_json::to_string(hops).context(TraphError::Chain { func_name: "get_params", comment: S("") })?;
                    let mut updated = GvmVariable::new(GvmVariableType::PathLength, "hops");
                    updated.set_value(str_hops);
					variables.push(updated);
				},
				_ => ()
			}
		}
		Ok(variables)
	}
}
impl fmt::Display for Traph {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Traph {} {}", 
			self.black_tree_id, self.black_tree_id.get_uuid());
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
#[derive(Debug, Fail)]
pub enum TraphError {
	#[fail(display = "TraphError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
	#[fail(display = "TraphError::ParentElement {}: No parent element for tree {} on cell {}", func_name, tree_id, cell_id)]
	ParentElement { func_name: &'static str, cell_id: CellID, tree_id: TreeID },
    #[fail(display = "TraphError::Tree {}: No tree with UUID {} on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid }
}
