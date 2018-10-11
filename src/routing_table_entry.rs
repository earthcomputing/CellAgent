use std::fmt;
use std::collections::HashSet;
//use uuid::Uuid;

use config::{PortNo};
use name::{Name, TreeID};
use utility::{Mask, PortNumber};
use uuid_ec::Uuid;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RoutingTableEntry {
    tree_uuid: Uuid,
    may_send: bool, // TODO: Move this from here to Tree
    inuse: bool,
    parent: PortNo,
    mask: Mask,
}
impl RoutingTableEntry {
    pub fn new(tree_id: &TreeID, inuse: bool, parent: PortNumber, mask: Mask,
            may_send: bool) -> RoutingTableEntry {
        RoutingTableEntry { tree_uuid: tree_id.get_uuid(), parent: parent.get_port_no(),
            may_send, inuse, mask }
    }
    pub fn default() -> RoutingTableEntry {
        let port_number = PortNumber::new0();
        let tree_id = TreeID::new("default").expect("The string 'default' is always a valid tree name")
;		RoutingTableEntry::new(&tree_id, false, port_number, Mask::empty(), true)
    }
    pub fn is_in_use(&self) -> bool { self.inuse }
    pub fn may_send(&self) -> bool { self.may_send }
//    pub fn may_receive(&self) -> bool { !self.mask.and(Mask::port0()).equal(Mask::empty()) }
    pub fn enable_send(&mut self) { self.may_send = true; }
    pub fn disable_send(&mut self) { self.may_send = false; }
//	pub fn is_on_tree(&self) -> bool {
//        self.may_send || self.may_receive()
//    }
    pub fn get_uuid(&self) -> Uuid { self.tree_uuid }
    pub fn set_uuid(&mut self, uuid: &Uuid) { self.tree_uuid = *uuid; }
    pub fn or_with_mask(&mut self, mask: Mask) { self.mask = self.mask.or(mask); }
    pub fn and_with_mask(&mut self, mask: Mask) { self.mask = self.mask.and(mask); }
    pub fn set_inuse(&mut self) { self.inuse = true; }
//	pub fn set_not_inuse(&mut self) { self.inuse = false; }
    pub fn get_parent(&self) -> PortNo { self.parent }
    pub fn get_mask(&self) -> Mask { self.mask }
    pub fn set_mask(&mut self, mask: Mask) { self.mask = mask; }
    pub fn set_tree_id(&mut self, tree_id: &TreeID) {
        self.tree_uuid = tree_id.get_uuid();
    }
//	pub fn get_other_index(&self, port_number: PortNumber) -> TableIndex {
//		let port_no = port_number.get_port_no().v as usize;
//		self.other_indices[port_no]
//	}
    pub fn add_children(&mut self, children: &HashSet<PortNumber>) {
        let mask = Mask::make(children);
        self.or_with_mask(mask);
    }
    pub fn clear_children(&mut self) {
        self.and_with_mask(Mask::port0())
    }
    pub fn set_parent(&mut self, port_number: PortNumber) {
        self.parent = port_number.get_port_no();
    }

}
impl fmt::Display for RoutingTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut uuid = self.tree_uuid.to_string();
        uuid.truncate(8);
        let mut s = format!(" {:8?}", uuid);
        if self.inuse { s = s + &format!("  Yes  "); }
        else          { s = s + &format!("  No   "); }
        if self.may_send { s = s + &format!("  Yes "); }
        else             { s = s + &format!("  No  "); }
        s = s + &format!("{:7}", self.parent.0);
        s = s + &format!("{}", self.mask);
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