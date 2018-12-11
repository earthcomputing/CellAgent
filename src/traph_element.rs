use std::fmt;

use crate::config::{CellNo, PathLength, PortNo};
use crate::traph::{PortStatus};
use crate::utility::{Path, PortNumber};

#[derive(Debug, Copy, Clone)]
pub struct TraphElement {
    port_no: PortNo,
    is_connected: bool,
    is_broken: bool,
    status: PortStatus,
    hops: PathLength,
    path: Path,
}
impl TraphElement {
    pub fn new(is_connected: bool, port_no: PortNo,
            status: PortStatus, hops: PathLength, path: Path) -> TraphElement {
        let _f = "new";
        TraphElement { port_no,  is_connected, is_broken: false, status, hops, path }
    }
    pub fn default(port_number: PortNumber) -> TraphElement {
        let _f = "default";
        let port_no = port_number.get_port_no();
        TraphElement::new(false, port_no, PortStatus::Pruned,
                    PathLength(CellNo(0)), Path::new0())
    }
    pub fn get_port_no(&self) -> PortNo { self.port_no }
    pub fn get_hops(&self) -> PathLength { self.hops }
    pub fn hops_plus_one(&self) -> PathLength { PathLength(CellNo((self.hops.0).0 + 1)) }
    pub fn get_path(&self) -> Path { self.path }
    pub fn get_status(&self) -> PortStatus { self.status }
    pub fn is_status(&self, status: PortStatus) -> bool { self.status == status }
    pub fn is_connected(&self) -> bool { self.is_connected }
    pub fn is_broken(&self) -> bool { self.is_broken }
    pub fn set_broken(&mut self) { self.is_broken = true; }
//  pub fn set_connected(&mut self) { self.is_connected = true; }
//  pub fn set_disconnected(&mut self) { self.is_connected = false; }
    fn set_status(&mut self, status: PortStatus) { self.status = status; }
    pub fn is_on_broken_path(&self, broken_path: Path) -> bool { self.path == broken_path }
    pub fn mark_parent(&mut self) { self.set_status(PortStatus::Parent) }
    pub fn mark_child(&mut self)  { self.set_status(PortStatus::Child) }
    pub fn mark_pruned(&mut self) { self.set_status(PortStatus::Pruned) }
    pub fn mark_broken(&mut self) { self.set_status(PortStatus::Broken) }
}
impl fmt::Display for TraphElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{:5} {:9} {:6} {:6} {:4} {:4}",
             (*self.port_no), self.is_connected, self.is_broken, self.status, (self.hops.0).0, *self.path.get_port_no());
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
