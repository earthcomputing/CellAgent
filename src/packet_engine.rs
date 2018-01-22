use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

//use uuid::Uuid;

use config::{PortNo, TableIndex};
use message_types::{TCP, PeFromCa, PeToCa, PeToPort, PeFromPort, CaToPePacket, PortToPePacket, PeToPortPacket, PeToCaPacket};
use name::{Name, CellID};
use packet::{Packet};
use routing_table::{RoutingTable};
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber, S, write_err};
use uuid_fake::Uuid;

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
			boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine, Error> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).context(PacketEngineError::Chain { func_name: "new", comment: S(cell_id.get_name())})?));
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table: routing_table, 
			boundary_port_nos: boundary_port_nos, pe_to_ca: packet_pe_to_ca, pe_to_ports: pe_to_ports })
	}
	pub fn start_threads(&self, pe_from_ca: PeFromCa, pe_from_ports: PeFromPort) {			
		let pe = self.clone();
		::std::thread::spawn( move ||  {
			let _ = pe.listen_ca(pe_from_ca).map_err(|e| write_err("packet_engine", e));
		});
		let pe = self.clone();
		::std::thread::spawn( move || {
			let _ = pe.listen_port(pe_from_ports).map_err(|e| write_err("packet_engine", e));
		});
	}
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	fn listen_ca(&self, entry_pe_from_ca: PeFromCa) -> Result<(), Error> {
		loop { 
			match entry_pe_from_ca.recv().context(PacketEngineError::Chain { func_name: "listen_ca", comment: S("recv entry from ca") + self.cell_id.get_name()})? {
				CaToPePacket::Entry(e) => {
					if *e.get_index() > 0 {
						let json = ::serde_json::to_string(&(&self.cell_id, &e, &e.get_mask().get_port_nos())).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S(self.cell_id.get_name())})?;
						::utility::append2file(json).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S("")})?;
					}
					self.routing_table.lock().unwrap().set_entry(e)
				},
				CaToPePacket::Packet((index, user_mask, packet)) => {
					//println!("PacketEngine {}: received from ca packet {}", self.cell_id, packet);
					let locked = self.routing_table.lock().unwrap();	// Hold lock until forwarding is done			
					let entry = locked.get_entry(index).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S(self.cell_id.get_name())})?;
					let port_no = PortNo{v:0};
					if entry.may_send() { self.forward(port_no, entry, user_mask, packet).context(PacketEngineError::Chain { func_name:"listen_ca", comment: S(self.cell_id.get_name())})?; }
				},
				CaToPePacket::Tcp((port_number, msg)) => {
                    let port_no = port_number.get_port_no();
                    match self.pe_to_ports.get(*port_no as usize) {
                        Some(sender) => sender.send(PeToPortPacket::Tcp(msg)).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S("send TCP to port ") + self.cell_id.get_name() })?,
                        _ => return Err(PacketEngineError::Sender { func_name: "listen_ca", cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    }
                }
			}; 
		}
	}
	fn listen_port(&self, pe_from_ports: PeFromPort) -> Result<(), Error> {
		loop {
			//println!("PacketEngine {}: waiting for status or packet", pe.cell_id);
			match pe_from_ports.recv().context(PacketEngineError::Chain { func_name: "listen_port", comment: S("receive")})? {
				PortToPePacket::Packet((port_no, my_index, packet))  => self.process_packet(port_no, my_index, packet).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + self.cell_id.get_name()})?,
				PortToPePacket::Status((port_no, is_border, status)) => self.pe_to_ca.send(PeToCaPacket::Status((port_no, is_border, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + self.cell_id.get_name()})?,
				PortToPePacket::Tcp((port_no, tcp_msg))              => self.pe_to_ca.send(PeToCaPacket::Tcp((port_no, tcp_msg))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send tcp msg to ca ") + self.cell_id.get_name()})?,
			};
		}		
	}
	fn process_packet(&self, port_no: PortNo, my_index: TableIndex, packet: Packet) -> Result<(), Error> {
		//println!("PacketEngine {}: received on port {} my index {} {}", self.cell_id, port_no.v, *my_index, packet);
        let locked = self.routing_table.lock().unwrap();
		let entry = locked.get_entry(my_index).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("not border port ") + self.cell_id.get_name()})?;
		if entry.is_in_use() {
			//println!("PacketEngine {}: entry {} header UUID {}", self.cell_id, entry, packet.get_header().get_tree_uuid());
			// The control tree is special since each cell has a different uuid
            //if ::message::MsgType::is_type(packet, "StackTree") && self.cell_id.get_name() == "C:1" { println!("PacketEngine {}: entry {}", self.cell_id, entry); }
			if (*entry.get_index() == 0) || (entry.get_uuid() == packet.get_header().get_tree_uuid()) {
				let mask = entry.get_mask();
				let other_indices = entry.get_other_indices();
				PortNumber::new(port_no, PortNo{v:other_indices.len() as u8}).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("port number ") + self.cell_id.get_name()})?; // Verify that port_no is valid
				self.forward(port_no, entry, mask, packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + self.cell_id.get_name()})?;
			} else {
				return Err(PacketEngineError::Uuid { cell_id: self.cell_id.clone(), func_name: "process_packet", index: *entry.get_index(), packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
			}
		}
		Ok(())	
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet) 
			-> Result<(), Error>{
		let header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		let recv_port_number = PortNumber::new(recv_port_no, PortNo{v:other_indices.len() as u8}).context(PacketEngineError::Chain { func_name: "forward", comment: S(self.cell_id.clone())})?; // Make sure recv_port_no is valid
        let default_mask = Mask::empty().not(); // Prevents resending a message
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent.v as usize) {
				if parent.v == 0 {
					self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, default_mask, entry.get_index(), packet)))?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent.v as usize) {
						sender.send(PeToPortPacket::Packet((*other_index, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S(self.cell_id.clone())})?;
						//println!("PacketEngine {}: sent rootward on port {} sent packet {}", self.cell_id, recv_port_no, packet);
						let is_up = entry.get_mask().and(user_mask).equal(Mask::new0());
						if is_up { // Send to cell agent, too
							self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, default_mask, entry.get_index(), packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name()})?;
						}
					} else {
						return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward root", port_no: *parent }.into());
					}
				}
			} 
		} else {
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
			//let is_stack_msg = match format!("{}", packet).find("StackTree") { Some(_) => true, None => false };
			//if ::message::MsgType::is_type(packet, "StackTree") { println!("PacketEngine {}: forwarding packet {} on ports {:?}, {}", self.cell_id, packet.get_count(), port_nos, entry); }
			for port_no in port_nos.iter() {
				if let Some(other_index) = other_indices.get(port_no.v as usize).cloned() {
					if port_no.v as usize == 0 {
						//println!("PacketEngine {}: sending to ca packet {}", self.cell_id, packet);
						self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, user_mask, entry.get_index(), packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("leafcast packet to ca ") + self.cell_id.get_name()})?;
					} else {
						match self.pe_to_ports.get(port_no.v as usize) {
							Some(s) => s.send(PeToPortPacket::Packet((other_index, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("send packet leafward ") + self.cell_id.get_name()})?,
							None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: **port_no }.into())
						};
						//if is_stack_msg { println!("Packet Engine {} sent to port {} packet {}", self.cell_id, port_no.v, packet); }
					}
				}
			}
		}
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
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum PacketEngineError {
	#[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
	#[fail(display = "PacketEngineError::Sender {}: No sender for port {:?} on cell {}", func_name, port_no, cell_id)]
	Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: index {}, entry uuid {}, packet uuid {}", func_name, cell_id, index, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, index: u32, table_uuid: Uuid, packet_uuid: Uuid }
}
