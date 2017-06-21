use std::fmt;
use std::sync::{Arc, Mutex};
use crossbeam::Scope;
use config::{PortNo, TableIndex};
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
				-> Result<PacketEngine> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).chain_err(|| ErrorKind::PacketEngineError)?)); 
		let pe = PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			pe_to_ca: packet_pe_to_ca, pe_to_ports: pe_to_ports };
		pe.ca_channel(scope, pe_from_ca).chain_err(|| ErrorKind::PacketEngineError)?;
		pe.port_channel(scope, pe_from_ports).chain_err(|| ErrorKind::PacketEngineError)?;
		Ok(pe)
	}
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet) 
			-> Result<()>{
		let mut header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		PortNumber::new(recv_port_no, other_indices.len() as u8).chain_err(|| ErrorKind::PacketEngineError)?; // Make sure recv_port_no is valid
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent as usize) {
				header.set_other_index(*other_index);
				if parent == 0 {
					self.pe_to_ca.send((None,Some((recv_port_no, *other_index, packet)))).chain_err(|| ErrorKind::PacketEngineError)?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent as usize) {
						sender.send(packet).chain_err(|| ErrorKind::PacketEngineError)?;
						//println!("PacketEngine {}: sent packet {} rootward on port {}", self.cell_id, packet.get_packet_count(), parent);
						let is_up = entry.get_mask().equal(Mask::new0());
						if is_up { // Send to cell agent, too
							let other_index: TableIndex = 0;
							self.pe_to_ca.send((None,Some((recv_port_no, other_index, packet)))).chain_err(|| ErrorKind::PacketEngineError)?;
						}
					} else {
						let max_ports = self.pe_to_ports.len() as u8;
						return Err(ErrorKind::Sender(self.cell_id.clone(), parent).into());
					}
				}
			} 
		} else {
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
			//println!("PacketEngine {}: forwarding packet {} on ports {:?}, {}", self.cell_id, packet.get_count(), port_nos, entry);
			for port_no in port_nos.iter() {
				let other_index = *other_indices.get(*port_no as usize).expect("PacketEngine: No such other index");
				header.set_other_index(other_index as u32);
				if *port_no as usize == 0 { 
					self.pe_to_ca.send((None,Some((recv_port_no, other_index, packet)))).chain_err(|| ErrorKind::PacketEngineError)?;
				} else {
					match self.pe_to_ports.get(*port_no as usize) {
						Some(s) => s.send(packet).chain_err(|| ErrorKind::PacketEngineError)?,
						None => return Err(ErrorKind::Sender(self.cell_id.clone(), *port_no).into())
					};
					//println!("Packet Engine {} sent packet {} to port {}", cell_id, packet_count, port_no);
				}
			}
		}
		Ok(())
	}
	pub fn ca_channel(&self, scope: &Scope, entry_pe_from_ca: PeFromCa) -> Result<()> {
		let pe = self.clone();
		scope.spawn( move || -> Result<()> {
			match pe.listen_ca(entry_pe_from_ca) {
				Ok(r) => Ok(r),
				Err(err) => pe.write_err(err)
			}
		});
		Ok(())
	}
	fn port_channel(&self, scope: &Scope, pe_from_ports: PeFromPort) -> Result<()> {
		//let cell_id = self.cell_id.clone();
		let pe = self.clone();
		scope.spawn( move || -> Result<()> {
			match pe.listen_port(pe_from_ports) {
				Ok(r) => Ok(r),
				Err(err) => pe.write_err(err)
			}
		});
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
	fn listen_ca(&self, entry_pe_from_ca: PeFromCa) -> Result<()> {
		loop { 
			let (opt_entry, opt_packet) = entry_pe_from_ca.recv().chain_err(|| ErrorKind::PacketEngineError)?; 
			let entry = match opt_entry {
				Some(e) => {
					self.routing_table.lock().unwrap().set_entry(e);
					//println!("PacketEngine {}: {}", self.cell_id, e);
					e
				},
				None => match opt_packet {
					Some((index, _, _)) => self.routing_table.lock().unwrap().get_entry(index).chain_err(|| ErrorKind::PacketEngineError)?,
					None => panic!("entry and packet empty")
				}
			};
			match opt_packet {
				Some((_, user_mask, packet)) => {
					//println!("PacketEngine {}: received packet {} from ca", self.cell_id, packet.get_count());
					let port_no = 0 as PortNo;
					self.forward(port_no, entry, user_mask, packet).chain_err(|| ErrorKind::PacketEngineError)?;
					//let ports = mask.get_port_nos();
					//println!("PacketEngine {}: send discover to ports {:?}", pe.cell_id, ports);
				}
				None => ()				
			}
		}		
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<()> {
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			let (opt_status, opt_packet) = pe_from_ports.recv()?;
			match opt_status { // Status or packet, never both
				Some(status) => {
					self.pe_to_ca.send((Some(status),None)).chain_err(|| ErrorKind::PacketEngineError)?
				},
				None => {
					match opt_packet {
						Some((port_no, packet)) => {
							//println!("PacketEngine {}: received packet {} from port {}", self.cell_id, packet.get_count(), port_no);
							self.process_packet(port_no, packet).chain_err(|| ErrorKind::PacketEngineError)?
						},
						None => println!("PacketEngine {}: Empty message", self.cell_id)
					};
				}
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, packet: Packet) -> Result<()> {
		//println!("PacketEngine {}: received packet {} on port {}", cell_id, packet_count, recv_port_no);
		let header = packet.get_header();
		let my_index = header.get_other_index();
		let entry;
		{
			entry = self.routing_table.lock().unwrap().get_entry(my_index).chain_err(|| ErrorKind::PacketEngineError)?;
		}
		//println!("PacketEngine {}: packet {} entry {}", self.cell_id, packet.get_count(), entry);
		let mask = entry.get_mask();
		let other_indices = entry.get_other_indices();
		PortNumber::new(port_no, other_indices.len() as u8).chain_err(|| ErrorKind::PacketEngineError)?; // Verify that port_no is valid
		self.forward(port_no, entry, mask, packet).chain_err(|| ErrorKind::PacketEngineError)?;	
		Ok(())	
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
		PeToPort(::nalcell::PePortError);
		PeToCa(::nalcell::PeCaError);
	}
	links {
		Port(::port::Error, ::port::ErrorKind);
		RoutingTable(::routing_table::Error, ::routing_table::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { PacketEngineError
		Sender(cell_id: CellID, port_no: PortNo) {
			description("No sender for port")
			display("No sender for port {} on cell {}", port_no, cell_id)
		}
	}
}
