use std::{fmt, fmt::Write,
          collections::HashSet};

use crate::config::{PortQty};
use crate::name::{Name, PortTreeID};
use crate::utility::{Mask, PortNo, PortNumber};
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
    pub fn new(port_tree_id: PortTreeID, inuse: bool, parent: PortNumber, mask: Mask,
            may_send: bool) -> RoutingTableEntry {
        RoutingTableEntry { tree_uuid: port_tree_id.get_uuid(), parent: parent.get_port_no(),
            may_send, inuse, mask }
    }
    pub fn is_in_use(&self) -> bool { self.inuse }
    pub fn may_send(&self) -> bool { self.may_send }
    pub fn enable_send(&mut self) { self.may_send = true; }
    pub fn disable_send(&mut self) { self.may_send = false; }
    pub fn enable_receive(&mut self) { self.mask = self.mask.or(Mask::port0()); }
    pub fn disable_receive(&mut self, no_ports: PortQty) { self.mask = self.mask.and(Mask::all_but_zero(no_ports)); }
    pub fn get_uuid(&self) -> Uuid { self.tree_uuid }
    pub fn set_uuid(&mut self, uuid: &Uuid) { self.tree_uuid = *uuid; }
    pub fn set_inuse(&mut self) { self.inuse = true; }
//  pub fn set_not_inuse(&mut self) { self.inuse = false; }
    pub fn get_parent(&self) -> PortNo { self.parent }
    pub fn get_mask(&self) -> Mask { self.mask }
    pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
    pub fn set_tree_id(&mut self, port_tree_id: PortTreeID) {
        self.tree_uuid = port_tree_id.get_uuid();
    }
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
    pub fn _has_child(&self, port_number: PortNumber) -> bool {
        self.get_mask().and(Mask::new(port_number)) != Mask::empty()
    }
    fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
    fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
}
impl Default for RoutingTableEntry { // Need may_sent = true
    fn default() -> Self { // Can't use ..Default::default() without overflowing stack
        RoutingTableEntry::new(PortTreeID::default(), false,
                               PortNumber::default(), Mask::default(), true)
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
