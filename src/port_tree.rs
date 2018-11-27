use std::fmt;

use config::{PathLength, PortNo};
use name::{Name, TreeID};
use routing_table_entry::RoutingTableEntry;
use utility::{PortNumber};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct PortTree {
    port_tree_id: TreeID,
    root_port_no: PortNo,
    in_port_no: PortNo,
    hops: PathLength,
    entry: RoutingTableEntry
}
impl PortTree {
    pub fn new(tree_id: &TreeID, root_port_number: &PortNumber, in_port_no: &PortNo, hops: &PathLength)
            -> PortTree {
        let port_tree_id = tree_id.with_root_port_number(root_port_number);
        PortTree { port_tree_id, root_port_no: root_port_number.get_port_no(),
                   in_port_no: *in_port_no, hops: *hops, entry: RoutingTableEntry::default() }
    }
    pub fn get_port_tree_id(&self) -> &TreeID { &self.port_tree_id }
    pub fn get_root_port_no(&self) -> &PortNo { &self.root_port_no }
    pub fn _get_in_port_no(&self) -> &PortNo { &self.in_port_no }
    pub fn _get_hops(&self) -> &PathLength { &self.hops }
    pub fn get_entry(&self) -> &RoutingTableEntry { &self.entry }
    pub fn set_entry(&mut self, new_entry: RoutingTableEntry) { self.entry = new_entry; }
}

impl fmt::Display for PortTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("PortTree: TreeID {} {:8?}, root_port {}, in_port {}, hops {} entry {}",
                        self.port_tree_id, self.port_tree_id.get_uuid(), *self.root_port_no,
                        *self.in_port_no, self.hops, self.entry);
        write!(f, "{}", s)
    }
}
