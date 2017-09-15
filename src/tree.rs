use std::fmt;
use std::collections::HashSet;
use uuid::Uuid;

use cellagent::CellAgent;
use config::{MAX_PORTS, PortNo, TableIndex};
use gvm_equation::GvmEquation;
use name::{Name, TreeID};
use routing_table_entry::RoutingTableEntry;
use utility::{Mask, PortNumber};

#[derive(Debug, Clone)]
pub struct Tree {
	tree_id: TreeID,
	black_tree_id: TreeID,
	stacked_tree_ids: Vec<TreeID>,
	table_entry: RoutingTableEntry,
	gvm_eqn: Option<GvmEquation>,
}
impl Tree {
	pub fn new(tree_id: &TreeID, black_tree_id: &TreeID, gvm_eqn: Option<GvmEquation>, 
			table_entry: RoutingTableEntry) -> Tree {
		Tree { black_tree_id: black_tree_id.clone(), tree_id: tree_id.clone(), gvm_eqn: gvm_eqn,
				table_entry: table_entry, stacked_tree_ids: Vec::new() }
	}
	pub fn get_id(&self) -> &TreeID { &self.tree_id }
	pub fn get_black_tree_id(&self) -> &TreeID { &self.black_tree_id }
	pub fn get_uuid(&self) -> Uuid { self.tree_id.get_uuid() }
	pub fn get_table_entry(&self) -> RoutingTableEntry { self.table_entry }
	pub fn set_table_entry(&mut self, entry: RoutingTableEntry) { self.table_entry = entry; }
	pub fn get_table_index(&self) -> TableIndex { self.table_entry.get_index() }
	pub fn get_gvm_eqn(&self) -> Option<GvmEquation> { self.gvm_eqn.clone() }
	pub fn set_gvm_eqn(&mut self, gvm_eqn: GvmEquation) { self.gvm_eqn = Some(gvm_eqn) }
	pub fn get_parent(&self) -> PortNo { self.get_table_entry().get_parent() }	
	pub fn set_parent(&mut self, port_number: PortNumber) { self.get_table_entry().set_parent(port_number); }
	pub fn add_children(&mut self, children: &HashSet<PortNumber>) { 
		self.get_table_entry().add_children(children); 
	}	
	pub fn add_other_index(&mut self, port_number: PortNumber, other_index: TableIndex) { 
		self.get_table_entry().add_other_index(port_number, other_index) 
	}
	pub fn set_inuse(&mut self) { self.get_table_entry().set_inuse(); }
}
impl fmt::Display for Tree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("TreeID {}, ", self.tree_id);
		match self.gvm_eqn {
			Some(ref eqn) => s = s + &format!("{}", eqn),
			None => s = s + &format!("No GVM equation")
		};
		for stacked in &self.stacked_tree_ids {
			s = s + &format!("\n{}", stacked);
		}
		write!(f, "{}", s)
	}	
}
error_chain! {
	errors {
	}
}