use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use crossbeam::Scope;
use config::{PortNo};
use nalcell::{PeFromCa, PeToCa, PeToPort, PeFromPort};
use name::CellID;
use packet::{Packet};
use routing_table::{RoutingTable};
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber};

#[derive(Debug, Clone)]
pub struct PacketEngine {
	cell_id: CellID,
	routing_table: Arc<Mutex<RoutingTable>>,
	pe_to_ca: PeToCa,
	pe_to_ports: Vec<PeToPort>,
}
#[deny(unused_must_use)]
impl PacketEngine {
	pub fn new(scope: &Scope, cell_id: &CellID, packet_pe_to_ca: PeToCa, 
		pe_from_ca: PeFromCa, pe_from_ports: PeFromPort, pe_to_ports: Vec<PeToPort>) 
				-> Result<PacketEngine, PacketEngineError> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone())?)); 
		let pe = PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			pe_to_ca: packet_pe_to_ca, pe_to_ports: pe_to_ports };
		pe.ca_channel(scope, pe_from_ca)?;
		pe.port_channel(scope, pe_from_ports)?;
		Ok(pe)
	}
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, mask: Mask, packet: Packet) 
			-> Result<(), PacketEngineError>{
		let mut header = packet.get_header();
		let parent = entry.get_parent();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let mask = mask.and(entry.get_mask());
		let other_indices = entry.get_other_indices();
		PortNumber::new(recv_port_no, other_indices.len() as u8)?;
		if header.is_rootcast() {
			if let Some(other_index) = other_indices.get(parent as usize) {
				header.set_other_index(*other_index);
				if parent == 0 {
					self.pe_to_ca.send((None,Some((recv_port_no, *other_index, packet))))?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent as usize) {
						sender.send(packet)?;
						//println!("PacketEngine {}: sent packet {} rootward on port {}", self.cell_id, packet.get_packet_count(), parent);
					} else {
						let max_ports = self.pe_to_ports.len() as u8;
						return Err(PacketEngineError::PortNumber(PortNumberError::new(parent, max_ports)));
					}
				}
			}
		} else {
			let port_nos = mask.get_port_nos();
			//println!("PacketEngine {}: forwarding packet {} on ports {:?}", self.cell_id, packet.get_count(), port_nos);
			for port_no in port_nos.iter() {
				let other_index = *other_indices.get(*port_no as usize).expect("PacketEngine: No such other index");
				header.set_other_index(other_index as u32);
				if *port_no as usize == 0 { 
					self.pe_to_ca.send((None,Some((recv_port_no, other_index, packet))))?;
				} else {
					match self.pe_to_ports.get(*port_no as usize) {
						Some(s) => s.send(packet)?,
						None => return Err(PacketEngineError::Port(PortError::new(*port_no)))
					};
					//println!("Packet Engine {} sent packet {} to port {}", cell_id, packet_count, port_no);
				}
			}
		}
		Ok(())
	}
	pub fn ca_channel(&self, scope: &Scope, entry_pe_from_ca: PeFromCa) -> Result<(),PacketEngineError> {
		let pe = self.clone();
		scope.spawn( move || -> Result<(), PacketEngineError> {
			match pe.listen_ca(entry_pe_from_ca) {
				Ok(_) => Ok(()),
				Err(err) => {
					println!("--- PacketEngine {}: ca_channel {}", pe.cell_id, err);
					Err(err)
				}
			}
		});
		Ok(())
	}
	fn port_channel(&self, scope: &Scope, pe_from_ports: PeFromPort) -> Result<(),PacketEngineError> {
		//let cell_id = self.cell_id.clone();
		let pe = self.clone();
		scope.spawn( move || -> Result<(), PacketEngineError> {
			match pe.listen_port(pe_from_ports) {
				Ok(_) => Ok(()),
				Err(err) => {
					println!("--- PacketEngine {}: port_channel {}", pe.cell_id, err);
					Err(err)
				}
			}
		});
		Ok(())
	}
	fn listen_ca(&self, entry_pe_from_ca: PeFromCa) -> Result<(), PacketEngineError> {
		loop { 
			let (opt_entry,opt_packet) = entry_pe_from_ca.recv()?; 
			let entry = match opt_entry {
				Some(e) => {
					self.routing_table.lock().unwrap().set_entry(e);
					//println!("PacketEngine {}: {}", pe.cell_id, e);
					e
				},
				None => match opt_packet {
					Some((index, _, _)) => self.routing_table.lock().unwrap().get_entry(index)?,
					None => panic!("entry and packet empty")
				}
			};
			match opt_packet {
				Some((_, mask, packet)) => {
					//println!("PacketEngine {}: received packet {} from ca", self.cell_id, packet.get_count());
					let port_no = 0 as PortNo;
					self.forward(port_no, entry, mask, packet)?;
					//let ports = mask.get_port_nos();
					//println!("PacketEngine {}: send discover to ports {:?}", pe.cell_id, ports);
				}
				None => ()				
			}
		}		
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<(),PacketEngineError> {
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			let (opt_status, opt_packet) = match pe_from_ports.recv() {
				Ok((s,p)) => (s,p),
				Err(err) => {
					println!("PacketEngine {}: packet channel error {}", self.cell_id, err);
					return Err(PacketEngineError::Recv(err));
				}
			};
			match opt_status {
				Some(status) => {
					self.pe_to_ca.send((Some(status),None))?
				},
				None => {
					match opt_packet {
						Some((port_no, packet)) => {
							//println!("PacketEngine {}: received packet {} from port {}", self.cell_id, packet.get_count(), port_no);
							self.process_packet(port_no, packet)?
						},
						None => println!("PacketEngine {}: Empty message", self.cell_id)
					};
				}
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, packet: Packet) -> Result<(),PacketEngineError> {
		//println!("PacketEngine {}: received packet {} on port {}", cell_id, packet_count, recv_port_no);
		let header = packet.get_header();
		let index = header.get_other_index();
		let entry;
		{
			entry = self.routing_table.lock().unwrap().get_entry(index)?;
		}
		let mask = entry.get_mask();
		let other_indices = entry.get_other_indices();
		// Verify that port_no is valid
		PortNumber::new(port_no, other_indices.len() as u8)?;
		self.forward(port_no, entry, mask, packet)?;	
		Ok(())	
	}
}
impl fmt::Display for PacketEngine {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nPacket Engine");
		s = s + &format!("{}", *self.routing_table.lock().unwrap());
		write!(f, "{}", s) }	
}
// Errors
use std::error::Error;
use nalcell::{PePortError, PeCaError};
use routing_table::{RoutingTableError};
use utility::{PortNumberError, UtilityError};
#[derive(Debug)]
pub enum PacketEngineError {
	RoutingTable(RoutingTableError),
	Utility(UtilityError),
	Port(PortError),
	PortNumber(PortNumberError),
	Send(PePortError),
	SendToCa(PeCaError),
	Recv(mpsc::RecvError),
}
impl Error for PacketEngineError {
	fn description(&self) -> &str {
		match *self {
			PacketEngineError::RoutingTable(ref err) => err.description(),
			PacketEngineError::Utility(ref err) => err.description(),
			PacketEngineError::Port(ref err) => err.description(),
			PacketEngineError::PortNumber(ref err) => err.description(),
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
			PacketEngineError::PortNumber(ref err) => Some(err),
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
			PacketEngineError::PortNumber(ref err) => write!(f, "Packet Engine Port Number Error caused by {}", err),
			PacketEngineError::SendToCa(ref err) => write!(f, "Packet Engine Send packet to Cell Agent Error caused by {}", err),
			PacketEngineError::Send(ref err) => write!(f, "Packet Engine Send packet to port Error caused by {}", err),
			PacketEngineError::Recv(ref err) => write!(f, "Packet Engine Receive Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: PortNo) -> PortError {
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
impl From<PortNumberError> for PacketEngineError {
	fn from(err: PortNumberError) -> PacketEngineError { PacketEngineError::PortNumber(err) }
}
impl From<RoutingTableError> for PacketEngineError {
	fn from(err: RoutingTableError) -> PacketEngineError { PacketEngineError::RoutingTable(err) }
}
impl From<mpsc::RecvError> for PacketEngineError {
	fn from(err: mpsc::RecvError) -> PacketEngineError { PacketEngineError::Recv(err) }
}
impl From<PePortError> for PacketEngineError {
	fn from(err: PePortError) -> PacketEngineError { PacketEngineError::Send(err) }
}
impl From<PeCaError> for PacketEngineError {
	fn from(err: PeCaError) -> PacketEngineError { PacketEngineError::SendToCa(err) }
}
impl From<UtilityError> for PacketEngineError {
	fn from(err: UtilityError) -> PacketEngineError { PacketEngineError::Utility(err) }
}