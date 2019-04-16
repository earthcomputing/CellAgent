use std::fmt;

use crate::config::{CellQty, PathLength, PortNo};
use crate::traph::{PortState};
use crate::utility::{Path, PortNumber};

#[derive(Debug, Copy, Clone)]
pub struct TraphElement {
    port_no: PortNo,
    is_connected: bool,
    is_broken: bool,
    state: PortState,
    hops: PathLength,
    path: Path,
}
impl TraphElement {
    pub fn new(is_connected: bool, port_no: PortNo,
               status: PortState, hops: PathLength, path: Path) -> TraphElement {
        let _f = "new";
        TraphElement { port_no,  is_connected, is_broken: false, state: status, hops, path }
    }
    pub fn default(port_number: PortNumber) -> TraphElement {
        let _f = "default";
        let port_no = port_number.get_port_no();
        TraphElement::new(false, port_no, PortState::Unknown,
                          PathLength(CellQty(0)), Path::new0())
    }
    pub fn get_port_no(&self) -> PortNo { self.port_no }
    pub fn get_hops(&self) -> PathLength { self.hops }
    pub fn hops_plus_one(&self) -> PathLength { PathLength(CellQty((self.hops.0).0 + 1)) }
    pub fn get_path(&self) -> Path { self.path }
    pub fn get_state(&self) -> PortState { self.state }
    pub fn is_state(&self, state: PortState) -> bool { self.state == state }
    pub fn is_connected(&self) -> bool { self.is_connected }
    pub fn is_broken(&self) -> bool { self.is_broken }
    pub fn set_broken(&mut self) { self.is_broken = true; }
    pub fn set_connected(&mut self) { self.is_connected = true; }
//  pub fn set_disconnected(&mut self) { self.is_connected = false; }
    fn set_state(&mut self, state: PortState) { self.state = state; }
    pub fn is_on_broken_path(&self, broken_path: Path) -> bool { self.path == broken_path }
    pub fn mark_parent(&mut self) { self.set_state(PortState::Parent) }
    pub fn _mark_child(&mut self)  { self.set_state(PortState::Child) }
    pub fn mark_pruned(&mut self) { self.set_state(PortState::Pruned) }
    pub fn mark_broken(&mut self) { self.set_state(PortState::Broken) }
}
impl fmt::Display for TraphElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{:5} {:9} {:6} {:6} {:4} {:4}",
                        (*self.port_no), self.is_connected, self.is_broken, self.state, (self.hops.0).0, *self.path.get_port_no());
        write!(f, "{}", s)
    }
}
// Errors
/*
#[derive(Debug, Fail)]
pub enum TraphElementError {
    #[fail(display = "TraphElementError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
*/
