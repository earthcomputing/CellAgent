use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use crossbeam::Scope;
use cellagent::{SendPacketCaToPe, ReceivePacketPeFromCa};
use nalcell::{EntryReceiver};
use name::CellID;
use packet::Packet;
use routing_table::{RoutingTable, RoutingTableError};
use utility::{ints_from_mask, UtilityError};

pub type SendPacket = mpsc::Sender<Packet>;
pub type ReceivePacket = mpsc::Receiver<Packet>;
pub type SendPacketError = mpsc::SendError<Packet>;

#[derive(Debug, Clone)]
pub struct PacketEngine {
	cell_id: CellID,
	routing_table: Arc<Mutex<RoutingTable>>,
}
impl PacketEngine {
	pub fn new(scope: &Scope, cell_id: &CellID, send_to_ca: SendPacketCaToPe, recv_from_ca: ReceivePacketPeFromCa, 
		recv_from_port: ReceivePacket, send_to_ports: Vec<SendPacket>, 
		recv_entry_from_ca: EntryReceiver) -> Result<PacketEngine, PacketEngineError> {
		let routing_table = Arc::new(Mutex::new(try!(RoutingTable::new()))); 
		let pe = PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table };
		try!(pe.entry_channel(scope, recv_entry_from_ca));
		pe.ca_channel(scope, send_to_ca, recv_from_ca, send_to_ports);
		Ok(pe)
	}
	fn ca_channel(&self, scope: &Scope, send_to_ca: SendPacketCaToPe, recv_from_ca: ReceivePacketPeFromCa,
			send_to_ports: Vec<SendPacket>) {
		let table = self.routing_table.clone();
		scope.spawn( move || -> Result<(), PacketEngineError> {
				loop {
					let (index, mask, packet) = try!(recv_from_ca.recv());
					let unlocked = table.lock().unwrap();
					let entry = (*unlocked).get_entry(index);
					let entry_mask = entry.get_mask();
					let port_nos = try!(ints_from_mask(entry_mask & mask));
					for port_no in port_nos.iter() {
						let sender = send_to_ports.get(*port_no as usize);
						match sender {
							Some(s) => try!(s.send(packet)),
							None => return Err(PacketEngineError::Port(PortError::new(*port_no)))
						};
					}
				}
				Ok(())
			}
		);
	}
	pub fn entry_channel(&self, scope: &Scope, recv_entry_from_ca: EntryReceiver) -> Result<(),PacketEngineError> {
		let table = self.routing_table.clone();
		let cell_id = self.cell_id.clone(); // Debug only
		scope.spawn( move || -> Result<(), PacketEngineError> {
			loop { 
				let entry = try!(recv_entry_from_ca.recv());
				table.lock().unwrap().set_entry(entry);
			}
			Ok(())
		});
		Ok(())
	}
	pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	pub fn stringify(&self) -> String {
		let mut s = format!("\nPacket Engine");
		let mut s = s + &self.routing_table.lock().unwrap().stringify();
		s
	}
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PacketEngineError {
	RoutingTable(RoutingTableError),
	Utility(UtilityError),
	Port(PortError),
	Send(SendPacketError),
	Recv(mpsc::RecvError),
}
impl Error for PacketEngineError {
	fn description(&self) -> &str {
		match *self {
			PacketEngineError::RoutingTable(ref err) => err.description(),
			PacketEngineError::Utility(ref err) => err.description(),
			PacketEngineError::Port(ref err) => err.description(),
			PacketEngineError::Send(ref err) => err.description(),
			PacketEngineError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketEngineError::RoutingTable(ref err) => Some(err),
			PacketEngineError::Utility(ref err) => Some(err),
			PacketEngineError::Port(ref err) => Some(err),
			PacketEngineError::Send(ref err) => Some(err),
			PacketEngineError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketEngineError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketEngineError::RoutingTable(ref err) => write!(f, "Cell Agent Routing Table Error caused by {}", err),
			PacketEngineError::Utility(ref err) => write!(f, "Cell Agent Utility Error caused by {}", err),
			PacketEngineError::Port(ref err) => write!(f, "Cell Agent Port Error caused by {}", err),
			PacketEngineError::Send(ref err) => write!(f, "Cell Agent Send Error caused by {}", err),
			PacketEngineError::Recv(ref err) => write!(f, "Cell Agent Receive Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: u8) -> PortError {
		PortError { msg: format!("No sender for port {}", port_no) }
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
impl From<PortError> for PacketEngineError {
	fn from(err: PortError) -> PacketEngineError { PacketEngineError::Port(err) }
}
impl From<RoutingTableError> for PacketEngineError {
	fn from(err: RoutingTableError) -> PacketEngineError { PacketEngineError::RoutingTable(err) }
}
impl From<mpsc::RecvError> for PacketEngineError {
	fn from(err: mpsc::RecvError) -> PacketEngineError { PacketEngineError::Recv(err) }
}
impl From<mpsc::SendError<Packet>> for PacketEngineError {
	fn from(err: mpsc::SendError<Packet>) -> PacketEngineError { PacketEngineError::Send(err) }
}
impl From<UtilityError> for PacketEngineError {
	fn from(err: UtilityError) -> PacketEngineError { PacketEngineError::Utility(err) }
}