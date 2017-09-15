use std::fmt;
use std::io::Write;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use uuid::Uuid;

use config::{PortNo, TableIndex};
use message::MsgType;
use message_types::{PeFromCa, PeToCa, PeToPort, PeFromPort, CaToPePacket, PortToPePacket, PeToCaPacket};
use name::{Name, CellID};
use packet::{Packet};
use routing_table::{RoutingTable};
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber, S};

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
		let f = "new";
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).chain_err(|| ErrorKind::RoutingTable(cell_id.clone(), S(f)))?)); 
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
		let f = "listen_ca";
		loop { 
			match entry_pe_from_ca.recv().chain_err(|| ErrorKind::RecvCa(self.cell_id.clone(), S(f)))? {
				CaToPePacket::Entry(e) => {
					if *e.get_index() > 0 {
						let children = e.get_mask().get_port_nos();
						let json = ::serde_json::to_string(&e).chain_err(|| ErrorKind::Serialize(self.cell_id.clone(), S(f), S(e)))?;
						let json2 = ::serde_json::to_string(&e.get_mask().get_port_nos()).chain_err(|| ErrorKind::Serialize(self.cell_id.clone(), S(f), S(e.get_mask())))?;
						let string = format!("Entry {}: {} {}", self.cell_id, json, json2);
						::utility::append2file(string).chain_err(|| ErrorKind::Trace(self.cell_id.clone(), S(f)))?;
					}
					self.routing_table.lock().unwrap().set_entry(e)
				},
				CaToPePacket::Packet((index, user_mask, packet)) => {
					//println!("PacketEngine {}: received from ca packet {}", self.cell_id, packet);
					let locked = self.routing_table.lock().unwrap();	// Hold lock until forwarding is done			
					let entry = locked.get_entry(index).chain_err(|| ErrorKind::TableIndex(self.cell_id.clone(), index, S(f)))?;
					let port_no = PortNo{v:0};
					if entry.may_send() { self.forward(port_no, entry, user_mask, packet).chain_err(|| ErrorKind::Forward(self.cell_id.clone(), port_no, S(f)))?; }
				}
			}; 
		}
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<()> {
		let f = "listen_port";
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			match pe_from_ports.recv().chain_err(|| ErrorKind::RecvPort(self.cell_id.clone(), S(f)))? {
				PortToPePacket::Status((port_no, is_border, status)) => self.pe_to_ca.send(PeToCaPacket::Status(port_no, is_border, status)).chain_err(|| ErrorKind::SendCa(self.cell_id.clone(), S(f)))?,
				PortToPePacket::Packet((port_no, my_index, packet)) => self.process_packet(port_no, my_index, packet).chain_err(|| ErrorKind::Process(self.cell_id.clone(), S(f), port_no, my_index, packet))?
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, my_index: TableIndex, packet: Packet) -> Result<()> {
		let f = "process_packet";
		//println!("PacketEngine {}: received on port {} my index {} {}", self.cell_id, port_no.v, *my_index, packet);
		let entry =
		{   
			let locked = self.routing_table.lock().unwrap();
			match self.boundary_port_nos.get(&port_no) {
				Some(_) => locked.get_entry(TableIndex(0)).chain_err(|| ErrorKind::TableIndex(self.cell_id.clone(), TableIndex(0), S(f)))?,
				None => locked.get_entry(my_index).chain_err(|| ErrorKind::TableIndex(self.cell_id.clone(), TableIndex(0), S(f)))?
			}
		};
		if entry.is_in_use() {
			//println!("PacketEngine {}: packet {} entry {}", self.cell_id, packet.get_count(), entry);
			// The control tree is special since each cell has a different uuid
			if (*entry.get_index() == 0) || (entry.get_uuid() == packet.get_header().get_tree_uuid()) {
				let mask = entry.get_mask();
				let other_indices = entry.get_other_indices();
				PortNumber::new(port_no, PortNo{v:other_indices.len() as u8}).chain_err(|| ErrorKind::PortNumber(self.cell_id.clone(), S(f), port_no))?; // Verify that port_no is valid
				self.forward(port_no, entry, mask, packet).chain_err(|| ErrorKind::Forward(self.cell_id.clone(), port_no, S(f)))?;	
			} else {
				return Err(ErrorKind::Uuid(self.cell_id.clone(), "process_packet".to_string(), entry.get_index(), entry.get_uuid(),
						packet.get_tree_uuid()).into());
			}
		}
		Ok(())	
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet) 
			-> Result<()>{
		let f = "forward";
		let header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		PortNumber::new(recv_port_no, PortNo{v:other_indices.len() as u8}).chain_err(|| ErrorKind::PortNumber(self.cell_id.clone(), S(f), recv_port_no))?; // Make sure recv_port_no is valid
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent.v as usize) {
				if parent.v == 0 {
					self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::SendCa(self.cell_id.clone(), S(f)))?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent.v as usize) {
						sender.send((*other_index, packet)).chain_err(|| ErrorKind::SendPort(self.cell_id.clone(), parent, S(f)))?;
						//println!("PacketEngine {}: sent rootward on port {} sent packet {}", self.cell_id, recv_port_no, packet);
						let is_up = entry.get_mask().equal(Mask::new0());
						if is_up { // Send to cell agent, too
							self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::SendCa(self.cell_id.clone(), S(f)))?;
						}
					} else {
						return Err(ErrorKind::Sender(self.cell_id.clone(), "forward root".to_string(), parent).into());
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
			//if MsgType::is_type(packet, "StackTree") { println!("PacketEngine {}: forwarding packet {} on ports {:?}, {}", self.cell_id, packet.get_count(), port_nos, entry); }
			for port_no in port_nos.iter() {
				if let Some(other_index) = other_indices.get(port_no.v as usize) {
					if port_no.v as usize == 0 { 
						//println!("PacketEngine {}: sending to ca packet {}", self.cell_id, packet);
						self.pe_to_ca.send(PeToCaPacket::Packet(recv_port_no, packet)).chain_err(|| ErrorKind::SendCa(self.cell_id.clone(), S(f)))?;
					} else {
						match self.pe_to_ports.get(port_no.v as usize) {
							Some(s) => s.send((*other_index, packet)).chain_err(|| ErrorKind::SendPort(self.cell_id.clone(), *port_no, S(f)))?,
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
	errors { 
		Forward(cell_id: CellID, port_no: PortNo, func_name: String) {
			display("PacketEngine {}: Cell {} can't forward packets on port {}", func_name, cell_id, port_no.v)
		}
		PortNumber(cell_id: CellID, func_name: String, port_no: PortNo) {
			display("PacketEngine {}: {} is not a valid port number on cell {}", func_name, port_no.v, cell_id)
		}
		Process(cell_id: CellID, func_name: String, port_no: PortNo, index: TableIndex, packet: Packet) {
			display("PacketEngine {}: Cell {} port {} index {} can't process packet {}", func_name, cell_id, port_no.v, **index, packet)
		}
		RecvCa(cell_id: CellID, func_name: String) {
			display("PacketEngine {}: Error receiving from CellAgent on cell {}", func_name, cell_id)
		}
		RecvPort(cell_id: CellID, func_name: String) {
			display("PacketEngine {}: Error receiving from port on cell {}", func_name, cell_id)
		}
		RoutingTable(cell_id: CellID, func_name: String) {
			display("PacketEngine {}: Can't create routing table on cell {}", func_name, cell_id)
		}
		SendCa(cell_id: CellID, func_name:String) {
			display("PacketEngine {}: Cell {} can't send to CellAgent", func_name, cell_id)
		}
		Sender(cell_id: CellID, func_name: String, port_no: PortNo) {
			display("PacketEngine {}: No sender for port {} on cell {}", func_name, port_no.v, cell_id)
		}
		SendPort(cell_id: CellID, port_no: PortNo, func_name:String) {
			display("PacketEngine {}: Cell {} can't send to port {}", func_name, cell_id, port_no.v)
		}
		Serialize(cell_id: CellID, func_name: String, s: String) {
			display("PacketEngine {}: Cell {} can't serialize {}", func_name, cell_id, s)
		}
		TableIndex(cell_id: CellID, index: TableIndex, func_name: String) {
			display("PacketEngine {}: No entry for table index {} on cell {}", func_name, **index, cell_id)
		}
		Trace(cell_id: CellID, func_name: String) {
			display("Cellagent {}: Error writing status output on cell {}", func_name, cell_id)
		}
		Uuid(cell_id: CellID, func_name: String, index: TableIndex, table_uuid: Uuid, packet_uuid: Uuid) {
			display("PacketEngine {}: CellID {}: index {}, entry uuid {}, packet uuid {}", func_name, cell_id, index.0, 
				table_uuid, packet_uuid)
		}
	}
}
