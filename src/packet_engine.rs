use std::{fmt, fmt::Write,
          sync::{Arc, Mutex},
          sync::mpsc::channel,
          collections::{HashSet},
          thread};

use crate::config::{CENTRAL_TREE, CONTINUE_ON_ERROR, DEBUG_OPTIONS,
                    MAX_PORTS, TRACE_OPTIONS, PortNo};
use crate::dal;
use crate::dal::{fork_trace_header, update_trace_header};
use crate::message::MsgType;
use crate::message_types::{PeFromCm, PeToCm,
                    PeToPort, PeFromPort, PortToPePacket, PeToPortPacket,
                    PeToPe, PeFromPe, CmToPePacket, PeToCmPacket};
use crate::name::{Name, CellID};
use crate::packet::{Packet};
use crate::port::PortStatus;
use crate::routing_table::{RoutingTable};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{Mask, S, TraceHeader, TraceHeaderParams, TraceType, write_err};
use crate::uuid_ec::{AitState, Uuid};

type BoolArray = Arc<Mutex<Vec<bool>>>;
type UsizeArray = Arc<Mutex<Vec<usize>>>;
type PacketArray = Arc<Mutex<Vec<Vec<Packet>>>>;

#[derive(Debug, Clone)]
pub struct PacketEngine {
    cell_id: CellID,
    boundary_port_nos: HashSet<PortNo>,
    routing_table: Arc<Mutex<RoutingTable>>,
    no_seen_packets: UsizeArray, // Number of packets received since last packet sent
    no_sent_packets: UsizeArray, // Number of packets sent since last packet received
    sent_packets: PacketArray, // Packets that may need to be resent
    out_buffer: PacketArray,   // Packets waiting to go on the out port
    in_buffer: PacketArray,    // Packets on the in port waiting to into out_buf on the out port
    port_got_event: BoolArray,
    pe_to_cm: PeToCm,
    pe_to_ports: Vec<PeToPort>,
}

impl PacketEngine {
    //pub fn get_table(&self) -> &Arc<Mutex<RoutingTable>> { &self.routing_table }
    // NEW
    pub fn new(cell_id: CellID, pe_to_cm: PeToCm, pe_to_ports: Vec<PeToPort>,
            boundary_port_nos: HashSet<PortNo>) -> Result<PacketEngine, Error> {
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id).context(PacketEngineError::Chain { func_name: "new", comment: S(cell_id.get_name())})?));
        let mut array = vec![];
        for i in 0..MAX_PORTS.0 as usize { array.push(vec![]); }
        let count_vec = [0; MAX_PORTS.0 as usize].to_vec();
        Ok(PacketEngine { cell_id, routing_table, boundary_port_nos,
            no_seen_packets: Arc::new(Mutex::new(count_vec.clone())),
            no_sent_packets: Arc::new(Mutex::new(count_vec.clone())),
            sent_packets: Arc::new(Mutex::new(array.clone())),
            out_buffer: Arc::new(Mutex::new(array.clone())),
            in_buffer: Arc::new(Mutex::new(array.clone())),
            port_got_event: Arc::new(Mutex::new([false; MAX_PORTS.0 as usize].to_vec())),
            pe_to_cm, pe_to_ports })
    }

    // INIT (PeFromCm PeFromPort)
    // WORKER (PacketEngine)
    pub fn initialize(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> Result<(), Error> {
// FIXME: dal::add_to_trace mutates trace_header, spawners don't ??
        let _f = "initialize";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        let (pe_to_pe, pe_from_pe): (PeToPe, PeFromPe) = channel();
        self.listen_cm(pe_from_cm, pe_to_pe)?;
        self.listen_port(pe_from_ports, pe_from_pe)?;
        Ok(())
    }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    
    fn can_send(&self, port_no: PortNo) -> bool {
        self.port_got_event.lock().unwrap()[port_no.as_usize()]
    }
    fn set_cannot_send(&mut self, port_no: PortNo) {
        self.port_got_event.lock().unwrap()[port_no.as_usize()] = false;
    }
    fn set_can_send(&mut self, port_no: PortNo) {
        self.port_got_event.lock().unwrap()[port_no.as_usize()] = true;
    }
    fn get_size(array: &PacketArray, port_no: PortNo) -> usize {
        (*array.lock().unwrap())[port_no.as_usize()].len()
    }
    fn get_outbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.out_buffer, port_no)
    }
    fn get_outbuf_first_type(&self, port_no: PortNo) -> Option<MsgType> {
        (*self.out_buffer.lock().unwrap())
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| MsgType::msg_type(packet))
    }
    fn get_outbuf_first_ait_state(&self, port_no: PortNo) -> Option<AitState> {
        (*self.out_buffer.lock().unwrap())
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| packet.get_ait_state())
    }
    fn get_inbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.in_buffer, port_no)
    }
    fn get_sent_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.sent_packets, port_no)
    }
    fn add_seen_packet(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_to_packet_count(&mut self.no_seen_packets, port_no);
    }
    fn clear_see_packets(&mut self, port_no: PortNo) {
        self.no_seen_packets.lock().unwrap()[port_no.as_usize()] = 0;
    }
    fn add_sent_packet(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(&mut self.sent_packets, port_no, packet);
        PacketEngine::add_to_packet_count(&mut self.no_sent_packets, port_no);
    }
    fn clear_sent_packets(&mut self, port_no: PortNo) {
        self.no_seen_packets.lock().unwrap()[port_no.as_usize()] = 0;
    }
    fn pop_first(array: &mut PacketArray, port_no: PortNo) -> Option<Packet> {
        let clone = array.lock().unwrap().clone();
        let item = clone.get(port_no.as_usize()).unwrap(); // Safe since vector always has MAX_PORTS entries
        if item.len() > 0 {
            let (car, cdr) = item.split_at(1); // Will panic on an empty vector
            array.lock().unwrap()[port_no.as_usize()] = cdr.to_vec();
            Some(car[0])
        } else {
            None
        }
    }
    fn pop_first_outbuf(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.out_buffer, port_no)
    }
    fn pop_first_inbuf(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.in_buffer, port_no)
    }
    fn pop_first_sent(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.sent_packets, port_no)
    }
    fn add_packet(array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        let mut locked = array.lock().unwrap();
        let item = locked.get_mut(port_no.as_usize()).unwrap();
        item.push(packet);
    }
    fn add_to_out_buffer(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(&mut self.out_buffer, port_no, packet);
    }
    fn add_to_in_buffer(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(&mut self.in_buffer, port_no, packet);
    }
    fn add_to_sent(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(&mut self.sent_packets, port_no, packet);
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
            let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = pe_from_cm.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("recv entry from cm ") + &self.cell_id.get_name()})?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": self.cell_id, "msg": &msg });
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
                    match self.pe_to_ports.get(port_no.as_usize()) {
                        Some(sender) => sender.send(PeToPortPacket::Tcp(msg)).context(PacketEngineError::Chain { func_name: _f, comment: S("send TCP to port ") + &self.cell_id.get_name() })?,
                        _ => return Err(PacketEngineError::Sender { func_name: _f, cell_id: self.cell_id, port_no: *port_no }.into())
                    }
                },

                // route packet, xmit to neighbor(s) or up to CModel
                CmToPePacket::Packet((user_mask, packet)) => {
                    self.route_cm_packet(user_mask, packet)?;
                }
            };
        }
    }

    fn route_cm_packet(&mut self, user_mask: Mask, mut packet: Packet) -> Result<(), Error> {
        let _f = "route_packet";
        let uuid = packet.get_tree_uuid().for_lookup();  // Strip AIT info for lookup
        let entry = {
            let locked = self.routing_table.lock().unwrap();
            locked.get_entry(uuid).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?
        };

        match packet.get_ait_state() {
            AitState::Entl |
            AitState::Tick |
            AitState::Tock |
            AitState::Tack |
            AitState::Teck => return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into()), // Not allowed here

            AitState::Ait => { // Update state and send on ports from entry
                packet.next_ait_state()?;
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
            AitState::Normal => {
                {
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_packet_from_cm" };
                        let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state, "msg_type": &msg_type });
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if DEBUG_OPTIONS.pe_pkt_recv {
                        match msg_type {
                            MsgType::DiscoverD => { if port_tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: {} got from cm {} {}", self.cell_id, _f, msg_type, port_tree_id); } },
                            _ => (),
                        }
                    }
                }
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
        }
        Ok(())
    }
    fn send_packet(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_packet";
        if self.can_send(port_no) {
            if let Some(packet) = self.pop_first_outbuf(port_no) {
                //let msg_type = MsgType::msg_type(&packet);
                //println!("PacketEngine {}: {} port {} msg type {} {}", self.cell_id, _f, *port_no, msg_type, packet.get_ait_state());
                self.pe_to_ports.get(port_no.as_usize())
                    .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: *port_no }.into())?
                    .send(PeToPortPacket::Packet(packet))?;
                match packet.get_ait_state() {
                    AitState::Entl => self.set_can_send(port_no),
                    // Use the next line to turn on flow control or the one after to turn it off
                    //_              => self.set_cannot_send(port_no)
                    _              => self.set_can_send(port_no)
                }
            } else {
                self.send_next_packet_or_entl(port_no)?;
            }
        } else {
            //println!("PacketEngine {}: {} port {} out buf size {} {:?} {:?}", self.cell_id, _f, *port_no, self.get_outbuf_size(port_no), self.get_outbuf_first_type(port_no), self.get_outbuf_first_ait_state(port_no));
        }
        Ok(())
    }
    fn send_next_packet_or_entl(&mut self, port_no: PortNo) -> Result<(), Error> {
        if self.get_outbuf_size(port_no) == 0 {
            self.add_to_out_buffer(port_no, Packet::make_entl_packet());
        }
        self.send_packet(port_no)
    }
    // WORKER (PeFromPort)
    fn listen_port_loop(&mut self, pe_from_ports: &PeFromPort, pe_from_pe: &PeFromPe) -> Result<(), Error> {
        let _f = "listen_port_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = pe_from_ports.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("receive")})?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pl_recv" };
                let trace = json!({ "cell_id": self.cell_id, "msg": &msg });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            match msg {
                // deliver to CModel
                PortToPePacket::Status((port_no, is_border, status)) => {
                    match status {
                        PortStatus::Connected    => self.set_can_send(port_no),
                        PortStatus::Disconnected => self.set_cannot_send(port_no)
                    }
                    self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + &self.cell_id.get_name()})?
                },
                PortToPePacket::Tcp((port_no, tcp_msg)) => {
                    self.pe_to_cm.send(PeToCmPacket::Tcp((port_no, tcp_msg))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send tcp msg to ca ") + &self.cell_id.get_name()})?
                },

                // recv from neighbor
                PortToPePacket::Packet((port_no, packet))  => {
                    self.process_packet(port_no, packet, pe_from_pe).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + &self.cell_id.get_name()})?
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
    
        self.set_can_send(port_no);
        match packet.get_ait_state() {
            AitState::Ait => return Err(PacketEngineError::Ait { func_name: _f, ait_state: AitState::Ait }.into()), // Error, should never get from port
            AitState::Tock => {
                // Send to CM and transition to ENTL if time is FORWARD
                //packet.next_ait_state()?;
                //self.add_to_out_buffer(port_no, packet);
                //self.send_packet(port_no)?;
                packet.make_ait();
                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet)))
                    .or_else(|_| -> Result<(), Error> {
                        // Time reverse on error sending to CM
                        println!("PacketEngine {}: {} error {} {}", self.cell_id, _f, MsgType::msg_type(&packet), packet.get_ait_state());
                        packet.make_tock();
                        packet.time_reverse();
                        self.add_to_out_buffer(port_no, packet);
                        self.send_packet(port_no)?;
                        Ok(())
                    })?;
                self.send_next_packet_or_entl(port_no)?;
            },

            AitState::Tick => { // Inform CM of success and enter ENTL
                // TODO: Handle error sending to cm
                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet)))?;
                self.send_next_packet_or_entl(port_no)?;
            },

            AitState::Tack | AitState::Teck => {
                // Update and send back on same port
                packet.next_ait_state()?;
                self.add_to_out_buffer(port_no, packet);
                self.send_packet(port_no)?;
            },
            AitState::Entl => { // In real life, send an ENTL
                // Next line only belongs in TICK, but I'm not using that state in the simulator
                //self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet)))?;
                if self.get_outbuf_size(port_no) > 0 { self.send_packet(port_no)?; }
            },
            AitState::Normal => { // Forward packet
                let uuid = packet.get_tree_uuid().for_lookup();
                let entry = { // Using this block releases the lock on the routing table
                    match self.routing_table.lock().unwrap().get_entry(uuid) {
                        Ok(e) => e,
                        Err(_) => {
                            // deliver to CellAgent when tree not recognized
                            self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + &self.cell_id.get_name() })?;
                            return Ok(())
                        }
                    }
                };
                {
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    {
                        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_process_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state,
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
                }
                if entry.is_in_use() {
                    if entry.get_uuid() == packet.get_uuid() {
                        let mask = entry.get_mask();
                        self.forward(port_no, entry, mask, packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + &self.cell_id.get_name() })?;
                    } else {
                        let msg_type = MsgType::msg_type(&packet);
                        return Err(PacketEngineError::Uuid { cell_id: self.cell_id, func_name: _f, msg_type, packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
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
    fn forward(&mut self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet: Packet)
            -> Result<(), Error> {
        let _f = "forward";
        if recv_port_no != entry.get_parent() {
            // Send to root if recv port is not parent
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
                                let trace = json!({ "cell_id": self.cell_id, "tree_name": &tree_name, "msg_type": &msg_type, "parent_port": &parent });
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
                // Forward rootward
                self.add_to_out_buffer(parent, packet);
                self.send_packet(entry.get_parent())?;
        
                // deliver to CModel if rootward, but current architecture is root
                //let is_up = entry.get_mask().and(user_mask).equal(Mask::port0());
                //if is_up {
                //    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + &self.cell_id.get_name() })?;
                //}
            }
        } else {
            // Send leafward if recv port is parent
            let mask = user_mask.and(entry.get_mask());
            let port_nos = mask.get_port_nos();
            {
                let msg_type = MsgType::msg_type(&packet);
                let port_tree_id = packet.get_port_tree_id();
                if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_leafward" };
                    let trace = json!({ "cell_id": self.cell_id, "port_tree_id": &port_tree_id, "msg_type": &msg_type, "port_nos": &port_nos });
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
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: _f, comment: S("leafcast packet to ca ") + &self.cell_id.get_name() })?;
                } else {
                    // forward to neighbor
                    self.add_to_out_buffer(port_no, packet);
                    self.send_packet(port_no)?;
                }
            }
        }
        Ok(())
    }
    fn add_to_packet_count(packet_count: &mut UsizeArray, port_no: PortNo) {
        let mut count = packet_count.lock().unwrap();
        if count.len() == 1 { // Replace 1 with PACKET_PIPELINE_SIZE when adding pipelining
            count[port_no.as_usize()] = 0;
        } else {
            count[port_no.as_usize()] = count[port_no.as_usize()] + 1;
        }
    }
}
impl fmt::Display for PacketEngine {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Packet Engine for cell {}", self.cell_id);
        write!(s, "{}", *self.routing_table.lock().unwrap())?;
        write!(_f, "{}", s) }
}
#[derive(Debug, Clone, Default)]
struct MyVec<T> { v: Vec<T> }
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PacketEngineError {
    #[fail(display = "PacketEngineError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
    #[fail(display = "PacketEngineError::Buffer {} no room for packet in {}", func_name, buffer_name)]
    Buffer { func_name: &'static str, buffer_name: &'static str },
    #[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "PacketEngineError::Sender {}: No sender for port {} on cell {}", func_name, port_no, cell_id)]
    Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, table_uuid: Uuid, packet_uuid: Uuid }
}
