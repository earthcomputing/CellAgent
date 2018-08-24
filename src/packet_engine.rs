use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::collections::HashSet;

use config::{CONTINUE_ON_ERROR, DEBUG_OPTIONS, MAX_PORTS, PortNo};
use dal;
use message::MsgType;
use message_types::{PeFromCm, PeToCm,
                    PeToPort, PeFromPort, PortToPePacket, PeToPortPacket,
                    PeToPe, PeFromPe, CmToPePacket, PeToCmPacket};
use name::{Name, CellID};
use packet::{Packet};
use routing_table::{RoutingTable};
use routing_table_entry::{RoutingTableEntry};
use utility::{Mask, PortNumber, S, TraceHeader, TraceHeaderParams, TraceType, write_err};
use uuid_ec::{AitState, Uuid};

// TODO: Figure out how to packet engine gets trace messagesto the DAL

const MODULE: &'static str = "packet_engine.rs";
#[derive(Debug, Clone)]
pub struct PacketEngine {
	cell_id: CellID,
	boundary_port_nos: HashSet<PortNo>,
	routing_table: Arc<Mutex<RoutingTable>>,
    pe_to_cm: PeToCm,
	pe_to_ports: Vec<PeToPort>,
}

impl PacketEngine {
	pub fn new(cell_id: &CellID, pe_to_cm: PeToCm, pe_to_ports: Vec<PeToPort>,
			boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine, Error> {
		let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).context(PacketEngineError::Chain { func_name: "new", comment: S(cell_id.get_name())})?));
		Ok(PacketEngine { cell_id: cell_id.clone(), routing_table, boundary_port_nos,
            pe_to_cm, pe_to_ports })
	}
	pub fn initialize(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort,
                      mut trace_header: TraceHeader) -> Result<(), Error> {
        let (pe_to_pe, pe_from_pe): (PeToPe, PeFromPe) = channel();
        //self.listen_cm(pe_from_cm, pe_to_pe, trace_header.fork_trace())?;
        self.listen_cm(pe_from_cm, pe_to_pe, trace_header.fork_trace())?;
        self.listen_port(pe_from_ports, pe_from_pe, trace_header.fork_trace())?;
        Ok(())
	}
    pub fn get_id(&self) -> CellID { self.cell_id.clone() }
    fn listen_cm(&self, pe_from_cm: PeFromCm, pe_to_pe: PeToPe,
                 outer_trace_header: TraceHeader) -> Result<(), Error> {
        let _f = "listen_cm";
        let mut pe = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        ::std::thread::spawn( move ||  {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = pe.listen_cm_loop(&pe_from_cm, &pe_to_pe, inner_trace_header).map_err(|e| write_err("packet_engine", e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_cm(pe_from_cm, pe_to_pe, outer_trace_header); }
        });
        Ok(())
    }
    // TODO: One thread for all ports; should be a different thread for each port
    fn listen_port(&self, pe_from_ports: PeFromPort, pe_from_pe: PeFromPe,
                   mut outer_trace_header: TraceHeader)
            -> Result<(),Error> {
        let f = "listen_port";
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_listen_ports" };
            let trace = json!({ "cell_id": &self.cell_id });
            let _ = dal::add_to_trace(&mut outer_trace_header, TraceType::Debug, trace_params, &trace, f);
        }
        let mut pe = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        ::std::thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = pe.listen_port_loop(&pe_from_ports, &pe_from_pe, inner_trace_header).map_err(|e| write_err("packet_engine", e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_port(pe_from_ports, pe_from_pe, outer_trace_header); }
        });
        Ok(())
    }
	//pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
    fn listen_cm_loop(&mut self, pe_from_cm: &PeFromCm, pe_to_pe: &PeToPe,
                      trace_header: &mut TraceHeader)
            -> Result<(), Error> {
        let f = "listen_cm_loop";
        loop {
            match pe_from_cm.recv().context(PacketEngineError::Chain { func_name: f, comment: S("recv entry from cm") + self.cell_id.get_name()})? {
                CmToPePacket::Entry(entry) => {
                    self.routing_table.lock().unwrap().set_entry(entry)
                },
                CmToPePacket::Packet((user_mask, mut packet)) => {
                    let mut uuid = packet.get_tree_uuid();
                    uuid.make_normal();  // Strip AIT info for lookup
                    let locked = self.routing_table.lock().unwrap();    // Hold lock until forwarding is done
                    let entry = locked.get_entry(uuid).context(PacketEngineError::Chain { func_name: f, comment: S(self.cell_id.get_name()) })?;
                    match packet.get_ait_state() {
                        AitState::Tick | AitState::Tock | AitState::Tack | AitState::Teck => return Err(PacketEngineError::Ait { func_name: f, ait_state: packet.get_ait_state() }.into()), // Not allowed here
                        AitState::Ait => { // Update state and send on ports from entry
                            packet.next_ait_state()?;
                            let mask = user_mask.and(entry.get_mask());
                            let port_nos = mask.get_port_nos();
                            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_pkt_send {   // Debug print
                                let msg_type = MsgType::msg_type(&packet);
                                let tree_id = packet.get_tree_id();
                                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_forward_leafward" };
                                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id,
                                    "ait_state": packet.get_ait_state(), "msg_type": &msg_type, "port_nos": &port_nos });
                                if DEBUG_OPTIONS.pe_pkt_send {
                                    match msg_type {
                                        MsgType::Discover => (),
                                        MsgType::DiscoverD => if tree_id.is_name("Tree:C:2") {
                                            println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id);
                                        },
                                        MsgType::Manifest => {
                                            println!("PacketEngine {} forwarding manifest leafward mask {} entry {}", self.cell_id, mask, entry);
                                        },
                                        MsgType::StackTree => {
                                            println!("Packetengine {}: {} AIT state {}", self.cell_id, f, packet.get_ait_state());
                                        },
                                        _ => {
                                            println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id);
                                        }
                                    }
                                }
                                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                            }
                            for port_no in port_nos.iter().cloned() {
                                match self.pe_to_ports.get(*port_no as usize) {
                                    Some(s) => s.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: f, comment: S("send packet leafward ") + self.cell_id.get_name() })?,
                                    None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())
                                };
                            }
                        }
                        AitState::Normal => {
                            let port_no = PortNo(0);
                            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_pkt_recv {  // Debug print
                                let msg_type = MsgType::msg_type(&packet);
                                let tree_id = packet.get_tree_id();
                                let ait_state = packet.get_ait_state();
                                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_packet_from_cm" };
                                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "ait_state": ait_state, "msg_type": &msg_type });
                                if DEBUG_OPTIONS.pe_pkt_recv {
                                    match msg_type {
                                        MsgType::DiscoverD => {
                                            if tree_id.is_name("Tree:C:2") {
                                                println!("PacketEngine {}: {} got from cm {} {}", self.cell_id, f, msg_type, tree_id);
                                            }
                                        },
                                        _ => (),
                                    }
                                }
                                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                            }
                            self.forward(port_no, entry, user_mask, packet, trace_header).context(PacketEngineError::Chain { func_name: f, comment: S(self.cell_id.get_name()) })?;
                        }
                    }
                },
                CmToPePacket::Tcp((port_number, msg)) => {
                    let port_no = port_number.get_port_no();
                    match self.pe_to_ports.get(*port_no as usize) {
                        Some(sender) => sender.send(PeToPortPacket::Tcp(msg)).context(PacketEngineError::Chain { func_name: f, comment: S("send TCP to port ") + self.cell_id.get_name() })?,
                        _ => return Err(PacketEngineError::Sender { func_name: f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    }
                },
                CmToPePacket::Unblock => {
                    //println!("PacketEngine {}: {} send unblock", self.cell_id, f);
                    pe_to_pe.send(S("Unblock"))?;
                }
            };
        }
    }
	fn listen_port_loop(&mut self, pe_from_ports: &PeFromPort, pe_from_pe: &PeFromPe,
                        trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "listen_port_loop";
		loop {
			match pe_from_ports.recv().context(PacketEngineError::Chain { func_name: f, comment: S("receive")})? {
				PortToPePacket::Packet((port_no, packet))  => {
                    self.process_packet(port_no, packet, pe_from_pe, trace_header).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + self.cell_id.get_name()})?
                },
				PortToPePacket::Status((port_no, is_border, status)) => {
                    self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + self.cell_id.get_name()})?
                },
				PortToPePacket::Tcp((port_no, tcp_msg)) => {
                    self.pe_to_cm.send(PeToCmPacket::Tcp((port_no, tcp_msg))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send tcp msg to ca ") + self.cell_id.get_name()})?
                },
			};
		}		
	}
	fn process_packet(&mut self, port_no: PortNo, mut packet: Packet, pe_from_pe: &PeFromPe,
                      trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "process_packet";
        //println!("PacketEngine {}: received on port {} my index {} {}", self.cell_id, port_no.v, *my_index, packet);
        // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
        // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
        // when I block waiting for a tree update because of a deadlock with listen_cm_loop.
        match packet.get_ait_state() {
            AitState::Ait => return Err(PacketEngineError::Ait { func_name: f, ait_state: AitState::Ait }.into()), // Error, should never get from port
            AitState::Tock => { // Send to CM and transition to ENTL
                packet.next_ait_state()?;
                match self.pe_to_ports.get(*port_no as usize) {
                    Some(s) => s.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: f, comment: S("send packet leafward ") + self.cell_id.get_name() })?,
                    None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())
                };
                packet.make_ait();
                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name() })?;
            },
            AitState::Tick => (), // Inform CM of success and enter ENTL
            AitState::Tack | AitState::Teck => { // Update and send back on same port
                packet.next_ait_state()?;
                match self.pe_to_ports.get(*port_no as usize) {
                    Some(s) => s.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: f, comment: S("send packet leafward ") + self.cell_id.get_name() })?,
                    None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())
                };
            },
            AitState::Normal => { // Forward packet
                let mut uuid = packet.get_tree_uuid();
                uuid.make_normal();
                let entry = {
                    let locked = self.routing_table.lock().unwrap();
                    match locked.get_entry(uuid) {
                        Ok(e) => e,
                        Err(_) => { // Send to Cell agent if tree not recognized
                            self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name() })?;
                            return Ok(())
                        }
                    }
                };
                if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_process_pkt {   // Debug print
                    let msg_type = MsgType::msg_type(&packet);
                    let tree_id = packet.get_tree_id();
                    let ait_state = packet.get_ait_state();
                    let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_process_packet" };
                    let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "ait_state": ait_state,
                        "msg_type": &msg_type, "port_no": &port_no, "entry": &entry });
                    if DEBUG_OPTIONS.pe_process_pkt {
                        match msg_type {
                            MsgType::Discover => (),
                            MsgType::DiscoverD => if tree_id.is_name("Tree:C:2") {
                                println!("PacketEngine {}: got from {} {} {}", self.cell_id, *port_no, msg_type, tree_id);
                            }
                            _ => {
                                println!("PacketEngine {}: got from {} {} {} {}", self.cell_id, *port_no, msg_type, tree_id, entry);
                            },
                        }
                    }
                    let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                }
                if entry.is_in_use() {
                    if entry.get_uuid() == packet.get_uuid() {
                        let mask = entry.get_mask();
                        // Next line verifies that port_no is valid
                        PortNumber::new(port_no, MAX_PORTS).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("port number ") + self.cell_id.get_name() })?; // Verify that port_no is valid
                        self.forward(port_no, entry, mask, packet, trace_header).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + self.cell_id.get_name() })?;
                    } else {
                        let msg_type = MsgType::msg_type(&packet);
                        return Err(PacketEngineError::Uuid { cell_id: self.cell_id.clone(), func_name: f, msg_type, packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
                    }
                    // TODO: Fix to block only the parent port of the specific tree
                    // Wait for permission to proceed if packet is from a port and will result in a tree update
                    if packet.is_blocking() && packet.is_last_packet() {
                        pe_from_pe.recv()?;
                    }
                }
            }
        }
		Ok(())
	}
	fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet,
               trace_header: &mut TraceHeader) -> Result<(), Error>{
        let f = "forward";
        if !(recv_port_no == entry.get_parent()) {
			let parent = entry.get_parent();
            if *parent == 0 {
                if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_pkt_send {   // Debug print
                    let msg_type = MsgType::msg_type(&packet);
                    if msg_type == MsgType::Manifest {
                        println!("PacketEngine {} forwarding manifest rootward", self.cell_id);
                    }
                    let tree_id = packet.get_tree_id();
                    let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_forward_to_cm" };
                    let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "msg_type": &msg_type, "parent_port": &parent });
                    if DEBUG_OPTIONS.pe_pkt_send {
                        match msg_type {
                            MsgType::Discover => (),
                            _ => {
                                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                                println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, f, *parent, msg_type, tree_id);
                            },

                        }
                    }
                }
                self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
            } else {
                if let Some(sender) = self.pe_to_ports.get((*parent) as usize) {
                    sender.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: f, comment: S(self.cell_id.clone()) })?;
                    if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_pkt_send {   // Debug print
                        let msg_type = MsgType::msg_type(&packet);
                        if msg_type == MsgType::Manifest {
                            println!("PacketEngine {} forwarding manifest leafward", self.cell_id);
                        }
                        let tree_id = packet.get_tree_id();
                        let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_forward_rootward" };
                        let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "msg_type": &msg_type, "parent_port": &parent });
                        if DEBUG_OPTIONS.pe_pkt_send {
                            match msg_type {
                                MsgType::Discover => (),
                                _ => {
                                    let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                                    println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, f, *parent, msg_type, tree_id);
                                },
                            }
                        }
                    }
                    let is_up = entry.get_mask().and(user_mask).equal(Mask::port0());
                    if is_up { // Send to cell agent, too
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name() })?;
                    }
                } else {
                    return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward rootward", port_no: *parent }.into());
                }
            }
		} else {  // Leafward
			let mask = user_mask.and(entry.get_mask());
			let port_nos = mask.get_port_nos();
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.pe_pkt_send {   // Debug print
                let msg_type = MsgType::msg_type(&packet);
                let tree_id = packet.get_tree_id();
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "pe_forward_leafward" };
                let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id, "msg_type": &msg_type, "port_nos": &port_nos });
                if DEBUG_OPTIONS.pe_pkt_send {
                    if msg_type == MsgType::Manifest {
                        println!("PacketEngine {} forwarding manifest leafward mask {} entry {}", self.cell_id, mask, entry);
                    }
                    match msg_type {
                        MsgType::Discover => (),
                        MsgType::DiscoverD => if tree_id.is_name("Tree:C:2") {
                            println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id);
                        }
                        _ => {
                            println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, f, port_nos, msg_type, tree_id);
                        }
                    }
                }
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            }
			for port_no in port_nos.iter().cloned() {
                if *port_no == 0 {
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: f, comment: S("leafcast packet to ca ") + self.cell_id.get_name() })?;
                } else {
                    match self.pe_to_ports.get(*port_no as usize) {
                        Some(s) => s.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: f, comment: S("send packet leafward ") + self.cell_id.get_name() })?,
                        None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())
                    };
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
    #[fail(display = "PacketEngineError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
	#[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
	#[fail(display = "PacketEngineError::Sender {}: No sender for port {:?} on cell {}", func_name, port_no, cell_id)]
	Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, table_uuid: Uuid, packet_uuid: Uuid }
}
