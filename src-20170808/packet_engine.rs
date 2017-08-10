use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use uuid::Uuid;

use config::{PortNo, TableIndex};
use message_types::{PeFromCa, PeToCa, PeToPort, PeFromPort, CaToPePacket, PortToPePacket, PeToCaPacket};
use name::CellID;
use packet::{Packet};
use routing_table::{RoutingTable};
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber};

#[derive(Debug, Clone)]
pub struct PacketEngine {
	cell_id: CellID,
	boundary_port_nos: HashSet<PortNo>,
	routing_table: Arc<Mutex<RoutingTable>>,
	pe_to_ca: PeToCa,
	pe_to_ports: Vec<PeToPort>,
}
#[deny(unused_must_use)]
impl PacketEngine {
	pub fn new(cell_id: &CellID, packet_pe_to_ca: PeToCa, pe_to_ports: Vec<PeToPort>, 
			boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).chain_err(|| ErrorKind::PacketEngineError)?)); 
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			boundary_port_nos: boundary_port_nos, pe_to_ca: packet_pe_to_ca, pe_to_ports: pe_to_ports })
	}
	pub fn start_threads(&self, pe_from_ca: PeFromCa, pe_from_ports: PeFromPort) -> Result<()> {			
		let pe = self.clone();
		::std::thread::spawn( move ||  {
			let _ = pe.listen_ca(pe_from_ca).map_err(|e| pe.write_err(e));
		});
		let pe = self.clone();
		::std::thread::spawn( move || {
			let _ = pe.listen_port(pe_from_ports).map_err(|e| pe.write_err(e));
		});
		Ok(())
	}
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	
	fn listen_ca(&self, entry_pe_from_ca: PeFromCa) -> Result<()> {
		loop { 
			match entry_pe_from_ca.recv().chain_err(|| ErrorKind::PacketEngineError)? {
				CaToPePacket::Entry(e) => self.routing_table.lock().unwrap().set_entry(e),
				CaToPePacket::Packet((index, user_mask, packet)) => {
					//println!("PacketEngine {}: received from ca packet {}", self.cell_id, packet);					
					let entry = self.routing_table.lock().unwrap().get_entry(index).chain_err(|| ErrorKind::PacketEngineError)?;
					let port_no = 0 as PortNo;
					self.forward(port_no, entry, user_mask, packet).chain_err(|| ErrorKind::PacketEngineError)?;
				}
			}; 
		}
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<()> {
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			match pe_from_ports.recv()? {
				PortToPePacket::Status((port_no, is_border, status)) => self.pe_to_ca.send(PeToCaPacket::Status(port_no, is_border, status)).chain_err(|| ErrorKind::PacketEngineError)?,
				PortToPePacket::Packet((port_no, my_index, packet)) => self.process_packet(port_no, my_index, packet).chain_err(|| ErrorKind::PacketEngineError)?
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, my_index: TableIndex, packet: Packet) -> Result<()> {
		//println!("PacketEngine {}: received on port {} {}", self.cell_id, port_no, packet);
		let entry;
		{   
			entry = match self.boundary_port_nos.get(&port_no) {
				Some(_) => self.routing_table.lock().unwrap().get_entry(0).chain_err(|| ErrorKind::PacketEngineError)?,
				None => self.routing_table.lock().unwrap().get_entry(my_index).chain_err(|| ErrorKind::PacketEngineError)?
			}
		}
		if entry.is_in_use() {
			//println!("PacketEngine {}: packet {} entry {}", self.cell_id, packet.get_count(), entry);
			// The control tree is special since each cell has a different uuid
			if (entry.get_index() == 0) || (entry.get_uuid() == packet.get_header().get_uuid()) {
				let mask = entry.get_mask();
				let other_indices = entry.get_other_indices();
				PortNumber::new(port_no, other_indices.len() as u8).chain_err(|| ErrorKind::PacketEngineError)?; // Verify that port_no is valid
				self.forward(port_no, entry, mask, packet).chain_err(|| ErrorKind::PacketEngineError)?;	
			} else {
				println!("CellID {}: entry index {}, entry uuid {}, packet uuid {}",
					self.cell_id, entry.get_index(), entry.get_uuid(), packet.get_header().get_uuid());
				return Err(ErrorKind::Uuid(self.cell_id.clone(), entry.get_index(), entry.get_uuid(),
						packet.get_uuid()).into());
			}
		}
		Ok(())	
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet) 
			-> Result<()>{
		let header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		PortNumber::new(recv_port_no, other_indices.len() as u8).chain_err(|| ErrorKind::PacketEngineError)?; // Make sure recv_port_no is valid
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent as usize) {
				if parent == 0 {
					self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::PacketEngineError)?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent as usize) {
						sender.send((*other_index, packet)).chain_err(|| ErrorKind::PacketEngineError)?;
						//println!("PacketEngine {}: sent rootward on port {} sent packet {}", self.cell_id, recv_port_no, packet);
						let is_up = entry.get_mask().equal(Mask::new0());
						if is_up { // Send to cell agent, too
							self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::PacketEngineError)?;
						}
					} else {
						return Err(ErrorKind::Sender(self.cell_id.clone(), parent).into());
					}
				}
			} 
		} else {
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
			let is_stack_msg = match format!("{}", packet).find("StackTree") {
				Some(_) => true,
				None => false
			};
			if is_stack_msg { println!("PacketEngine {}: forwarding packet {} on ports {:?}, {}", self.cell_id, packet.get_count(), port_nos, entry); }
			for port_no in port_nos.iter() {
				if let Some(other_index) = other_indices.get(*port_no as usize) {
					if *port_no as usize == 0 { 
						//println!("PacketEngine {}: sending to ca packet {}", self.cell_id, packet);
						self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::PacketEngineError)?;
					} else {
						match self.pe_to_ports.get(*port_no as usize) {
							Some(s) => s.send((*other_index, packet)).chain_err(|| ErrorKind::PacketEngineError)?,
							None => return Err(ErrorKind::Sender(self.cell_id.clone(), *port_no).into())
						};
						if is_stack_msg { println!("Packet Engine {} sent to port {} packet {}", self.cell_id, port_no, packet); }
					}
				}
			}
		}
		Ok(())
	}
	fn write_err(&self, e: Error) -> Result<()>{
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "PacketEngine {}: {}", self.cell_id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
		Err(e)
	}
}
impl fmt::Display for PacketEngine {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Packet Engine for cell {}", self.cell_id);
		s = s + &format!("{}", *self.routing_table.lock().unwrap());
		write!(f, "{}", s) }	
}
// Errors
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		PeToPort(::message_types::PePortError);
		PeToCa(::message_types::PeCaError);
	}
	links {
		Port(::port::Error, ::port::ErrorKind);
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { PacketEngineError
		Uuid(cell_id: CellID, index: TableIndex, table_uuid: Uuid, packet_uuid: Uuid) {
			display("CellID {}: index {}, entry uuid {}, packet uuid {}", cell_id, index, 
				table_uuid, packet_uuid)
		}
		Sender(cell_id: CellID, port_no: PortNo) {
			display("No sender for port {} on cell {}", port_no, cell_id)
		}
	}
}
