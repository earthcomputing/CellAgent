use std::fmt;
use std::thread;
use std::collections::HashMap;
use message::{Sender,Receiver};
use name::{CellID, TreeID};
use traph::Traph;

#[derive(Debug)]
pub struct CellAgent {
	cell_id: CellID,
	traphs: HashMap<TreeID,Traph>
}
impl CellAgent {
	pub fn new(cell_id: CellID, send_to_pe: Sender, recv_from_pe: Receiver) -> Result<CellAgent, CellAgentError> {
		let ca = CellAgent { cell_id: cell_id.clone(), traphs: HashMap::new() };
		thread::spawn( move || { CellAgent::work(cell_id.clone(), send_to_pe, recv_from_pe); } );
		Ok(ca)
	}
	pub fn work(cell_id: CellID, send_to_pe: Sender, recv_from_pe: Receiver) {
		println!("Cell Agent on cell {} is working", cell_id);
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum CellAgentError {
	Name(NameError),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Name(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Name(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Name(_) => write!(f, "Cell Agent Name Error caused by"),
		}
	}
}
impl From<NameError> for CellAgentError {
	fn from(err: NameError) -> CellAgentError { CellAgentError::Name(err) }
}
