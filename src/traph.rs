use std::fmt;
use std::collections::HashMap;
use config::MAX_PORTS;
use name::{TreeID, PortID, NameError};
use port::Port;

#[derive(Debug, Clone)]
pub struct Traph {
	tree_id: TreeID,
	table_index: usize,
	elements: Box<[TraphElement]>,
}
impl Traph {
	pub fn new(tree_id: TreeID, table_index: usize) -> Result<Traph, TraphError> {
		let default_id = try!(PortID::new(MAX_PORTS+1));
		let default = TraphElement::new(0, default_id, 0);
		let mut elements = Vec::new();
		for i in 0..MAX_PORTS { elements.push(default.clone()); }
		Ok(Traph { tree_id: tree_id, table_index: table_index, elements: elements.into_boxed_slice() })
	}
	pub fn get_table_index(&self) -> usize { self.table_index } 
	pub fn add_element(&mut self, port: Port, other_index: usize) -> Result<(), TraphError> {
		let port_no = port.get_port_no();
		if port_no > MAX_PORTS { return Err(TraphError::Port(PortError::new(port_no))) };
		let port_id = port.get_id();
		let element = TraphElement::new(port_no, port_id, other_index);
		self.elements[port_no as usize] = element;
		Ok(())
	}
	pub fn set_connected(&mut self, port_no: u8) -> Result<(), TraphError> { 
		self.set_connected_state(port_no, true); 
		Ok(())
	}
	pub fn set_disconnected(&mut self, port_no: u8) -> Result<(), TraphError> { 
		self.set_connected_state(port_no, false); 
		Ok(())
	}
	fn set_connected_state(&mut self, port_no: u8, state: bool) -> Result<(),TraphError> {
		if port_no > MAX_PORTS { return Err(TraphError::Port(PortError::new(port_no))); }
		if state { self.elements[port_no as usize].set_connected(); }
		else     { self.elements[port_no as usize].set_disconnected(); }
		Ok(())
	}
}
#[derive(Debug, Copy, Clone)]
pub enum TraphStatus {
	Parent,
	Child,
	Pruned
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum TraphError {
	Name(NameError),
	Port(PortError),
	Lookup(LookupError),
}
impl Error for TraphError {
	fn description(&self) -> &str {
		match *self {
			TraphError::Name(ref err) => err.description(),
			TraphError::Port(ref err) => err.description(),
			TraphError::Lookup(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TraphError::Name(ref err) => Some(err),
			TraphError::Port(ref err) => Some(err),
			TraphError::Lookup(ref err) => Some(err),
		}
	}
}
impl fmt::Display for TraphError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TraphError::Name(ref err) => write!(f, "Traph Name Error caused by {}", err),
			TraphError::Port(ref err) => write!(f, "Traph Port Error caused by {}", err),
			TraphError::Lookup(ref err) => write!(f, "Traph Lookup Error caused by {}", err),
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
#[derive(Debug)]
pub struct LookupError { msg: String }
impl LookupError { 
	pub fn new(port_id: PortID) -> LookupError {
		LookupError { msg: format!("No traph entry for port {}", port_id) }
	}
}
impl Error for LookupError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for LookupError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<LookupError> for TraphError {
	fn from(err: LookupError) -> TraphError { TraphError::Lookup(err) }
}
impl From<NameError> for TraphError {
	fn from(err: NameError) -> TraphError { TraphError::Name(err) }
}

#[derive(Debug, Clone)]
struct TraphElement {
	port_index: u8,
	port_id: PortID,
	other_index: usize,
	is_connected: bool,
	is_broken: bool,
	status: TraphStatus,
	hops: usize,
	path: Option<TreeID> // or Option<PortID>
}
impl TraphElement {
	fn new(port_index: u8, port_id: PortID, other_index: usize) -> TraphElement {
		TraphElement { port_index: port_index, port_id: port_id, other_index: other_index, is_connected: false,
					is_broken: false, status: TraphStatus::Pruned, hops: 0, path: None } 
	}
	fn set_connected(&mut self) { self.is_connected = true; }
	fn set_disconnected(&mut self) { self.is_connected = false; }
	
}