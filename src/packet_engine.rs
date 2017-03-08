use std::fmt;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::RecvError;
use crossbeam::Scope;
use message::{Sender, Receiver};
use nalcell::{EntryReceiver};
use name::CellID;
use routing_table::{RoutingTable, RoutingTableError};
use routing_table_entry::RoutingTableEntry;

#[derive(Debug, Clone)]
pub struct PacketEngine {
	cell_id: CellID,
	routing_table: Arc<Mutex<RoutingTable>>,
}
impl PacketEngine {
	pub fn new(scope: &Scope, cell_id: CellID, send_to_ca: Sender, recv_from_ca: Receiver, recv_from_port: Receiver, 
				send_to_ports: Vec<Sender>, recv_entry_from_ca: EntryReceiver) -> Result<PacketEngine, PacketEngineError> {
		let routing_table = Arc::new(Mutex::new(try!(RoutingTable::new()))); 
		PacketEngine::entry_channel(scope, &cell_id, &routing_table, recv_entry_from_ca);
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table })
	}
	fn ca_channel(scope: Scope, cell_id: &CellID, send_to_ca: Sender, recv_from_ca: Receiver) {
		println!("Packet Engine for cell {} here", cell_id);
	}
	pub fn entry_channel(scope: &Scope, cell_id: &CellID, routing_table: &Arc<Mutex<RoutingTable>>, recv_entry_from_ca: EntryReceiver) -> Result<(),PacketEngineError> {
		println!("Packet Engine entry receiver for cell {}", cell_id);
		scope.spawn( move || -> Result<(), PacketEngineError> {
			loop { 
				let entry = match recv_entry_from_ca.recv() {
					Ok(e) => { println!("received entry {}", e); e },
					Err(err) => {
						println!("Receive error {} in entry_channel", err);
						return Err(PacketEngineError::Receive(err))
					}
				};
				//self.routing_table.lock().unwrap().set_entry(entry);
			}
			Ok(())
		});
		println!("Return from entry_channel");
		Ok(())
	}
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PacketEngineError {
	RoutingTable(RoutingTableError),
	Receive(RecvError),
}
impl Error for PacketEngineError {
	fn description(&self) -> &str {
		match *self {
			PacketEngineError::RoutingTable(ref err) => err.description(),
			PacketEngineError::Receive(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketEngineError::RoutingTable(ref err) => Some(err),
			PacketEngineError::Receive(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketEngineError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketEngineError::RoutingTable(ref err) => write!(f, "Cell Agent RoutingTable Error caused by {}", err),
			PacketEngineError::Receive(ref err) => write!(f, "Cell Agent RoutingTable Error caused by {}", err),
		}
	}
}
impl From<RoutingTableError> for PacketEngineError {
	fn from(err: RoutingTableError) -> PacketEngineError { PacketEngineError::RoutingTable(err) }
}
impl From<RecvError> for PacketEngineError {
	fn from(err: RecvError) -> PacketEngineError { PacketEngineError::Receive(err) }
}
