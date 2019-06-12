use std::fmt;

use crate::config::{PathLength, PortNo};
use crate::name::{PortTreeID};
use crate::routing_table_entry::RoutingTableEntry;
use crate::utility::{PortNumber};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct PortTree {
    port_tree_id: PortTreeID,
    root_port_no: PortNo,
    in_port_no: PortNo,
    hops: PathLength,
    entry: RoutingTableEntry
}
impl PortTree {
    pub fn new(port_tree_id: PortTreeID, in_port_no: PortNo, hops: PathLength)
            -> PortTree {
        let root_port_no = port_tree_id.get_port_no();
        let mut entry = RoutingTableEntry::default();
        entry.set_tree_id(port_tree_id);
        entry.add_child(PortNumber::new0());
        PortTree { port_tree_id: port_tree_id.clone(), root_port_no,
                   in_port_no, hops, entry }
    }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.port_tree_id }
    pub fn get_root_port_no(&self) -> PortNo { self.root_port_no }
    pub fn get_in_port_no(&self) -> PortNo { self.in_port_no }
    pub fn _get_hops(&self) -> &PathLength { &self.hops }
    pub fn get_entry(&self) -> RoutingTableEntry { self.entry }
    pub fn _has_child(&self, child: PortNumber) -> bool { self.entry._has_child(child) }
    pub fn set_entry(&mut self, new_entry: RoutingTableEntry) { self.entry = new_entry; }
    pub fn add_child(&mut self, child: PortNumber) -> RoutingTableEntry {
        self.entry.add_child(child)
    }
    pub fn remove_child(&mut self, child: PortNumber) -> RoutingTableEntry {
        self.entry.remove_child(child)
    }
    pub fn set_parent(&mut self, new_parent: PortNumber) -> RoutingTableEntry {
        self.entry.set_parent(new_parent)
    }
    pub fn _make_child_parent(&mut self, child: PortNumber) -> RoutingTableEntry {
        self.remove_child(child);
        self.set_parent(child)
    }
}

impl fmt::Display for PortTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("PortTree: TreeID {}: root_port {}, in_port {}, hops {} entry {}",
                        self.port_tree_id, *self.root_port_no,
                        *self.in_port_no, self.hops, self.entry);
        write!(f, "{}", s)
    }
}
