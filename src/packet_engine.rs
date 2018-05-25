use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::collections::HashSet;

//use uuid::Uuid;

use config::{PortNo, TableIndex};
use message::MsgType;
use message_types::{PeFromCa, PeToCa, PeToPort, PeFromPort, CaToPePacket, PortToPePacket, PeToPortPacket, PeToCaPacket,
    PeToPe, PeFromPe};
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
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table, boundary_port_nos,
            pe_to_ca: packet_pe_to_ca, pe_to_ports })
	}
	pub fn start_threads(&self, pe_from_ca: PeFromCa, pe_from_ports: PeFromPort) -> Result<(), Error> {
        let (pe_to_pe, pe_from_pe): (PeToPe, PeFromPe) = channel();
        self.listen_ca(pe_from_ca, pe_to_pe)?;
        self.listen_port(pe_from_ports, pe_from_pe)?;
        Ok(())
	}
	fn listen_ca(&self, pe_from_ca: PeFromCa, pe_to_pe: PeToPe) -> Result<(), Error> {
        let mut pe = self.clone();
        ::std::thread::spawn( move || -> Result<(), Error> {
            let _ = pe.listen_ca_loop(&pe_from_ca, &pe_to_pe).map_err(|e| write_err("packet_engine", e));
            let _ = pe.listen_ca(pe_from_ca, pe_to_pe);
            Ok(())
        });
        Ok(())
    }
    // One thread for all ports; should be a different thread for each port
    fn listen_port(&self, pe_from_ports: PeFromPort, pe_from_pe: PeFromPe) -> Result<(),Error> {
        let mut pe = self.clone();
        ::std::thread::spawn( move || -> Result<(), Error> {
            let _ = pe.listen_port_loop(&pe_from_ports, &pe_from_pe).map_err(|e| write_err("packet_engine", e));
            let _ = pe.listen_port(pe_from_ports, pe_from_pe);
            Ok(())
        });
        Ok(())
    }
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
	fn listen_ca_loop(&mut self, pe_from_ca: &PeFromCa, pe_to_pe: &PeToPe) -> Result<(), Error> {
        let f = "PacketEngine: listen_ca_loop";
		loop {
			match pe_from_ca.recv().context(PacketEngineError::Chain { func_name: f, comment: S("recv entry from ca") + self.cell_id.get_name()})? {
				CaToPePacket::Entry(entry) => {
                    {/*   // Visualization output
                        if *entry.get_index() > 0 {
                            {/*  // Debug print
                                if *entry.get_index() == 4 && self.cell_id.is_name("C:3") { println!("PacketEngine {}: updated entry {}", self.cell_id, entry); }
                            */}
                            let json = ::serde_json::to_string(&(&self.cell_id, &entry, &entry.get_mask().get_port_nos())).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S(self.cell_id.get_name()) })?;
                            ::utility::append2file(json).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S("") })?;
                        }
                    */}
					self.routing_table.lock().unwrap().set_entry(entry)
				},
				CaToPePacket::Packet((index, user_mask, packet)) => {
					let locked = self.routing_table.lock().unwrap();	// Hold lock until forwarding is done
					let entry = locked.get_entry(index).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S(self.cell_id.get_name())})?;
					let port_no = PortNo{v:0};
                    {/*   // Debug print
                        let msg_type = MsgType::msg_type(&packet);
                        let tree_id = packet.get_tree_id();
                        match msg_type {
                            MsgType::DiscoverD => {
                                if tree_id.is_name("C:2") { println!("PacketEngine {}: got from ca {} {}", self.cell_id, msg_type, tree_id); }
                            },
                            _ => (),
                        }
                    */}
 					self.forward(port_no, entry, user_mask, packet).context(PacketEngineError::Chain { func_name:"listen_ca", comment: S(self.cell_id.get_name())})?;
				},
				CaToPePacket::Tcp((port_number, msg)) => {
                    let port_no = port_number.get_port_no();
                    match self.pe_to_ports.get(*port_no as usize) {
                        Some(sender) => sender.send(PeToPortPacket::Tcp(msg)).context(PacketEngineError::Chain { func_name: "listen_ca", comment: S("send TCP to port ") + self.cell_id.get_name() })?,
                        _ => return Err(PacketEngineError::Sender { func_name: f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    }
                },
                CaToPePacket::Unblock => {
                    //println!("PacketEngine {}: {} send unblock", self.cell_id, f);
                    pe_to_pe.send(S("Unblock"))?;
                }
			}; 
		}
	}
	fn listen_port_loop(&mut self, pe_from_ports: &PeFromPort, pe_from_pe: &PeFromPe) -> Result<(), Error> {
        let f = "listen_port_loop";
		loop {
			match pe_from_ports.recv().context(PacketEngineError::Chain { func_name: f, comment: S("receive")})? {
				PortToPePacket::Packet((port_no, my_index, packet))  => self.process_packet(port_no, my_index, packet, pe_from_pe).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + self.cell_id.get_name()})?,
				PortToPePacket::Status((port_no, is_border, status)) => self.pe_to_ca.send(PeToCaPacket::Status((port_no, is_border, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + self.cell_id.get_name()})?,
				PortToPePacket::Tcp((port_no, tcp_msg))              => self.pe_to_ca.send(PeToCaPacket::Tcp((port_no, tcp_msg))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send tcp msg to ca ") + self.cell_id.get_name()})?,
			};
		}		
	}
	fn process_packet(&mut self, port_no: PortNo, my_index: TableIndex, packet: Packet, pe_from_pe: &PeFromPe) -> Result<(), Error> {
        let f = "process_packet";
        //println!("PacketEngine {}: received on port {} my index {} {}", self.cell_id, port_no.v, *my_index, packet);
        // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
        // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
        // when I block waiting for a tree update because of a deadlock with listen_ca_loop.
        let entry = {
            let locked = self.routing_table.lock().unwrap();
            locked.get_entry(my_index).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("not border port ") + self.cell_id.get_name() })?
        };
        {/*   // Debug print
            let msg_type = MsgType::msg_type(&packet);
            let tree_id = packet.get_tree_id();
            match msg_type {
                MsgType::Discover => (),
                MsgType::DiscoverD => if tree_id.is_name("C:2") { println!("PacketEngine {}: got from {} {} {}", self.cell_id, port_no.v, msg_type, tree_id); }
                _ => {
                    println!("PacketEngine {}: got from {} {} {} {}", self.cell_id, port_no.v, msg_type, tree_id, entry);
                },
            }
        */}
        if entry.is_in_use() {
            // The control tree is special since each cell has a different uuid
            if (*entry.get_index() == 0) || (entry.get_uuid() == packet.get_header().get_uuid()) {
                let mask = entry.get_mask();
                let other_indices = entry.get_other_indices();
                PortNumber::new(port_no, PortNo { v: other_indices.len() as u8 }).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("port number ") + self.cell_id.get_name() })?; // Verify that port_no is valid
                self.forward(port_no, entry, mask, packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + self.cell_id.get_name() })?;
            } else {
                let msg_type = MsgType::msg_type(&packet);
                return Err(PacketEngineError::Uuid { cell_id: self.cell_id.clone(), func_name: f, msg_type, index: *entry.get_index(), packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
            }
            // TODO: Fix to block only the parent port of the specific tree
            // Wait for permission to proceed if packet is from a port and will result in a tree update
            if packet.is_blocking() && packet.is_last_packet() {
                pe_from_pe.recv()?;
            }
        }
		Ok(())
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet)
			-> Result<(), Error>{
        let f = "forward";
		let header = packet.get_header();
		//println!("PacketEngine {}: forward packet {}, mask {}, entry {}", self.cell_id, packet.get_count(), mask, entry);
		let other_indices = entry.get_other_indices();
		if header.is_rootcast() {
			let parent = entry.get_parent();
			if let Some(other_index) = other_indices.get(parent.v as usize) {
				if parent.v == 0 {
					self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, entry.get_index(), packet)))?;
				} else {
					if let Some(sender) = self.pe_to_ports.get(parent.v as usize) {
						sender.send(PeToPortPacket::Packet((*other_index, packet))).context(PacketEngineError::Chain { func_name: f, comment: S(self.cell_id.clone())})?;
                        {/*   // Debug print
                            let msg_type = MsgType::msg_type(&packet);
                            let tree_id = packet.get_tree_id();
                            match msg_type {
                                MsgType::Discover => (),
                                _ => {
                                    println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, f, parent.v, msg_type, tree_id);
                                },

                            }
                        */}
						let is_up = entry.get_mask().and(user_mask).equal(Mask::port0());
						if is_up { // Send to cell agent, too
							self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, entry.get_index(), packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name()})?;
						}
					} else {
						return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward rootward", port_no: *parent }.into());
					}
				}
			} 
		} else {
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
            {/*   // Debug print
                let msg_type = MsgType::msg_type(&packet);
                let tree_id = packet.get_tree_id();
                match msg_type {
                    MsgType::Discover => (),
                    MsgType::DiscoverD => if tree_id.is_name("C:2") { println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id); }
                    _ => {
                        println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id);
                    }
                }
            */}
			for port_no in port_nos.iter() {
				if let Some(other_index) = other_indices.get(port_no.v as usize).cloned() {
					if port_no.v as usize == 0 {
						self.pe_to_ca.send(PeToCaPacket::Packet((recv_port_no, entry.get_index(), packet))).context(PacketEngineError::Chain { func_name: f, comment: S("leafcast packet to ca ") + self.cell_id.get_name()})?;
					} else {
                        //if msg_type == MsgType::StackTree || msg_type == MsgType::Manifest || msg_type == MsgType::Application {
                        //    println!("PacketEngine {}: sent {} leafward on port {}", self.cell_id, msg_type, port_no.v); }
						match self.pe_to_ports.get(port_no.v as usize) {
							Some(s) => s.send(PeToPortPacket::Packet((other_index, packet))).context(PacketEngineError::Chain { func_name: f, comment: S("send packet leafward ") + self.cell_id.get_name()})?,
							None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: **port_no }.into())
						};
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
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PacketEngineError {
	#[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
	#[fail(display = "PacketEngineError::Sender {}: No sender for port {:?} on cell {}", func_name, port_no, cell_id)]
	Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} index {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, index, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, index: u32, table_uuid: Uuid, packet_uuid: Uuid }
}
