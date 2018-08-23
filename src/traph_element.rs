use std::fmt;

use config::{CellNo, PathLength, PortNo};
use traph::{PortStatus};
use utility::{Path, PortNumber};

#[derive(Debug, Copy, Clone)]
pub struct TraphElement {
	port_no: PortNo,
	is_connected: bool,
	is_broken: bool,
	status: PortStatus,
	hops: PathLength,
	path: Option<Path>
}
impl TraphElement {
	pub fn new(is_connected: bool, port_no: PortNo,
			status: PortStatus, hops: PathLength, path: Option<Path>) -> TraphElement {
		TraphElement { port_no,  is_connected, is_broken: false, status,
			hops, path }
	}
	pub fn default(port_number: PortNumber) -> TraphElement {
		let port_no = port_number.get_port_no();
		TraphElement::new(false, port_no, PortStatus::Pruned,
					PathLength(CellNo(0)), None)
	}
//	pub fn get_port_no(&self) -> PortNo { self.port_no }
	pub fn get_hops(&self) -> PathLength { self.hops }
	pub fn hops_plus_one(&self) -> PathLength { PathLength(CellNo((self.hops.0).0 + 1)) }
	pub fn get_path(&self) -> Option<Path> { self.path }
	pub fn get_status(&self) -> PortStatus { self.status }
//	pub fn get_other_index(&self) -> TableIndex { self.other_index }
	pub fn is_connected(&self) -> bool { self.is_connected }
//	pub fn set_connected(&mut self) { self.is_connected = true; }
//	pub fn set_disconnected(&mut self) { self.is_connected = false; }
//	pub fn set_status(&mut self, status: PortStatus) { self.status = status; }
}
impl fmt::Display for TraphElement {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("{:5} {:9} {:6} {:6} {:4}",
							(*self.port_no), self.is_connected, self.is_broken, self.status, (self.hops.0).0);
        s = s + &match self.path {
            Some(p) => format!(" {:4}", (*p.get_port_no())),
            None => format!(" None")
        };
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