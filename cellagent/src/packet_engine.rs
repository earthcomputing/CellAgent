use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet, VecDeque},
          sync::{Arc, Mutex},
          thread,
          thread::JoinHandle,
          };

use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{MsgType};
use crate::ec_message_formats::{PeFromCm, PeToCm,
                                PeToPort, PeFromPort, PortToPePacket,
                                CmToPePacket, PeToCmPacket};
use crate::name::{Name, CellID, TreeID};
use crate::packet::Packet;
use crate::port::PortStatus;
use crate::routing_table::RoutingTable;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{Mask, PortNo, S, TraceHeaderParams, TraceType, write_err };
use crate::uuid_ec::{AitState, Uuid};

// I need one slot per port, but ports use 1-based indexing.  I could subtract 1 all the time,
// but it's safer to waste slot 0.
// TODO: Use Config.max_num_phys_ports_per_cell
const MAX_SLOTS: usize = 10; //MAX_NUM_PHYS_PORTS_PER_CELL.0 as usize + 1;

type BoolArray = [bool; MAX_SLOTS];
type UsizeArray = [usize; MAX_SLOTS];
type PacketArray = Vec<VecDeque<Packet>>;
type Reroute = [PortNo; MAX_SLOTS];

#[derive(Debug, Clone)]
pub struct PacketEngine {
    cell_id: CellID,
    connected_tree_uuid: Uuid,
    border_port_nos: HashSet<PortNo>,
    routing_table: RoutingTable,
    routing_table_mutex: Arc<Mutex<RoutingTable>>,  // So I can show the routing table on the console
    no_seen_packets: UsizeArray, // Number of packets received since last packet sent
    no_sent_packets: UsizeArray, // Number of packets sent since last packet received
    sent_packets: PacketArray, // Packets that may need to be resent
    out_buffers: PacketArray,   // Packets waiting to go on the out port
    in_buffer: PacketArray,    // Packets on the in port waiting to into out_buf on the out port
    port_got_event: BoolArray,
    reroute: Reroute,
    pe_to_cm: PeToCm,
    pe_to_ports: HashMap<PortNo, PeToPort>,
}

impl PacketEngine {
    // NEW
    pub fn new(cell_id: CellID, connected_tree_id: TreeID, pe_to_cm: PeToCm,
               pe_to_ports: HashMap<PortNo, PeToPort>,
               border_port_nos: &HashSet<PortNo>) -> PacketEngine {
        let routing_table = RoutingTable::new(cell_id);
        let routing_table_mutex = Arc::new(Mutex::new(routing_table.clone()));
        let mut array = vec![];
        for _ in 0..MAX_SLOTS { array.push(VecDeque::new()); }
        let count = [0; MAX_SLOTS];
        PacketEngine {
            cell_id,
            connected_tree_uuid: connected_tree_id.get_uuid(),
            routing_table,
            routing_table_mutex,  // Needed so I can print the routing table from main
            border_port_nos: border_port_nos.clone(),
            no_seen_packets: count,
            no_sent_packets: count,
            sent_packets: array.clone(),
            out_buffers: array.clone(),
            in_buffer: array,
            port_got_event: [false; MAX_SLOTS],
            reroute: [PortNo(0); MAX_SLOTS],
            pe_to_cm,
            pe_to_ports,
        }
    }
    
    // SPAWN THREAD (pe.initialize)
    pub fn start(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> JoinHandle<()> {
        let _f = "start_packet_engine";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_start_pe" };
                let trace = json!({ "cell_id": self.get_cell_id() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {}", self.get_cell_id());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.initialize(pe_from_cm, pe_from_ports).map_err(|e| write_err("nalcell", &e));
            if CONFIG.continue_on_error { } // Don't automatically restart packet engine if it crashes
        }).expect("thread failed")
    }

    // INIT (PeFromCm PeFromPort)
    // WORKER (PacketEngine)
    pub fn initialize(&mut self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> Result<(), Error> {
        let _f = "initialize";
        loop {
            select! {
                recv(pe_from_cm) -> recvd => {
                    let msg = recvd.context(PacketEngineError::Chain { func_name: _f, comment: S("pe from cm") })?;
                    self.listen_cm(msg).context(PacketEngineError::Chain { func_name: _f, comment: S("listen cm") })?;
                },
                recv(pe_from_ports) -> recvd => {
                    let msg = recvd.context(PacketEngineError::Chain { func_name: _f, comment: S("pe from port") })?;
                    self.listen_port(msg).context(PacketEngineError::Chain { func_name: _f, comment: S("listen port") })?;
                }
            }
        }
    }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    
    fn may_send(&self, port_no: PortNo) -> bool {
        self.port_got_event[port_no.as_usize()]
    }
    fn set_may_not_send(&mut self, port_no: PortNo) {
        self.port_got_event[port_no.as_usize()] = false;
    }
    fn set_may_send(&mut self, port_no: PortNo) {
        self.port_got_event[port_no.as_usize()] = true;
    }
    fn _get_outbuf(&self, port_no: PortNo) -> VecDeque<Packet> {
        self.out_buffers.get(port_no.as_usize()).unwrap().clone()
    }
    fn get_size(array: &PacketArray, port_no: PortNo) -> usize {
        (*array)[port_no.as_usize()].len()
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
        (*self.out_buffers)
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| MsgType::msg_type(packet))
    }
    fn get_outbuf_first_ait_state(&self, port_no: PortNo) -> Option<AitState> {
        (*self.out_buffers)
            .get(port_no.as_usize())
            .unwrap()
            .get(0)
            .map(|packet| packet.get_ait_state())
    }
    fn add_to_packet_count(packet_count: &mut UsizeArray, port_no: PortNo) {
         if packet_count.len() == 1 { // Replace 1 with PACKET_PIPELINE_SIZE when adding pipelining
            packet_count[port_no.as_usize()] = 0;
        } else {
            packet_count[port_no.as_usize()] = packet_count[port_no.as_usize()] + 1;
        }
    }
    fn get_packet_count(packet_count: &UsizeArray, port_no: PortNo) -> usize {
        packet_count[port_no.as_usize()]
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
        self.no_seen_packets[port_no.as_usize()] = 0;
    }
    fn add_sent_packet(&mut self, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet_to_back(&mut self.sent_packets, port_no, packet);
        PacketEngine::add_to_packet_count(&mut self.no_sent_packets, port_no);
    }
    fn clear_sent_packets(&mut self, port_no: PortNo) {
        self.no_seen_packets[port_no.as_usize()] = 0;
    }
    fn pop_first(array: &mut PacketArray, port_no: PortNo) -> Option<Packet> {
        let item = array.get_mut(port_no.as_usize()).unwrap(); // Safe since vector always has MAX_NUM_PHYS_PORTS_PER_CELL entries
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
        let item = array.get_mut(port_no.as_usize()).unwrap();
        if to_end { item.push_back(packet); } else { item.push_front(packet); }
    }
    fn add_packet_to_front(array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(false, array, port_no, packet);
    }
    fn add_packet_to_back(array: &mut PacketArray, port_no: PortNo, packet: Packet) {
        PacketEngine::add_packet(true, array, port_no, packet);
    }
    #[allow(dead_code)]
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
    fn listen_cm(&mut self, msg: CmToPePacket) -> Result<(), Error> {
        let _f = "listen_cm";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                match &msg {
                    CmToPePacket::Packet((user_mask, packet)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "user_mask": user_mask, "packet": packet.to_string()? });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CmToPePacket::Delete(uuid) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
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
            CmToPePacket::Delete(uuid) => {
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.pe_process_pkt {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_delete_entry_dbg" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
                        let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                    }
                }
                self.routing_table.delete_entry(uuid);
                (*self.routing_table_mutex.lock().unwrap()) = self.routing_table.clone();
            }
            CmToPePacket::Entry(entry) => {
                self.routing_table.set_entry(entry);
                (*self.routing_table_mutex.lock().unwrap()) = self.routing_table.clone();
            },
        
            // route packet, xmit to neighbor(s) or up to CModel
            CmToPePacket::Packet((user_mask, packet)) => {
                self.route_cm_packet(user_mask, packet)?;
            }
        };
        Ok(())
    }
    // SPAWN THREAD (listen_port)
    // TODO: One thread for all ports; should be a different thread for each port
    fn listen_port(&mut self, msg: PortToPePacket) -> Result<(), Error> {
        let _f = "listen_port";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                match &msg {
                    PortToPePacket::Packet((port_no, packet)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "packet": packet.to_string()? });
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
                    PortStatus::Connected => self.set_may_send(port_no),
                    PortStatus::Disconnected => self.set_may_not_send(port_no)
                }
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_status" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, number_of_packets, status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + &self.cell_id.get_name() })?
            },
            
            // recv from neighbor
            PortToPePacket::Packet((port_no, packet)) => {
                self.process_packet_from_port(port_no, packet).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + &self.cell_id.get_name() })?
            }
        };
        Ok(())
    }

    fn reroute_packets(&mut self, broken_port_no: PortNo, new_parent: PortNo, no_packets: NumberOfPackets) {
        let _f = "reroute_packets";
        self.reroute[broken_port_no.as_usize()] = new_parent;
        let no_my_sent_packets = self.get_no_sent_packets(broken_port_no);
        let sent_buf = &mut self.sent_packets[broken_port_no.as_usize()];
        let no_her_seen_packets = no_packets.get_number_seen();
        let no_resend = no_my_sent_packets - no_her_seen_packets;
        let mut remaining_sent = sent_buf.split_off(no_resend);
        let broken_outbuf = &mut self.out_buffers[broken_port_no.as_usize()].clone();
        let new_parent_outbuf = &mut self.out_buffers[new_parent.as_usize()];
        new_parent_outbuf.append(&mut remaining_sent);
        new_parent_outbuf.append(broken_outbuf);
    }
    fn route_cm_packet(&mut self, user_mask: Mask, packet: Packet) -> Result<(), Error> {
        let _f = "route_cm_packet";
        let uuid = packet.get_tree_uuid().for_lookup();  // Strip AIT info for lookup
        let entry = self.routing_table.get_entry(uuid).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
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
                        if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_packet_from_cm" };
                            let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state, "packet": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        if CONFIG.debug_options.pe_pkt_recv {
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
    fn send_packet(&mut self, port_no: PortNo, packet: &Packet) -> Result<(), Error> {
        let _f = "send_packet";
        let mut reroute_port_no = self.reroute[port_no.as_usize()];
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_packet" };
                let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "reroute_port_no": reroute_port_no, "packet": packet.to_string()? });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if reroute_port_no == PortNo(0) {
            reroute_port_no = port_no;
        } else {
            let broken_outbuf = &mut self.out_buffers[port_no.as_usize()];
            if broken_outbuf.len() > 0 {
                // Only clone if there are packets in the broken out buffer
                let broken_outbuf = &mut self.out_buffers[port_no.as_usize()].clone();
                let reroute_outbuf = &mut self.out_buffers[reroute_port_no.as_usize()];
                reroute_outbuf.append(broken_outbuf);
            }
        }
        if port_no == PortNo(0) {
            self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet.clone())))?;
        } else {
            self.pe_to_ports.get(&reroute_port_no)
                .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: *reroute_port_no }.into())?
                .send(packet.clone())?;
        }
        Ok(())
    }
    fn send_packet_flow_control(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_packet";
        if self.may_send(port_no) {
            if let Some(packet) = self._pop_first_outbuf(port_no) {
                self.set_may_not_send(port_no);
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
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
                if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
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

    // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
    // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
    // when I block waiting for a tree update because of a deadlock with listen_cm_loop.
    fn process_packet_from_port(&mut self, port_no: PortNo, packet: Packet)
                                -> Result<(), Error> {
        let _f = "process_packet";
        // Got a packet from the other side, so clear state
        self.set_may_send(port_no);
        self.add_seen_packet_count(port_no);
        self.clear_sent_packets(port_no);
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
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
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
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
                let entry = match self.routing_table.get_entry(uuid) {
                    Ok(e) => e,
                    Err(err) => {
                        // deliver to CellAgent when tree not recognized
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_err" };
                                let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "err": err.to_string(), "packet": packet });
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.pe_to_cm.send(PeToCmPacket::Packet((port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + &self.cell_id.get_name() })?;
                        return Ok(())
                    }
                };
                { // Debug block
                    {
                        if CONFIG.debug_options.all | CONFIG.debug_options.pe_pkt_send {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_entry" };
                            let trace = json!({ "cell_id": &self.cell_id, "entry": entry, "packet": packet });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    let msg_type = MsgType::msg_type(&packet);
                    let port_tree_id = packet.get_port_tree_id();
                    let ait_state = packet.get_ait_state();
                    {
                        if CONFIG.debug_options.all | CONFIG.debug_options.pe_process_pkt {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_process_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "port_tree_id": port_tree_id, "ait_state": ait_state,
                            "msg_type": &msg_type, "port_no": &port_no, "entry": &entry });
                            let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
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
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward" };
                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "user_mask": user_mask, "entry_mask": entry.get_mask(), "packet": packet.to_string()? });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if packet.get_tree_uuid().for_lookup() == self.connected_tree_uuid {
           // Send with CA flow control (currently none)
            let mask = user_mask.and(entry.get_mask());
            let port_nos = mask.get_port_nos();
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_connected_tree" };
                    let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no,
                        "user_mask": user_mask, "entry_mask": entry.get_mask(), "port_nos": port_nos,
                        "packet": packet.to_string()? });
                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            for port_no in port_nos.into_iter() {
                self.send_packet(port_no, packet_ref)?;
            }
        } else {
            if recv_port_no != entry.get_parent() {
                // Send to root if recv port is not parent
                let parent = entry.get_parent();
                if *parent == 0 {
                    // deliver to CModel
                    {
                        if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_rootward" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        if CONFIG.debug_options.all || CONFIG.debug_options.pe_pkt_send {
                            let msg_type = MsgType::msg_type(&packet);
                            match msg_type {
                                MsgType::Discover => (),
                                _ => {
                                    let tree_name = packet.get_port_tree_id();
                                    {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_rootward" };
                                        let trace = json!({ "cell_id": self.cell_id, "tree_name": &tree_name, "msg_type": &msg_type, "parent_port": &parent });
                                        let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                                    }
                                    if msg_type == MsgType::Manifest { println!("PacketEngine {} forwarding manifest rootward", self.cell_id); }
                                    println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, _f, *parent, msg_type, tree_name);
                                },
                            }
                        }
                    }
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
                } else {
                    // Forward rootward
                    {
                        if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_ports_rootward" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.add_to_out_buffer_back(parent, packet);
                    self.send_next_packet_or_entl(entry.get_parent())?;
                }
            } else {
                // Send leafward if recv port is parent
                let mask = user_mask.and(entry.get_mask());
                let port_nos = mask.get_port_nos();
                // Only side effects so use explicit loop instead of map
                for port_no in port_nos.iter().cloned() {
                    if *port_no == 0 {
                        // deliver to CModel
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_leafward_uptree" };
                                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone()))).context(PacketEngineError::Chain { func_name: _f, comment: S("leafcast packet to ca ") + &self.cell_id.get_name() })?;
                    } else {
                        // forward to neighbor
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_leafward" };
                                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.to_string()? });
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                            if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
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
        write!(s, "{}", self.routing_table_mutex.lock().unwrap())?;
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
