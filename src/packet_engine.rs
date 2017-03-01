use std::fmt;
use std::thread;
use message::{Sender, Receiver};
use name::CellID;
use routing_table::{RoutingTable, RoutingTableError};

#[derive(Debug)]
pub struct PacketEngine {
	cell_id: CellID,
	routing_table: RoutingTable,
}
impl PacketEngine {
	pub fn new(cell_id: CellID, send_to_ca: Sender, recv_from_ca: Receiver, pe_ports: Vec<(Sender,Receiver)>) -> Result<PacketEngine, PacketEngineError> {
		let routing_table = try!(RoutingTable::new()); 
		let pe = PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table };
		thread::spawn( || { PacketEngine::ca_channel(cell_id, send_to_ca, recv_from_ca) });
		Ok(pe)
	}
	fn ca_channel(cell_id: CellID, send_to_ca: Sender, recv_from_ca: Receiver) {
		println!("Packet Engine for cell {} here", cell_id);
	}
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PacketEngineError {
	RoutingTable(RoutingTableError),
}
impl Error for PacketEngineError {
	fn description(&self) -> &str {
		match *self {
			PacketEngineError::RoutingTable(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketEngineError::RoutingTable(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketEngineError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketEngineError::RoutingTable(_) => write!(f, "Cell Agent RoutingTable Error caused by"),
		}
	}
}
impl From<RoutingTableError> for PacketEngineError {
	fn from(err: RoutingTableError) -> PacketEngineError { PacketEngineError::RoutingTable(err) }
}
