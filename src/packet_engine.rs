use std::{fmt, fmt::Write,
          sync::{Arc, Mutex},
          sync::mpsc::Sender,
          collections::{HashSet, VecDeque},
          thread};

use crate::config::{CENTRAL_TREE, CONTINUE_ON_ERROR, DEBUG_OPTIONS,
                    MAX_NUM_PHYS_PORTS_PER_CELL, PAYLOAD_DEFAULT_ELEMENT, TRACE_OPTIONS,
                    ByteArray, PortNo};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{MsgType};
use crate::ec_message_formats::{PeFromCm, PeToCm,
                                PeToPort, PeFromPort, PortToPePacket, PeToPortPacket,
                                CmToPePacket, PeToCmPacket};
use crate::name::{Name, CellID, TreeID};
use crate::packet::Packet;
use crate::port::PortStatus;
use crate::routing_table::RoutingTable;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{Mask, S, TraceHeader, TraceHeaderParams, TraceType, write_err};
use crate::uuid_ec::{AitState, Uuid};

// I need one slot per port, but ports use 1-based indexing.  I could subtract 1 all the time,
// but it's safer to waste slot 0.
const MAX_SLOTS: usize = MAX_NUM_PHYS_PORTS_PER_CELL.0 as usize + 1;

type BoolArray = Arc<Mutex<[bool; MAX_SLOTS]>>;
type UsizeArray = Arc<Mutex<[usize; MAX_SLOTS]>>;
type PacketArray = Arc<Mutex<Vec<VecDeque<Packet>>>>;
type Reroute = Arc<Mutex<[PortNo; MAX_SLOTS]>>;

#[derive(Debug, Clone)]
pub struct PacketEngine {
    cell_id: CellID,
    connected_tree_uuid: Uuid,
    border_port_nos: HashSet<PortNo>,
    routing_table: Arc<Mutex<RoutingTable>>,
    no_seen_packets: UsizeArray, // Number of packets received since last packet sent
    no_sent_packets: UsizeArray, // Number of packets sent since last packet received
    sent_packets: PacketArray, // Packets that may need to be resent
    out_buffers: PacketArray,   // Packets waiting to go on the out port
    in_buffer: PacketArray,    // Packets on the in port waiting to into out_buf on the out port
    port_got_event: BoolArray,
    reroute: Reroute,
    pe_to_cm: PeToCm,
    pe_to_ports: Vec<PeToPort>,
}

impl PacketEngine {
    // NEW
    pub fn new(cell_id: CellID, connected_tree_id: TreeID, pe_to_cm: PeToCm, pe_to_ports: Vec<PeToPort>,
               border_port_nos: &HashSet<PortNo>) -> Result<PacketEngine, Error> {
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(cell_id)));
        let mut array = vec![];
        for _ in 0..MAX_SLOTS { array.push(VecDeque::new()); }
        let count = [0; MAX_SLOTS];
        Ok(PacketEngine { cell_id, connected_tree_uuid: connected_tree_id.get_uuid(),
            routing_table, border_port_nos: border_port_nos.clone(),
            no_seen_packets: Arc::new(Mutex::new(count)),
            no_sent_packets: Arc::new(Mutex::new(count)),
            sent_packets: Arc::new(Mutex::new(array.clone())),
            out_buffers: Arc::new(Mutex::new(array.clone())),
            in_buffer: Arc::new(Mutex::new(array)),
            port_got_event: Arc::new(Mutex::new([false; MAX_SLOTS])),
            reroute: Arc::new(Mutex::new([PortNo(0); MAX_SLOTS])),
            pe_to_cm, pe_to_ports })
    }

    // INIT (PeFromCm PeFromPort)
    // WORKER (PacketEngine)
    pub fn initialize(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> Result<(), Error> {
// FIXME: dal::add_to_trace mutates trace_header, spawners don't ??
        let _f = "initialize";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.listen_cm(pe_from_cm)?;
        self.listen_port(pe_from_ports)?;
        Ok(())
    }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    
    fn may_send(&self, port_no: PortNo) -> bool {
        self.port_got_event.lock().unwrap()[port_no.as_usize()]
    }
    fn set_may_not_send(&mut self, port_no: PortNo) {
        self.port_got_event.lock().unwrap()[port_no.as_usize()] = false;
    }
    fn set_may_send(&mut self, port_no: PortNo) {
        self.port_got_event.lock().unwrap()[port_no.as_usize()] = true;
    }
    fn _get_outbuf(&self, port_no: PortNo) -> VecDeque<Packet> {
        self.out_buffers.lock().unwrap().get(port_no.as_usize()).unwrap().clone()
    }
    fn get_size(array: &PacketArray, port_no: PortNo) -> usize {
        (*array.lock().unwrap())[port_no.as_usize()].len()
    }
    fn get_outbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.out_buffers, port_no)
    }
    fn _get_inbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.in_buffer, port_no)
    }
    fn _get_sent_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.sent_packets, port_no)
    }
    fn get_outbuf_first_type(&self, port_no: PortNo) -> Option<MsgType> {
        (*self.out_buffers.lock().unwrap())
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| MsgType::msg_type(packet))
    }
    fn get_outbuf_first_ait_state(&self, port_no: PortNo) -> Option<AitState> {
        (*self.out_buffers.lock().unwrap())
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| packet.get_ait_state())
    }
    fn add_to_packet_count(packet_count: &mut UsizeArray, port_no: PortNo) {
        let mut count = packet_count.lock().unwrap();
        if count.len() == 1 { // Replace 1 with PACKET_PIPELINE_SIZE when adding pipelining
            count[port_no.as_usize()] = 0;
        } else {
            count[port_no.as_usize()] = count[port_no.as_usize()] + 1;
        }
    }
    fn get_packet_count(packet_count: &UsizeArray, port_no: PortNo) -> usize {
        packet_count.lock().unwrap()[port_no.as_usize()]
    }
    fn get_no_sent_packets(&self, port_no: PortNo) -> usize {
        PacketEngine::get_packet_count(&self.no_sent_packets, port_no)
    }
    fn get_no_seen_packets(&self, port_no: PortNo) -> usize {
        PacketEngine::get_packet_count(&self.no_seen_packets, port_no)
    }
    fn add_seen_packet_count(&mut self, port_no: PortNo) {
        PacketEngine::add_to_packet_count(&mut self.no_seen_packets, port_no);
    }
    fn _clear_seen_packet_count(&mut self, port_no: PortNo) {
        self.no_seen_packets.lock().unwrap()[port_no.as_usize()] = 0;
    }
    fn add_sent_packet(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet_to_back(&mut self.sent_packets, port_no, packet);
        PacketEngine::add_to_packet_count(&mut self.no_sent_packets, port_no);
    }
    fn clear_sent_packets(&mut self, port_no: PortNo) {
        self.no_seen_packets.lock().unwrap()[port_no.as_usize()] = 0;
    }
    fn pop_first(array: &mut PacketArray, port_no: PortNo) -> Option<Packet> {
        let mut locked = array.lock().unwrap();
        let item = locked.get_mut(port_no.as_usize()).unwrap(); // Safe since vector always has MAX_NUM_PHYS_PORTS_PER_CELL entries
        item.pop_front()
    }
    fn _pop_first_outbuf(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.out_buffers, port_no)
    }
    fn _pop_first_inbuf(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.in_buffer, port_no)
    }
    fn _pop_first_sent(&mut self, port_no: PortNo) -> Option<Packet> {
        PacketEngine::pop_first(&mut self.sent_packets, port_no)
    }
    fn add_packet(to_end: bool, array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        let mut locked = array.lock().unwrap();
        let item = locked.get_mut(port_no.as_usize()).unwrap();
        if to_end { item.push_back(packet); }
        else      { item.push_front(packet); }
    }
    fn add_packet_to_front(array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(false, array, port_no, packet);
    }
    fn add_packet_to_back(array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(true, array, port_no, packet);
    }
    fn add_to_out_buffer_front(&mut self, port_no: PortNo, packet: Packet) {
        let _f = "add_to_out_buffer_front";
        PacketEngine::add_packet_to_front(&mut self.out_buffers, port_no, packet);
    }
    fn add_to_out_buffer_back(&mut self, port_no: PortNo, packet: Packet) {
        let _f = "add_to_out_buffer_back";
        PacketEngine::add_packet_to_back(&mut self.out_buffers, port_no, packet);
    }
    fn _add_to_in_buffer_back(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet_to_back(&mut self.in_buffer, port_no, packet);
    }
    fn _add_to_sent_back(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet_to_back(&mut self.sent_packets, port_no, packet);
    }
    // SPAWN THREAD (listen_cm_loop)
    fn listen_cm(&self, pe_from_cm: PeFromCm) -> Result<(), Error> {
        let _f = "listen_cm";
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {} listen_cm_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.listen_cm_loop(&pe_from_cm).map_err(|e| write_err("packet_engine", &e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_cm(pe_from_cm); }
        })?;
        Ok(())
    }

    // SPAWN THREAD (listen_port)
    // TODO: One thread for all ports; should be a different thread for each port
    fn listen_port(&self, pe_from_ports: PeFromPort)
            -> Result<(),Error> {
        let _f = "listen_port";
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {} listen_port_loop", self.cell_id);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.listen_port_loop(&pe_from_ports).map_err(|e| write_err("packet_engine", &e));
            if CONTINUE_ON_ERROR { let _ = pe.listen_port(pe_from_ports); }
        })?;
        Ok(())
    }

    // WORKER (PeFromCm)
    fn listen_cm_loop(&mut self, pe_from_cm: &PeFromCm)
            -> Result<(), Error> {
        let _f = "listen_cm_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = pe_from_cm.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("recv entry from cm ") + &self.cell_id.get_name()})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                    match &msg {
                        CmToPePacket::Packet((_, packet)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "bytes": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToPePacket::App((_, bytes)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_app" };
                            let trace = json!({ "cell_id": &self.cell_id, "bytes": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToPePacket::Entry(entry) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_entry" };
                            let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        CmToPePacket::Reroute((broken_port_no, new_parent, no_packets)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_reroute" };
                            let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port_no, "new_parent": new_parent, "no_packets": no_packets });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                }
            }
            match msg {
                // control plane from CellAgent
                CmToPePacket::Reroute((broken_port_no, new_parent, no_packets)) => {
                    self.reroute_packets(broken_port_no, new_parent, no_packets);
                },
                CmToPePacket::Entry(entry) => {
                    self.routing_table.lock().unwrap().set_entry(entry)
                },

                // encapsulated APP
                CmToPePacket::App((port_number, bytes)) => {
                    let port_no = port_number.get_port_no();
                    {
                        if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_app" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "bytes": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    match self.pe_to_ports.get(port_no.as_usize()) {
                        Some(sender) => sender.send(PeToPortPacket::App(bytes)).context(PacketEngineError::Chain { func_name: _f, comment: S("send APP to port ") + &self.cell_id.get_name() })?,
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
    fn reroute_packets(&mut self, broken_port_no: PortNo, new_parent: PortNo, no_packets: NumberOfPackets) {
        let _f = "reroute_packets";
        {
            let mut locked_reroute = self.reroute.lock().unwrap();
            locked_reroute[broken_port_no.as_usize()] = new_parent;
            //println!("PacketEngine {}: {} broken port {} to {:?}", self.cell_id, _f, *broken_port_no, *locked_reroute);
        }
        let mut locked_outbuf = self.out_buffers.lock().unwrap();
        let mut locked_sent = self.sent_packets.lock().unwrap();
        let sent_buf = &mut locked_sent[broken_port_no.as_usize()];
        let no_my_sent_packets = self.get_no_sent_packets(broken_port_no);
        let no_her_seen_packets = no_packets.get_number_seen();
        let no_resend = no_my_sent_packets - no_her_seen_packets;
        let mut remaining_sent = sent_buf.split_off(no_resend);
        let broken_outbuf = &mut locked_outbuf[broken_port_no.as_usize()].clone();
        let new_parent_outbuf = &mut locked_outbuf[new_parent.as_usize()];
        new_parent_outbuf.append(&mut remaining_sent);
        new_parent_outbuf.append(broken_outbuf);
    }
    fn route_cm_packet(&mut self, user_mask: Mask, packet: Packet) -> Result<(), Error> {
        let _f = "route_cm_packet";
        let uuid = packet.get_tree_uuid().for_lookup();  // Strip AIT info for lookup
        let entry = {
            let locked = self.routing_table.lock().unwrap();
            locked.get_entry(uuid).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?
        };

        match packet.get_ait_state() {
            AitState::AitD |
            AitState::Entl |
            AitState::Tick |
            AitState::Tock |
            AitState::Tack |
            AitState::Teck => return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into()), // Not allowed here

            AitState::Normal |
            AitState::Ait => {
                { // Debug block
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    {
                        if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_cm {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_packet_from_cm" };
                            let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state, "msg": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        if DEBUG_OPTIONS.pe_pkt_recv {
                            match msg_type {
                                MsgType::Manifest => println!("PacketEngine {}: {} got from cm {} {}", self.cell_id, _f, msg_type, user_mask),
                                _ => (),
                            }
                        }
                    }
                }
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, &packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
        }
        Ok(())
    }
    fn send_packet(&self, port_no: PortNo, packet: &Packet) -> Result<(), Error> {
        let _f = "send_packet";
        let mut reroute_port_no = self.reroute.lock().unwrap()[port_no.as_usize()];
        if reroute_port_no == PortNo(0) {
            reroute_port_no = port_no;
        } else {
            let mut locked_outbuf = self.out_buffers.lock().unwrap();
            let broken_outbuf = &mut locked_outbuf[port_no.as_usize()];
            if broken_outbuf.len() > 0 {
                // Only clone if there are packets in the broken out buffer
                let broken_outbuf = &mut locked_outbuf[port_no.as_usize()].clone();
                let reroute_outbuf = &mut locked_outbuf[reroute_port_no.as_usize()];
                reroute_outbuf.append(broken_outbuf);
            }
        }
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_packet" };
                let trace = json!({ "cell_id": self.cell_id, "port_no": reroute_port_no, "msg": packet.to_string()? });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.pe_to_ports.get(reroute_port_no.as_usize())
            .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: *reroute_port_no }.into())?
            .send(PeToPortPacket::Packet(packet.clone()))?;
        Ok(())
    }
    fn send_packet_flow_control(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_packet";
        if self.may_send(port_no) {
            if let Some(packet) = self._pop_first_outbuf(port_no) {
                self.set_may_not_send(port_no);
                {
                    if DEBUG_OPTIONS.all || DEBUG_OPTIONS.flow_control {
                        let msg_type = MsgType::msg_type(&packet);
                        match packet.get_ait_state() {
                            AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} {} {}", self.cell_id, *port_no, _f, self.get_outbuf_size(port_no), msg_type, packet.get_ait_state()),
                            _ => ()
                        }
                    }
                }
                match packet.get_ait_state() {
                    AitState::Entl => self.set_may_send(port_no),
                    _              => self.set_may_not_send(port_no)
                }
                self.send_packet(port_no, &packet)?;
                self.add_sent_packet(port_no, packet);
            }
        } else { // Debug only
            {
                if DEBUG_OPTIONS.all || DEBUG_OPTIONS.flow_control {
                    match self.get_outbuf_first_ait_state(port_no) {
                        Some(ait_state) => match ait_state {
                            AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} {:?} {:?}", self.cell_id, *port_no, _f, self.get_outbuf_size(port_no), self.get_outbuf_first_type(port_no), self.get_outbuf_first_ait_state(port_no)),
                            _ => ()
                        },
                        None => ()
                    }
                }
            }
        }
        Ok(())
    }
    fn send_next_packet_or_entl(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_next_packet_or_entl";
        // TOCTTOU race here, but the only cost is sending an extra ENTL packet
        if self.get_outbuf_size(port_no) == 0 {
            self.add_to_out_buffer_back(port_no, Packet::make_entl_packet());
        }
        self.send_packet_flow_control(port_no)
    }
    // WORKER (PeFromPort)
    fn listen_port_loop(&mut self, pe_from_ports: &PeFromPort) -> Result<(), Error> {
        let _f = "listen_port_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = pe_from_ports.recv().context(PacketEngineError::Chain { func_name: _f, comment: S("pe from packet")})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                    match &msg {
                        PortToPePacket::Packet((_, packet)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "msg": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        PortToPePacket::App((_, bytes)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_app" };
                            let trace = json!({ "cell_id": &self.cell_id, "msg": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        PortToPePacket::Status((port_no, is_border, port_status)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_status" };
                            let trace = json!({ "cell_id": &self.cell_id,  "port": port_no, "is_border": is_border, "status": port_status});
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    };
                }
            }
            match msg {
                // deliver to CModel
                PortToPePacket::Status((port_no, is_border, status)) => {
                    let number_of_packets = NumberOfPackets {
                        sent: self.get_no_sent_packets(port_no),
                        recd: self.get_no_seen_packets(port_no)
                    };
                    match status {
                        PortStatus::Connected    => self.set_may_send(port_no),
                        PortStatus::Disconnected => self.set_may_not_send(port_no)
                    }
                    {
                        if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_status" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, number_of_packets, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + &self.cell_id.get_name()})?
                },
                PortToPePacket::App((port_no, bytes)) => {
                    {
                        if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_app" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "bytes": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.pe_to_cm.send(PeToCmPacket::App((port_no, bytes))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send app msg to ca ") + &self.cell_id.get_name()})?
                },

                // recv from neighbor
                PortToPePacket::Packet((port_no, packet))  => {
                    self.process_packet(port_no, packet).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + &self.cell_id.get_name()})?
                }
            };
        }
    }

    // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
    // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
    // when I block waiting for a tree update because of a deadlock with listen_cm_loop.
    fn process_packet(&mut self, port_no: PortNo, packet: Packet)
            -> Result<(), Error> {
        let _f = "process_packet";
        // Got a packet from the other side, so clear state
        self.set_may_send(port_no);
        self.add_seen_packet_count(port_no);
        self.clear_sent_packets(port_no);
        {
            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.flow_control {
                let msg_type = MsgType::msg_type(&packet);
                match packet.get_ait_state() {
                    AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} msg type {} {}", self.cell_id, *port_no, _f, self.get_outbuf_size(port_no), msg_type, packet.get_ait_state()),
                    _ => ()
                }
            }
        }
        match packet.get_ait_state() {
            AitState::Teck |
            AitState::Tack |
            AitState::Tock |
            AitState::Tick => {
                self.send_next_packet_or_entl(port_no)?; // Don't lock up the port on an error
                return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into())
            },
            AitState::Entl => self.send_packet_flow_control(port_no)?,
            AitState::Ait  => {
                {
                    if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "packet": packet.to_string()? });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet)))?
            },
            AitState::AitD => (), // TODO: Send to cm once cell agent knows how to handle it
            AitState::Normal => { // Forward packet
                let uuid = packet.get_tree_uuid().for_lookup();
                let entry =
                    { // Using this block releases the lock on the routing table
                        match self.routing_table.lock().unwrap().get_entry(uuid) {
                            Ok(e) => e,
                            Err(err) => {
                                // deliver to CellAgent when tree not recognized
                                {
                                    if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_err" };
                                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "err": err.to_string(), "packet": packet.to_string()? });
                                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + &self.cell_id.get_name() })?;
                                return Ok(())
                            }
                        }
                    };
                { // Debug block
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    {
                        if DEBUG_OPTIONS.all | DEBUG_OPTIONS.pe_process_pkt {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_process_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state,
                            "msg_type": &msg_type, "port_no": &port_no, "entry": &entry });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                            match msg_type {
                                MsgType::Discover => (),
                                MsgType::DiscoverD => if port_tree_id.is_name(CENTRAL_TREE) { println!("PacketEngine {}: got from {} {} {}", self.cell_id, *port_no, msg_type, port_tree_id); }
                                _ => { println!("PacketEngine {}: got from {} {} {} {}", self.cell_id, *port_no, msg_type, port_tree_id, entry); },
                            }
                        }
                    }
                }
                if entry.is_in_use() {
                    if entry.get_uuid() == uuid {
                        // Put packets on the right port's queue
                        let mask = entry.get_mask();
                        self.forward(port_no, entry, mask, &packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + &self.cell_id.get_name() })?;
                    } else {
                        let msg_type = MsgType::msg_type(&packet);
                        return Err(PacketEngineError::Uuid { cell_id: self.cell_id, func_name: _f, msg_type, packet_uuid: packet.get_tree_uuid(), table_uuid: entry.get_uuid() }.into());
                    }
                    // Send the packet at the head of the port's queue
                    self.send_next_packet_or_entl(port_no)?;
                }
            }
        }
        Ok(())
    }
    fn forward(&mut self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet_ref: &Packet)
            -> Result<(), Error> {
        let _f = "forward";
        let packet = packet_ref.clone();
        if packet.get_tree_uuid().for_lookup() == self.connected_tree_uuid {
           // Send with CA flow control (currently none)
            let mask = user_mask.and(entry.get_mask());
            let port_nos = mask.get_port_nos();
            for port_no in port_nos.into_iter() {
                self.send_packet(port_no, packet_ref)?;
            }
        } else {
            if recv_port_no != entry.get_parent() {
                // Send to root if recv port is not parent
                let parent = entry.get_parent();
                if *parent == 0 {
                    {
                        if DEBUG_OPTIONS.all || DEBUG_OPTIONS.pe_pkt_send {
                            let msg_type = MsgType::msg_type(&packet);
                            match msg_type {
                                MsgType::Discover => (),
                                _ => {
                                    let tree_name = packet.get_port_tree_id();
                                    {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe to cm rootward" };
                                        let trace = json!({ "cell_id": self.cell_id, "tree_name": &tree_name, "msg_type": &msg_type, "parent_port": &parent });
                                        let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                                    }
                                    if msg_type == MsgType::Manifest { println!("PacketEngine {} forwarding manifest rootward", self.cell_id); }
                                    println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, _f, *parent, msg_type, tree_name);
                                },
                            }
                        }
                    }
                    // deliver to CModel
                    {
                        if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
                } else {
                    // Forward rootward
                    self.add_to_out_buffer_back(parent, packet);
                    self.send_next_packet_or_entl(entry.get_parent())?;
                }
            } else {
                // Send leafward if recv port is parent
                let mask = user_mask.and(entry.get_mask());
                let port_nos = mask.get_port_nos();
                { // Debug block
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe to cm leafward" };
                        let trace = json!({ "cell_id": self.cell_id, "port_tree_id": &port_tree_id, "port_nos": &port_nos, "msg": packet.to_string()? });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if DEBUG_OPTIONS.all || DEBUG_OPTIONS.pe_pkt_send {
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
                        {
                            if TRACE_OPTIONS.all | TRACE_OPTIONS.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet" };
                                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone()))).context(PacketEngineError::Chain { func_name: _f, comment: S("leafcast packet to ca ") + &self.cell_id.get_name() })?;
                    } else {
                        // forward to neighbor
                        {
                            if DEBUG_OPTIONS.all || DEBUG_OPTIONS.flow_control {
                                let msg_type = MsgType::msg_type(&packet);
                                match packet.get_ait_state() {
                                    AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} msg type {} {}", self.cell_id, *port_no, _f, self.get_outbuf_size(port_no), msg_type, packet.get_ait_state()),
                                    _ => ()
                                }
                            }
                        }
                        self.add_to_out_buffer_back(port_no, packet.clone());
                        self.send_next_packet_or_entl(port_no)?;
                    }
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
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NumberOfPackets {
    sent: usize,
    recd: usize
}
impl NumberOfPackets {
    pub fn new() -> NumberOfPackets { NumberOfPackets { sent: 0, recd: 0 }}
    pub fn _get_number_sent(&self) -> usize { self.sent }
    pub fn get_number_seen(&self) -> usize { self.recd }
}
impl fmt::Display for NumberOfPackets {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(_f, "Number sent {}, Number received {}", self.sent, self.recd)
    }
}
// Errors
use failure::{Error, ResultExt};

#[derive(Debug, Fail)]
pub enum PacketEngineError {
    #[fail(display = "PacketEngineError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
    //#[fail(display = "PacketEngineError::Buffer {} no room for packet in {}", func_name, buffer_name)]
    //Buffer { func_name: &'static str, buffer_name: &'static str },
    #[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "PacketEngineError::Sender {}: No sender for port {} on cell {}", func_name, port_no, cell_id)]
    Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, table_uuid: Uuid, packet_uuid: Uuid }
}
