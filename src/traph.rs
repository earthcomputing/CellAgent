use std::fmt;
use std::collections::HashMap;
use name::{TreeID,PortID};
use port::Port;

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	table_index: usize,
	elements: HashMap<PortID, TraphElement>,
}
impl Traph {
	pub fn new(tree_id: TreeID, table_index: usize) -> Traph {
		Traph { tree_id: tree_id, table_index: table_index, elements: HashMap::new() }
	}
	pub fn get_table_index(&self) -> usize { self.table_index } 
	pub fn add_element(port: Port, other_index: usize) {
		let port_id = port.get_id();
		let port_no = port.get_port_no();
		let element = TraphElement::new(port_no, other_index);
	}
}
#[derive(Debug, Copy, Clone)]
enum TraphStatus {
	Parent,
	Child,
	Pruned
}
// Errors
use std::error::Error;
use config::MAX_PORTS;
#[derive(Debug)]
pub enum TraphError {
	Port(PortError)
}
impl Error for TraphError {
	fn description(&self) -> &str {
		match *self {
			TraphError::Port(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TraphError::Port(ref err) => Some(err),
		}
	}
}
impl fmt::Display for TraphError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TraphError::Port(ref err) => write!(f, "Traph Port Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: u8) -> PortError {
		PortError { msg: format!("Port number {} is greater than the maximum of {}", port_no, MAX_PORTS) }
	}
}
impl Error for PortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PortError> for TraphError {
	fn from(err: PortError) -> TraphError { TraphError::Port(err) }
}

#[derive(Debug, Clone)]
struct TraphElement {
	port_index: u8,
	other_index: usize,
	is_connected: bool,
	is_broken: bool,
	status: TraphStatus,
	hops: usize,
	path: Option<TreeID> // or Option<PortID>
}
impl TraphElement {
	pub fn new(port_index: u8, other_index: usize) -> TraphElement {
		TraphElement { port_index: port_index, other_index: other_index, is_connected: false,
					is_broken: false, status: TraphStatus::Pruned, hops: 0, path: None } 
	}
}