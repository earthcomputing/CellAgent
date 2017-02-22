use std::fmt;
use name::{CellID};
use routingtable::{RoutingTable, RoutingTableError};
use packet_engine::PacketEngine;

#[derive(Debug, Clone)]
pub struct CellAgent {
	cell_id: CellID,
	routing_table: RoutingTable,
	packet_engine: PacketEngine,
}
impl CellAgent {
	pub fn new(cell_id: CellID) -> Result<CellAgent, CellAgentError> {
		let routing_table = try!(RoutingTable::new()); 
		Ok(CellAgent { cell_id: cell_id, routing_table: routing_table,
			packet_engine: PacketEngine::new() })
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum CellAgentError {
	Name(NameError),
	RoutingTable(RoutingTableError)
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::RoutingTable(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::RoutingTable(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Name(_) => write!(f, "Cell Agent Name Error caused by"),
			CellAgentError::RoutingTable(_) => write!(f, "Cell Agent Routing Table Error caused by"),
		}
	}
}
impl From<NameError> for CellAgentError {
	fn from(err: NameError) -> CellAgentError { CellAgentError::Name(err) }
}
impl From<RoutingTableError> for CellAgentError {
	fn from(err: RoutingTableError) -> CellAgentError { CellAgentError::RoutingTable(err) }
}
