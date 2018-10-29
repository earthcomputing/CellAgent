use std::fmt;
use config::MAX_PORTS;
use name::{NameError,CellID};
use port::{Port};
use vm::VirtualMachine;

use cellagent::{CellAgent};

struct NalCell {
	id: CellID,
	ports: Vec<Port>,
	cell_agent: CellAgent,
	vms: Vec<VirtualMachine>,
}
impl NalCell {
	fn new(id: &str, nports: u8) -> Result<NalCell,NalCellError> {
		let cell_id = try!(CellID::new(id));
		let mut ports = Vec::new();
		for i in 0..MAX_PORTS + 1 {
			ports.push(try!(Port::new(cell_id, i as u8)));
		}
		Ok(NalCell { id: cell_id, ports: ports, cell_agent: CellAgent::new(), vms: Vec::new() })
	}
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum NalCellError {
	Name(NameError),
	Quota(QuotaError)
}
impl Error for NalCellError {
	fn description(&self) -> &str {
		match *self {
			NalCellError::Quota(ref err) => err.description(),
			NalCellError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			NalCellError::Quota(_) => None,
			NalCellError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for NalCellError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NalCellError::Quota(ref err) => write!(f, "NalCell Quota Error: {}", err),
			NalCellError::Name(_) => write!(f, "NalCell Name Error caused by")
		}
	}
}
#[derive(Debug)]
pub struct QuotaError { msg: String }
impl QuotaError { 
	pub fn new(n: usize, available: usize) -> QuotaError {
		QuotaError { msg: format!("You asked for {} ports, but only {} are available", n, available) }
	}
}
impl Error for QuotaError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for QuotaError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<QuotaError> for NalCellError {
	fn from(err: QuotaError) -> NalCellError { NalCellError::Quota(err) }
}
impl From<NameError> for NalCellError {
	fn from(err: NameError) -> NalCellError { NalCellError::Name(err) }
}
