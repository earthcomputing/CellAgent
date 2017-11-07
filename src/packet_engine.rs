use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use uuid::Uuid;

use config::{PortNo, TableIndex};
use message_types::{PeFromCa, PeToCa, PeToPort, PeFromPort, CaToPePacket, PortToPePacket, PeToCaPacket};
use name::{CellID};
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

impl PacketEngine {
	pub fn new(cell_id: &CellID, packet_pe_to_ca: PeToCa, pe_to_ports: Vec<PeToPort>, 
			boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone())?)); 
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			boundary_port_nos: boundary_port_nos, pe_to_ca: packet_pe_to_ca, pe_to_ports: pe_to_ports })
	}
	pub fn start_threads(&self, pe_from_ca: PeFromCa, pe_from_ports: PeFromPort) {			
		let pe = self.clone();
		::std::thread::spawn( move ||  {
			let _ = pe.listen_ca(pe_from_ca).map_err(|e| pe.write_err(e));
		});
		let pe = self.clone();
		::std::thread::spawn( move || {
			let _ = pe.listen_port(pe_from_ports).map_err(|e| pe.write_err(e));
		});
	}
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	fn listen_ca(&self, entry_pe_from_ca: PeFromCa) -> Result<()> {
		loop { 
			match entry_pe_from_ca.recv()? {
				CaToPePacket::Entry(e) => {
					if *e.get_index() > 0 {
						let json = ::serde_json::to_string(&(&self.cell_id, &e, &e.get_mask().get_port_nos()))?;
						::utility::append2file(json)?;
					}
					self.routing_table.lock().unwrap().set_entry(e)
				},
				CaToPePacket::Packet((index, user_mask, packet)) => {
					//println!("PacketEngine {}: received from ca packet {}", self.cell_id, packet);
					let locked = self.routing_table.lock().unwrap();	// Hold lock until forwarding is done			
					let entry = locked.get_entry(index)?;
					let port_no = PortNo{v:0};
					if entry.may_send() { self.forward(port_no, entry, user_mask, packet)?; }
				}
			}; 
		}
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<()> {
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			match pe_from_ports.recv()? {
				PortToPePacket::Status((port_no, is_border, status)) => self.pe_to_ca.send(PeToCaPacket::Status(port_no, is_border, status))?,
				PortToPePacket::Packet((port_no, my_index, packet)) => self.process_packet(port_no, my_index, packet)?
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, my_index: TableIndex, packet: Packet) -> Result<()> {
		//println!("PacketEngine {}: received on port {} my index {} {}", self.cell_id, port_no.v, *my_index, packet);
		let entry =
		{   
			let locked = self.routing_table.lock().unwrap();
			match self.boundary_port_nos.get(&port_no) {
				Some(_) => locked.get_entry(TableIndex(0))?,
				None => locked.get_entry(my_index)?
			}
		};
		if entry.is_in_use() {
			//println!("PacketEngine {}: packet {} entry {}", self.cell_id, packet.get_count(), entry);
			// The control tree is special since each cell has a different uuid
			if (*entry.get_index() == 0) || (entry.get_uuid() == packet.get_header().get_tree_uuid()) {
				let mask = entry.get_mask();
				let other_indices = entry.get_other_indices();
				PortNumber::new(port_no, PortNo{v:other_indices.len() as u8})?; // Verify that port_no is valid
				self.forward(port_no, entry, mask, packet)?;	
			} else {
				return Err(ErrorKind::Uuid(self.cell_id.clone(), "process_packet".to_string(), entry.get_index(), entry.get_uuid(),
						packet.get_tree_uuid()).into());
			}
		}
		Ok(())	
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet) 
			-> Result<()>{
		let header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		PortNumber::new(recv_port_no, PortNo{v:other_indices.len() as u8})?; // Make sure recv_port_no is valid
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent.v as usize) {
				if parent.v == 0 {
					self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, entry.get_index(), packet))?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent.v as usize) {
						sender.send((*other_index, packet))?;
						//println!("PacketEngine {}: sent rootward on port {} sent packet {}", self.cell_id, recv_port_no, packet);
						let is_up = entry.get_mask().and(user_mask).equal(Mask::new0());
						if is_up { // Send to cell agent, too
							self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, entry.get_index(), packet))?;
						}
					} else {
						return Err(ErrorKind::Sender(self.cell_id.clone(), "forward root".to_string(), parent).into());
					}
				}
			} 
		} else {
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
			//let is_stack_msg = match format!("{}", packet).find("StackTree") { Some(_) => true, None => false };
			//if MsgType::is_type(packet, "StackTree") { println!("PacketEngine {}: forwarding packet {} on ports {:?}, {}", self.cell_id, packet.get_count(), port_nos, entry); }
			for port_no in port_nos.iter() {
				if let Some(other_index) = other_indices.get(port_no.v as usize) {
					if port_no.v as usize == 0 { 
						//println!("PacketEngine {}: sending to ca packet {}", self.cell_id, packet);
						self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, entry.get_index(), packet))?;
					} else {
						match self.pe_to_ports.get(port_no.v as usize) {
							Some(s) => s.send((*other_index, packet))?,
							None => return Err(ErrorKind::Sender(self.cell_id.clone(), "forward leaf".to_string(), *port_no).into())
						};
						//if is_stack_msg { println!("Packet Engine {} sent to port {} packet {}", self.cell_id, port_no.v, packet); }
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
		PeToCa(::message_types::PeCaError);
		PeToPort(::message_types::PePortError);
		Recv(::std::sync::mpsc::RecvError);	
		Serialize(::serde_json::Error);	
	}
	links {
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { 
		Sender(cell_id: CellID, func_name: String, port_no: PortNo) {
			display("PacketEngine {}: No sender for port {} on cell {}", func_name, port_no.v, cell_id)
		}
		Uuid(cell_id: CellID, func_name: String, index: TableIndex, table_uuid: Uuid, packet_uuid: Uuid) {
			display("PacketEngine {}: CellID {}: index {}, entry uuid {}, packet uuid {}", func_name, cell_id, index.0, 
				table_uuid, packet_uuid)
		}
	}
}
