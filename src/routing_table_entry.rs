use std::{fmt, fmt::Write,
          collections::HashSet};

use crate::config::{PortNo};
use crate::name::{Name, PortTreeID, TreeID};
use crate::utility::{Mask, PortNumber};
use crate::uuid_ec::Uuid;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RoutingTableEntry {
    tree_uuid: Uuid,
    may_send: bool, // TODO: Move this from here to Tree
    inuse: bool,
    parent: PortNo,
    mask: Mask,
}
impl RoutingTableEntry {
    pub fn new(port_tree_id: &PortTreeID, inuse: bool, parent: PortNumber, mask: Mask,
            may_send: bool) -> RoutingTableEntry {
        RoutingTableEntry { tree_uuid: port_tree_id.get_uuid(), parent: parent.get_port_no(),
            may_send, inuse, mask }
    }
    pub fn default() -> RoutingTableEntry {
        let port_number = PortNumber::new0();
        let tree_id = TreeID::new("default").expect("The string 'default' is always a valid tree name");
        let port_tree_id = tree_id.to_port_tree_id_0();
        RoutingTableEntry::new(&port_tree_id, false, port_number, Mask::empty(), true)
    }
    pub fn is_in_use(&self) -> bool { self.inuse }
    pub fn may_send(&self) -> bool { self.may_send }
//  pub fn may_receive(&self) -> bool { !self.mask.and(Mask::port0()).equal(Mask::empty()) }
    pub fn enable_send(&mut self) { self.may_send = true; }
    pub fn disable_send(&mut self) { self.may_send = false; }
//  pub fn is_on_tree(&self) -> bool {
//        self.may_send || self.may_receive()
//    }
    pub fn get_uuid(&self) -> Uuid { self.tree_uuid }
    pub fn set_uuid(&mut self, uuid: &Uuid) { self.tree_uuid = *uuid; }
    pub fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
    pub fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
    pub fn set_inuse(&mut self) { self.inuse = true; }
//  pub fn set_not_inuse(&mut self) { self.inuse = false; }
    pub fn get_parent(&self) -> PortNo { self.parent }
    pub fn get_mask(&self) -> Mask { self.mask }
    pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
    pub fn set_tree_id(&mut self, port_tree_id: &PortTreeID) {
        self.tree_uuid = port_tree_id.get_uuid();
    }
//    pub fn get_other_index(&self, port_number: PortNumber) -> TableIndex {
//        let port_no = port_number.get_port_no().v as usize;
//        self.other_indices[port_no]
//    }
    pub fn add_children(&mut self, children: &HashSet<PortNumber>) {
        let mask = Mask::make(children);
        self.or_with_mask(mask);
    }
    pub fn add_child(&mut self, child: PortNumber) -> Self {
        let mask = Mask::new(child);
        self.or_with_mask(mask);
        *self
    }
    pub fn remove_child(&mut self, port_number: PortNumber) -> Self {
        let mask = Mask::new(port_number).not();
        self.and_with_mask(mask);
        *self
    }
    pub fn clear_children(&mut self) -> Self {
        self.and_with_mask(Mask::port0());
        *self
    }
    pub fn set_parent(&mut self, port_number: PortNumber) -> Self {
        self.parent = port_number.get_port_no();
        *self
    }
    pub fn has_child(&self, port_number: PortNumber) -> bool {
        self.get_mask().and(Mask::new(port_number)) != Mask::empty()
    }
}
impl fmt::Display for RoutingTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut uuid = self.tree_uuid.to_string();
        uuid.truncate(8);
        let mut s = format!(" {:8?}", uuid);
        if self.inuse { write!(s, "  Yes  ")?; }
        else          { write!(s, "  No   ")?; }
        if self.may_send { write!(s, "  Yes ")?; }
        else             { write!(s, "  No  ")?; }
        write!(s, "{:7}", self.parent.0)?;
        write!(s, "{}", self.mask)?;
        write!(f, "{}", s)
    }
}
// Errors
/*
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum RoutingTableEntryError {
    #[fail(display = "RoutingTableEntryError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
*/
