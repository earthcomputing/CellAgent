use std::fmt;
use config::MAX_PORTS;
use name::{CellID};
use port::{Port};
use vm::VirtualMachine;
use cellagent::{CellAgent, CellAgentError};

#[derive(Debug)]
pub struct NalCell {
	id: CellID,
	cell_no: usize,
	is_border: bool,
	ports: Vec<Port>,
	cell_agent: CellAgent,
	vms: Vec<VirtualMachine>,
}
impl NalCell {
	pub fn new(cell_no: usize, nports: u8, is_border: bool) -> Result<NalCell,NalCellError> {
		let id = &format!("C:{}", cell_no);
		let cell_id = try!(CellID::new(id));
		let mut ports = Vec::new();
		let mut is_border_port;
		for i in 0..nports + 1 {
			if is_border & (i == 2) { is_border_port = true; }
			else                    { is_border_port = false; }
			ports.push(try!(Port::new(&cell_id, i as u8, is_border_port)));
		}
		ports[0].set_connected(None, None);
		let cell_agent = try!(CellAgent::new(cell_id));
		Ok(NalCell { id: cell_id, cell_no: cell_no, ports: ports, is_border: is_border,
				cell_agent: cell_agent, vms: Vec::new(), })
	}
	pub fn get_id(&self) -> CellID { self.id }
	pub fn get_cell_no(&self) -> usize { self.cell_no }
	pub fn get_port(&mut self, index: u8) -> &mut Port { &mut self.ports[index as usize] }
	pub fn get_free_port_mut (&mut self) -> Result<&mut Port,NalCellError> {
		for p in &mut self.ports {
			if !p.is_connected() & !p.is_border() { return Ok(p); }
		}
		Err(NalCellError::NoFreePort(NoFreePortError::new(self.id)))
	}
	pub fn to_string(&self) -> String {
		let mut s = String::new();
		if self.is_border { s = s + &format!("Border Cell {}", self.id); }
		else              { s = s + &format!("Cell {}", self.id); }
		for p in &self.ports {
			if p.get_port_no() < 4 { s = s + "\n" + &format!("{}", p); }
		}
		s
	}
}
impl fmt::Display for NalCell { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum NalCellError {
	Name(NameError),
	NoFreePort(NoFreePortError),
	CellAgent(CellAgentError)
}
impl Error for NalCellError {
	fn description(&self) -> &str {
		match *self {
			NalCellError::NoFreePort(ref err) => err.description(),
			NalCellError::Name(ref err) => err.description(),
			NalCellError::CellAgent(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			NalCellError::NoFreePort(_) => None,
			NalCellError::Name(ref err) => Some(err),
			NalCellError::CellAgent(ref err) => Some(err),
		}
	}
}
impl fmt::Display for NalCellError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NalCellError::NoFreePort(ref err) => write!(f, "NalCell NoFreePort Error caused by {}", err),
			NalCellError::Name(ref err) => write!(f, "NalCell Name Error caused by {}", err),
			NalCellError::CellAgent(ref err) => write!(f, "NalCell Cell Agent Error caused by {}", err),
		}
	}
}
impl From<NameError> for NalCellError {
	fn from(err: NameError) -> NalCellError { NalCellError::Name(err) }
}
impl From<CellAgentError> for NalCellError {
	fn from(err: CellAgentError) -> NalCellError { NalCellError::CellAgent(err) }
}
#[derive(Debug)]
pub struct NoFreePortError { msg: String }
impl NoFreePortError { 
	pub fn new(cell_id: CellID) -> NoFreePortError {
		NoFreePortError { msg: format!("All ports have been assigned for cell {}", cell_id) }
	}
}
impl Error for NoFreePortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NoFreePortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NoFreePortError> for NalCellError {
	fn from(err: NoFreePortError) -> NalCellError { NalCellError::NoFreePort(err) }
}
