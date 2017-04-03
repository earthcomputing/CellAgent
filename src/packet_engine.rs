use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::sync::mpsc::channel;
use crossbeam::Scope;
use nalcell::{EntryPeFromCa, PacketSend, PacketRecv, PacketSendError, PacketCaToPe, PacketPeFromCa, 
	PacketPeToCa, PacketPeCaSendError, PacketPeFromPort};
use name::CellID;
use packet::Packet;
use routing_table::{RoutingTable, RoutingTableError};
use utility::{ints_from_mask, UtilityError};

#[derive(Debug)]
pub struct PacketEngine {
	cell_id: CellID,
	routing_table: Arc<Mutex<RoutingTable>>,
	packet_pe_to_ports: Vec<PacketSend>,
}
impl PacketEngine {
	pub fn new(scope: &Scope, cell_id: &CellID, packet_pe_to_ca: PacketPeToCa, 
		packet_pe_from_ca: PacketPeFromCa, recv_entry_from_ca: EntryPeFromCa, 
		packet_pe_from_ports: PacketPeFromPort, packet_pe_to_ports: Vec<PacketSend>) -> Result<PacketEngine, PacketEngineError> {
		let routing_table = Arc::new(Mutex::new(try!(RoutingTable::new()))); 
		let pe = PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			packet_pe_to_ports: packet_pe_to_ports };
		try!(pe.entry_channel(scope, recv_entry_from_ca));
		try!(pe.ca_channel(scope, packet_pe_from_ca));
		try!(pe.port_channel(scope, packet_pe_from_ports, packet_pe_to_ca));
		Ok(pe)
	}
	fn ca_channel(&self, scope: &Scope,  packet_pe_from_ca: PacketPeFromCa) -> Result<(), PacketEngineError> {
		let table = self.routing_table.clone();
		let packet_pe_to_ports = self.packet_pe_to_ports.clone();
		scope.spawn( move || -> Result<(), PacketEngineError> {
			loop {
				let (index, mask, packet) = try!(packet_pe_from_ca.recv());
				let unlocked = table.lock().unwrap();
				let entry = (*unlocked).get_entry(index);
				let entry_mask = entry.get_mask();
				let port_nos = try!(ints_from_mask(entry_mask & mask));
				for port_no in port_nos.iter() {
					let sender = packet_pe_to_ports.get(*port_no as usize);
					match sender {
						Some(s) => try!(s.send(packet)),
						None => return Err(PacketEngineError::Port(PortError::new(*port_no)))
					};
				}
			}
		});
		Ok(())
	}
	fn port_channel(&self, scope: &Scope, packet_pe_from_ports: PacketPeFromPort, 
			packet_pe_to_ca: PacketPeToCa) -> Result<(),PacketEngineError> {
		let cell_id = self.cell_id.clone();
		scope.spawn( move || -> Result<(), PacketEngineError> {
			loop {
				let (port_no, packet) = try!(packet_pe_from_ports.recv());
				let index = packet.get_header().get_other_index();
				try!(packet_pe_to_ca.send((port_no, index, packet)));
			}
		});
		Ok(())
	}
	pub fn entry_channel(&self, scope: &Scope, entry_pe_from_ca: EntryPeFromCa) -> Result<(),PacketEngineError> {
		let table = self.routing_table.clone();
		let cell_id = self.cell_id.clone(); // Debug only
		scope.spawn( move || -> Result<(), PacketEngineError> {
			loop { 
				let entry = try!(entry_pe_from_ca.recv());
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
	Send(PacketSendError),
	SendToCa(PacketPeCaSendError),
	Recv(mpsc::RecvError),
}
impl Error for PacketEngineError {
	fn description(&self) -> &str {
		match *self {
			PacketEngineError::RoutingTable(ref err) => err.description(),
			PacketEngineError::Utility(ref err) => err.description(),
			PacketEngineError::Port(ref err) => err.description(),
			PacketEngineError::SendToCa(ref err) => err.description(),
			PacketEngineError::Send(ref err) => err.description(),
			PacketEngineError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketEngineError::RoutingTable(ref err) => Some(err),
			PacketEngineError::Utility(ref err) => Some(err),
			PacketEngineError::Port(ref err) => Some(err),
			PacketEngineError::SendToCa(ref err) => Some(err),
			PacketEngineError::Send(ref err) => Some(err),
			PacketEngineError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketEngineError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketEngineError::RoutingTable(ref err) => write!(f, "Packet Engine Routing Table Error caused by {}", err),
			PacketEngineError::Utility(ref err) => write!(f, "Packet Engine Utility Error caused by {}", err),
			PacketEngineError::Port(ref err) => write!(f, "Packet Engine Port Error caused by {}", err),
			PacketEngineError::SendToCa(ref err) => write!(f, "Packet Engine Send packet to Cell Agent Error caused by {}", err),
			PacketEngineError::Send(ref err) => write!(f, "Packet Engine Send packet to port Error caused by {}", err),
			PacketEngineError::Recv(ref err) => write!(f, "Packet Engine Receive Error caused by {}", err),
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
impl From<mpsc::SendError<(u8,u32,Packet)>> for PacketEngineError {
	fn from(err: mpsc::SendError<(u8,u32,Packet)>) -> PacketEngineError { PacketEngineError::SendToCa(err) }
}
impl From<UtilityError> for PacketEngineError {
	fn from(err: UtilityError) -> PacketEngineError { PacketEngineError::Utility(err) }
}