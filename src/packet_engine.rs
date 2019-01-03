use std::{fmt, fmt::Write,
          sync::{Arc, Mutex},
          sync::mpsc::channel,
          collections::HashSet,
          thread};

use crate::config::{CENTRAL_TREE, CONTINUE_ON_ERROR, DEBUG_OPTIONS, MAX_PORTS, TRACE_OPTIONS, PortNo};
use crate::dal;
use crate::dal::{fork_trace_header, update_trace_header};
use crate::message::MsgType;
use crate::message_types::{PeFromCm, PeToCm,
                    PeToPort, PeFromPort, PortToPePacket, PeToPortPacket,
                    PeToPe, PeFromPe, CmToPePacket, PeToCmPacket};
use crate::name::{Name, CellID};
use crate::packet::{Packet};
use crate::routing_table::{RoutingTable};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{Mask, S, TraceHeader, TraceHeaderParams, TraceType, write_err};
use crate::uuid_ec::{AitState, Uuid};

#[derive(Debug, Clone)]
pub struct PacketEngine {
    cell_id: CellID,
    boundary_port_nos: HashSet<PortNo>,
    routing_table: Arc<Mutex<RoutingTable>>,
    pe_to_cm: PeToCm,
    pe_to_ports: Vec<PeToPort>,
}

impl PacketEngine {
    pub fn get_id(&self) -> &CellID { &self.cell_id }
    //pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }

    // NEW
    pub fn new(cell_id: &CellID, pe_to_cm: PeToCm, pe_to_ports: Vec<PeToPort>,
            boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine, Error> {
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id.clone()).context(PacketEngineError::Chain { func_name: "new", comment: S(cell_id.get_name())})?));
        Ok(PacketEngine { cell_id: cell_id.clone(), routing_table, boundary_port_nos,
            pe_to_cm, pe_to_ports })
    }

    // INIT (PeFromCm PeFromPort)
    // WORKER (PacketEngine)
    pub fn initialize(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> Result<(), Error> {
// FIXME: dal::add_to_trace mutates trace_header, spawners don't ??
        let _f = "initialize";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        let (pe_to_pe, pe_from_pe): (PeToPe, PeFromPe) = channel();
        self.listen_cm(pe_from_cm, pe_to_pe)?;
        self.listen_port(pe_from_ports, pe_from_pe)?;
        Ok(())
    }

    // SPAWN THREAD (listen_cm_loop)
    fn listen_cm(&self, pe_from_cm: PeFromCm, pe_to_pe: PeToPe) -> Result<(), Error> {
        let _f = "listen_cm";
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {} listen_cm_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.listen_cm_loop(&pe_from_cm, &pe_to_pe).map_err(|e| write_err("packet_engine", &e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_cm(pe_from_cm, pe_to_pe); }
        })?;
        Ok(())
    }

    // SPAWN THREAD (listen_port)
    // TODO: One thread for all ports; should be a different thread for each port
    fn listen_port(&self, pe_from_ports: PeFromPort, pe_from_pe: PeFromPe)
            -> Result<(),Error> {
        let _f = "listen_port";
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {} listen_port_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.listen_port_loop(&pe_from_ports, &pe_from_pe).map_err(|e| write_err("packet_engine", &e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_port(pe_from_ports, pe_from_pe); }
        })?;
        Ok(())
    }

    // WORKER (PeFromCm)
    fn listen_cm_loop(&mut self, pe_from_cm: &PeFromCm, pe_to_pe: &PeToPe)
            -> Result<(), Error> {
        let _f = "listen_cm_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = pe_from_cm.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("recv entry from cm ") + self.cell_id.get_name()})?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.clone() });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            match msg {
                // control plane from CellAgent
                CmToPePacket::Entry(entry) => {
                    self.routing_table.lock().unwrap().set_entry(entry)
                },
                CmToPePacket::Unblock => {
                    pe_to_pe.send(S("Unblock"))?;
                },

                // encapsulated TCP
                CmToPePacket::Tcp((port_number, msg)) => {
                    let port_no = port_number.get_port_no();
                    match self.pe_to_ports.get(*port_no as usize) {
                        Some(sender) => sender.send(PeToPortPacket::Tcp(msg)).context(PacketEngineError::Chain { func_name: _f, comment: S("send TCP to port ") + self.cell_id.get_name() })?,
                        _ => return Err(PacketEngineError::Sender { func_name: _f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
                    }
                },

                // route packet, xmit to neighbor(s) or up to CModel
                CmToPePacket::Packet((user_mask, packet)) => {
                    self.route_packet(user_mask, packet)?;
                }
            };
        }
    }

    fn route_packet(&mut self, user_mask: Mask, mut packet: Packet) -> Result<(), Error> {
        let _f = "route_packet";
        let mut uuid = packet.get_tree_uuid();
        uuid.make_normal();  // Strip AIT info for lookup
        let locked = self.routing_table.lock().unwrap();    // Hold lock until forwarding is done
        let entry = locked.get_entry(uuid).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;

        match packet.get_ait_state() {
            AitState::Tick | AitState::Tock | AitState::Tack | AitState::Teck => return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into()), // Not allowed here

            AitState::Ait => { // Update state and send on ports from entry
                packet.next_ait_state()?;
                let mask = user_mask.and(entry.get_mask());
                let port_nos = mask.get_port_nos();

                {
                    let msg_type = MsgType::msg_type(&packet);
                    let tree_id = packet.get_port_tree_id();
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_leafward" };
                        let trace = json!({ "cell_id": &self.cell_id, "tree_id": &tree_id,
                            "ait_state": packet.get_ait_state(), "msg_type": &msg_type, "port_nos": &port_nos });
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if DEBUG_OPTIONS.pe_pkt_send {
                        match msg_type {
                            MsgType::Discover => (),
                            MsgType::DiscoverD => if tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, _f, port_nos, msg_type, tree_id); },
                            MsgType::StackTree => { println!("Packetengine {}: {} AIT state {}", self.cell_id, _f, packet.get_ait_state()); },
                            _ => { println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, _f, port_nos, msg_type, tree_id); }
                        }
                    }
                }
                for port_no in port_nos.iter().cloned() {
                    // I think the match version is clearer than the ok_or version
                    //self.pe_to_ports.get(*port_no as usize)
                    //    .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())?
                    //    .send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S("send packet leafward ") + self.cell_id.get_name() })?;
                   match self.pe_to_ports.get(*port_no as usize) {
                        // forward to neighbor
                        Some(s) => s.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S("send packet leafward ") + self.cell_id.get_name() })?,
                        None => return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())
                    };
                }
            }

            AitState::Normal => {
                {
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_packet_from_cm" };
                        let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": &port_tree_id, "ait_state": ait_state, "msg_type": &msg_type });
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if DEBUG_OPTIONS.pe_pkt_recv {
                        match msg_type {
                            MsgType::DiscoverD => { if port_tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: {} got from cm {} {}", self.cell_id, _f, msg_type, port_tree_id); } },
                            _ => (),
                        }
                    }
                }
                // deliver to CModel
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
        }
        Ok(())
    }

    // WORKER (PeFromPort)
    fn listen_port_loop(&mut self, pe_from_ports: &PeFromPort, pe_from_pe: &PeFromPe) -> Result<(), Error> {
        let _f = "listen_port_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = pe_from_ports.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("receive")})?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pl_recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.clone() });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            match msg {
                // deliver to CModel
                PortToPePacket::Status((port_no, is_border, status)) => {
                    self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + self.cell_id.get_name()})?
                },
                PortToPePacket::Tcp((port_no, tcp_msg)) => {
                    self.pe_to_cm.send(PeToCmPacket::Tcp((port_no, tcp_msg))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send tcp msg to ca ") + self.cell_id.get_name()})?
                },

                // recv from neighbor
                PortToPePacket::Packet((port_no, packet))  => {
                    self.process_packet(port_no, packet, pe_from_pe).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + self.cell_id.get_name()})?
                }
            };
        }
    }

    // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
    // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
    // when I block waiting for a tree update because of a deadlock with listen_cm_loop.
    fn process_packet(&mut self, port_no: PortNo, mut packet: Packet, pe_from_pe: &PeFromPe)
            -> Result<(), Error> {
        let _f = "process_packet";

        match packet.get_ait_state() {
            AitState::Ait => return Err(PacketEngineError::Ait { func_name: _f, ait_state: AitState::Ait }.into()), // Error, should never get from port

            AitState::Tock => {
                packet.next_ait_state()?;

                // Send to CM and transition to ENTL
                let channel = self.pe_to_ports.get(*port_no as usize)
                    .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())?;
                channel.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S("send packet leafward ") + self.cell_id.get_name() })?;

                packet.make_ait();
                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).or_else(|_| -> Result<(), Error> {
                    // Time reverse on error sending to CM
                    packet.make_tock();
                    packet.time_reverse();
                    channel.send(PeToPortPacket::Packet(packet))?;
                    Ok(())
                })?;
            },

            AitState::Tick => (), // Inform CM of success and enter ENTL

            AitState::Tack | AitState::Teck => {
                // Update and send back on same port
                packet.next_ait_state()?;
                let channel = self.pe_to_ports.get(*port_no as usize)
                    .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())?;
                channel.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S("send packet leafward ") + self.cell_id.get_name() })?;
            },

            AitState::Normal => { // Forward packet
                let mut uuid = packet.get_tree_uuid();
                uuid.make_normal();
                let entry = {
                    match self.routing_table.lock().unwrap().get_entry(uuid) {
                        Ok(e) => e,
                        Err(_) => {
                            // deliver to CellAgent when tree not recognized
                            self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name() })?;
                            return Ok(())
                        }
                    }
                };
                {
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_process_packet" };
                        let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": &port_tree_id, "ait_state": ait_state,
                            "msg_type": &msg_type, "port_no": &port_no, "entry": &entry });
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if DEBUG_OPTIONS.pe_process_pkt {
                        match msg_type {
                            MsgType::Discover => (),
                            MsgType::DiscoverD => if port_tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: got from {} {} {}", self.cell_id, *port_no, msg_type, port_tree_id); }
                            _ => { println!("PacketEngine {}: got from {} {} {} {}", self.cell_id, *port_no, msg_type, port_tree_id, entry); },
                        }
                    }
                }
                if entry.is_in_use() {
                    if entry.get_uuid() == packet.get_uuid() {
                        let mask = entry.get_mask();
                        port_no.make_port_number(MAX_PORTS).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("port number ") + self.cell_id.get_name() })?; // Verify that port_no is valid
                        self.forward(port_no, entry, mask, packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + self.cell_id.get_name() })?;
                    } else {
                        let msg_type = MsgType::msg_type(&packet);
                        return Err(PacketEngineError::Uuid { cell_id: self.cell_id.clone(), func_name: _f, msg_type, packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
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
    fn forward(&self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet)
            -> Result<(), Error> {
        let _f = "forward";
        if recv_port_no != entry.get_parent() {
            let parent = entry.get_parent();
            if *parent == 0 {
                if DEBUG_OPTIONS.manifest && MsgType::msg_type(&packet) == MsgType::Manifest { println!("PacketEngine {} forwarding manifest leafward mask {} entry {}", self.cell_id, user_mask, entry); };
                if DEBUG_OPTIONS.pe_pkt_send {
                    let msg_type = MsgType::msg_type(&packet);
                    match msg_type {
                        MsgType::Discover => (),
                        _ => {
                            let tree_name = packet.get_port_tree_id();
                            {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_to_cm" };
                                let trace = json!({ "cell_id": &self.cell_id, "tree_name": &tree_name, "msg_type": &msg_type, "parent_port": &parent });
                                let _ = dal::add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                            }
                            if msg_type == MsgType::Manifest { println!("PacketEngine {} forwarding manifest rootward", self.cell_id); }
                            println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, _f, *parent, msg_type, tree_name);
                        },
                    }
                }
                // deliver to CModel
                self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
            } else {
// FIXME : option logic
                if let Some(channel) = self.pe_to_ports.get((*parent) as usize) {
                    if DEBUG_OPTIONS.pe_pkt_send {
                        let msg_type = MsgType::msg_type(&packet);
                        match msg_type {
                            MsgType::Discover => (),
                            _ => {
                                let tree_name = packet.get_port_tree_id();
                                {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_rootward" };
                                    let trace = json!({ "cell_id": &self.cell_id, "tree_name": &tree_name, "msg_type": &msg_type, "parent_port": &parent });
                                    let _ = dal::add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                                }
                                println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, _f, *parent, msg_type, tree_name);
                            },
                        }
                    }

                    // forward rootward
                    channel.send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.clone()) })?;

                    // deliver to CModel
                    let is_up = entry.get_mask().and(user_mask).equal(Mask::port0());
                    if is_up {
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + self.cell_id.get_name() })?;
                    }
                } else {
                    return Err(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward rootward", port_no: *parent }.into());
                }
            }
        } else {  // Leafward
            let mask = user_mask.and(entry.get_mask());
            let port_nos = mask.get_port_nos();
            {
                let msg_type = MsgType::msg_type(&packet);
                let port_tree_id = packet.get_port_tree_id();
                if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_leafward" };
                    let trace = json!({ "cell_id": &self.cell_id, "port_tree_id": &port_tree_id, "msg_type": &msg_type, "port_nos": &port_nos });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
                if DEBUG_OPTIONS.pe_pkt_send {
                    match msg_type {
                        MsgType::Discover => (),
                        MsgType::DiscoverD => if port_tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, _f, port_nos, msg_type, port_tree_id); },
                        MsgType::Manifest => { println!("PacketEngine {} forwarding manifest leafward mask {} entry {}", self.cell_id, mask, entry); },
                        _ => { println!("PacketEngine {}: {} on {:?} {} {}", self.cell_id, _f, port_nos, msg_type, port_tree_id); }
                    };
                }
            }
            // Only side effects so use explicit loop instead of map
            for port_no in port_nos.iter().cloned() {
                if *port_no == 0 {
                    // deliver to CModel
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: _f, comment: S("leafcast packet to ca ") + self.cell_id.get_name() })?;
                } else {
                    // forward to neighbor
                    let channel = self.pe_to_ports.get(*port_no as usize)
                        .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id.clone(), func_name: "forward leaf", port_no: *port_no }.into())?;
                    channel
                        .send(PeToPortPacket::Packet(packet)).context(PacketEngineError::Chain { func_name: _f, comment: S("send packet leafward ") + self.cell_id.get_name() })?;
                }
            }
        }
        Ok(())
    }
}
impl fmt::Display for PacketEngine {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Packet Engine for cell {}", self.cell_id);
        write!(s, "{}", *self.routing_table.lock().unwrap())?;
        write!(_f, "{}", s) }
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
