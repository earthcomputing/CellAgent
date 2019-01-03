use std::{fmt, fmt::Write};

//use uuid::Uuid;

use crate::gvm_equation::GvmEquation;
use crate::name::{Name, PortTreeID, TreeID};
use crate::routing_table_entry::RoutingTableEntry;
use crate::utility::PortNumber;
use crate::uuid_ec::Uuid;

#[derive(Debug, Clone)]
pub struct Tree {
    port_tree_id: PortTreeID,
    base_tree_id: TreeID,
    parent_tree_id: PortTreeID,
    stacked_tree_ids: Vec<PortTreeID>,
    table_entry: RoutingTableEntry,
    gvm_eqn: GvmEquation,
}
impl Tree {
    pub fn new(tree_id: &PortTreeID, base_tree_id: &TreeID, parent_tree_id: &PortTreeID,
               gvm_eqn: &GvmEquation, table_entry: RoutingTableEntry) -> Tree {
        Tree { base_tree_id: base_tree_id.clone(), port_tree_id: tree_id.clone(),
            parent_tree_id: parent_tree_id.clone(),
            gvm_eqn: gvm_eqn.clone(), table_entry, stacked_tree_ids: Vec::new() }
    }
    pub fn get_port_tree_id(&self) -> &PortTreeID { &self.port_tree_id }
    //pub fn get_base_tree_id(&self) -> &TreeID { &self.base_tree_id }
    //pub fn get_parent_tree_id(&self) -> &TreeID { &self.parent_tree_id }
    pub fn get_stacked_tree_ids(&self) -> &Vec<PortTreeID> { &self.stacked_tree_ids }
    pub fn get_uuid(&self) -> Uuid { self.port_tree_id.get_uuid() }
    pub fn get_table_entry(&self) -> RoutingTableEntry { self.table_entry }
    pub fn set_table_entry(&mut self, entry: RoutingTableEntry) { self.table_entry = entry; }
    //pub fn get_table_index(&self) -> TableIndex { self.table_entry.get_index() }
    pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
    pub fn has_child(&self, child: PortNumber) -> bool { self.table_entry.has_child(child) }
    pub fn add_child(&mut self, child: PortNumber) -> RoutingTableEntry { self.table_entry.add_child(child) }
    pub fn remove_child(&mut self, child: PortNumber) -> RoutingTableEntry { self.table_entry.remove_child(child) }
    pub fn swap_children(&mut self, old_child: PortNumber, new_child: PortNumber) {
        self.remove_child(old_child);
        self.add_child(new_child);
    }
    //pub fn set_gvm_eqn(&mut self, gvm_eqn: GvmEquation) { self.gvm_eqn = gvm_eqn; }
    //pub fn get_parent(&self) -> PortNo { self.get_table_entry().get_parent() }
    //pub fn set_parent(&mut self, port_number: PortNumber) { self.get_table_entry().set_parent(port_number); }
    //pub fn add_children(&mut self, children: &HashSet<PortNumber>) {
    //    self.get_table_entry().add_children(children);
    //}
    //pub fn add_other_index(&mut self, port_number: PortNumber, other_index: TableIndex) {
    //    self.get_table_entry().add_other_index(port_number, other_index)
    //}
    //pub fn set_inuse(&mut self) { self.get_table_entry().set_inuse(); }
}
impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("TreeID {}: {} {}", self.port_tree_id, self.table_entry, self.gvm_eqn);
        for stacked in &self.stacked_tree_ids {
            write!(s, "\n{} {}", stacked, stacked.get_uuid())?;
        }
        write!(f, "{}", s)
    }
}
// Errors
//#[derive(Debug, Fail)]
//pub enum RoutingTableError {
//    #[fail(display = "RoutingTableError::Chain {} {}", func_name, comment)]
//    Chain { func_name: &'static str, comment: String },
//}
