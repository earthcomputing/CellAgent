use std::{
    collections::{HashMap, HashSet, VecDeque}, 
    fmt, fmt::Write, str, 
    sync::{Arc, Mutex}, 
    thread, thread::JoinHandle};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{MsgType};
use crate::ec_message_formats::{PeFromCm, PeToCm,
                                PeToPort, PeFromPort, PortToPePacket, PeToPortSync,
                                CmToPePacket, PeToCmPacket};
use crate::name::{Name, CellID, TreeID};
use crate::packet::{Packet};
use crate::routing_table::RoutingTable;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{Mask, PortNo, S, TraceHeaderParams, TraceType, write_err };
use crate::uuid_ec::{AitState, Uuid};

// I need one slot per port, but ports use 1-based indexing.  I could subtract 1 all the time,
// but it's safer to waste slot 0.
// TODO: Use Config.max_num_phys_ports_per_cell
const MAX_SLOTS: usize = 10; //MAX_NUM_PHYS_PORTS_PER_CELL.0 as usize + 1;

type UsizeArray = [usize; MAX_SLOTS];
type InBuffer = (usize, Packet);
type OutBuffer = VecDeque<(bool, PortNo, Packet)>;
type CtlBuffer = Vec<Packet>;
type PacketArray = Vec<OutBuffer>;
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
    sent_packets: Vec<OutBuffer>, // Packets that may need to be resent
    ctl_buffer: CtlBuffer, // Separate buffer for control messages so they can bypass others
    out_buffers: Vec<OutBuffer>,   // Packets waiting to go on the out port
    in_buffers: [InBuffer; MAX_SLOTS],    // Packets on the in port waiting to into out_buf on the out port
    reroute: Reroute,
    pe_to_cm: PeToCm,
    pe_to_ports: HashMap<PortNo, PeToPort>,
    pe_to_ports_sync: HashMap<PortNo, PeToPortSync>
}

impl PacketEngine {
    // NEW
    pub fn new(cell_id: CellID, connected_tree_id: TreeID, pe_to_cm: PeToCm,
               pe_to_ports: HashMap<PortNo, PeToPort>, pe_to_ports_sync: HashMap<PortNo, PeToPortSync>,
               border_port_nos: &HashSet<PortNo>) -> PacketEngine {
        let routing_table = RoutingTable::new(cell_id);
        let routing_table_mutex = Arc::new(Mutex::new(routing_table.clone()));
        let count = [0; MAX_SLOTS];
        PacketEngine {
            cell_id,
            connected_tree_uuid: connected_tree_id.get_uuid(),
            routing_table,
            routing_table_mutex,  // Needed so I can print the routing table from main
            border_port_nos: border_port_nos.clone(),
            no_seen_packets: count,
            no_sent_packets: count,
            sent_packets: vec![Default::default(); MAX_SLOTS], // Slots need to be allocated ahead of time
            ctl_buffer: vec![Default::default(); MAX_SLOTS],
            out_buffers: vec![Default::default(); 2*MAX_SLOTS],
            in_buffers: [(0, Default::default()); MAX_SLOTS],
            reroute: [PortNo(0); MAX_SLOTS],
            pe_to_cm,
            pe_to_ports,
            pe_to_ports_sync
        }
    }
    
    // SPAWN THREAD (pe.initialize)
    pub fn start(&self, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) -> JoinHandle<()> {
        let _f = "start_packet_engine";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "start_pe" };
                let trace = json!({ "cell_id": self.get_cell_id() });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let mut pe = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {}", self.get_cell_id());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.initialize(pe_from_cm.clone(), pe_from_ports.clone()).map_err(|e| write_err("Called by nalcell", &e));
            if CONFIG.continue_on_error { pe.start(pe_from_cm, pe_from_ports); } 
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
    fn set_inbuf(&mut self, port_no: PortNo, count: usize, packet: &Packet) -> Result<(), Error> {
        let _f = "set_inbuf";
        let mut inbuf = self.in_buffers[port_no.as_usize()];
        if count > 0 && packet.clone() != Default::default() {
            return Err(PacketEngineError::Inbuf { func_name: _f, cell_id: self.cell_id, port_no }.into());
        }
        inbuf.0 = count;
        inbuf.1 = packet.clone();        
        Ok(())
    } 
    fn get_outbuf_mut(&mut self, port_no: PortNo) -> &mut OutBuffer {
        self.out_buffers.get_mut(port_no.as_usize()).expect("PacketEngine: get_outbuf must succeed")
    }
    fn get_size(array: &PacketArray, port_no: PortNo) -> usize {
        array[port_no.as_usize()].len()
    }
    fn get_outbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::get_size(&self.out_buffers, port_no)
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
    fn clear_seen_packet_count(&mut self, port_no: PortNo) {
        self.no_seen_packets[port_no.as_usize()] = 0;
    }
    fn add_sent_packet(&mut self, port_no: PortNo, packet: Packet) {
        let sent_packets = self.sent_packets.get_mut(port_no.as_usize()).expect("PacketEngine: sent_packets must be set");
        sent_packets.push_back((true, port_no, packet));
        PacketEngine::add_to_packet_count(&mut self.no_sent_packets, port_no);
    }
    fn clear_sent_packets(&mut self, port_no: PortNo) {
        self.sent_packets.get_mut(*port_no as usize).expect("PacketEngine: sent_packets entry must be set").clear();
        self.clear_seen_packet_count(port_no);
    }
    fn pop_first_outbuf(&mut self, port_no: PortNo) -> Option<(bool, PortNo, Packet)> {
        self.get_outbuf_mut(port_no).pop_front()
    }
    fn add_to_outbuf_back(&mut self, recv_port_no: PortNo, port_no: PortNo, packet: Packet) -> bool {
        let _f = "add_to_outbuf_back";
        // If the packet was put into the first half of the buffer, then the pong was sent.  We use true for
        // pong_sent to denote this case.  If the packet was put into the second half of the buffer, the pong
        // was not sent.  In that case, we remember the recv_port_no and use it to send the pong when the packet
        // reaches the first half of the buffer.  We then set pong_sent to true.
        let outbuf = self.get_outbuf_mut(port_no);
        let pong_sent = outbuf.len() < MAX_SLOTS;
        outbuf.push_back((pong_sent, recv_port_no, packet));
        pong_sent
    }
    // SPAWN THREAD (listen_cm_loop)
    fn listen_cm(&mut self, msg: CmToPePacket) -> Result<(), Error> {
        let _f = "listen_cm";
        match msg {
            // control plane from CellAgent
            CmToPePacket::Reroute((broken_port_no, new_parent, no_packets)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_reroute" };
                        let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port_no, "new_parent": new_parent, "no_packets": no_packets });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.reroute_packets(broken_port_no, new_parent, no_packets);
            },
            CmToPePacket::Delete(uuid) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if CONFIG.debug_options.all || CONFIG.debug_options.pe_process_pkt {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_delete_entry_dbg" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
                        add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                    }
                }
                self.routing_table.delete_entry(uuid);
                (*self.routing_table_mutex.lock().unwrap()) = self.routing_table.clone();
            }
            CmToPePacket::Entry(entry) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.routing_table.set_entry(entry);
                (*self.routing_table_mutex.lock().unwrap()) = self.routing_table.clone();
            },       
            // route packet, xmit to neighbor(s) or up to CModel
            CmToPePacket::Packet((user_mask, packet)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "user_mask": user_mask, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.process_packet_from_cm(user_mask, packet)?;
            },
            CmToPePacket::SnakeD((ack_port_no, packet)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_cm_snaked" };
                        let trace = json!({ "cell_id": &self.cell_id, "ack_port_no": ack_port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.send_pong_if_room(PortNo(0), ack_port_no, &packet)?;
             }
        };
        Ok(())
    }
    // SPAWN THREAD (listen_port)
    // TODO: One thread for all ports; should be a different thread for each port
    fn listen_port(&mut self, msg: PortToPePacket) -> Result<(), Error> {
        let _f = "listen_port";
        match msg {
            // deliver to CModel
            PortToPePacket::Status((port_no, is_border, port_status)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_status" };
                        let trace = json!({ "cell_id": &self.cell_id,  "port": port_no, "is_border": is_border, "status": port_status});
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                let number_of_packets = NumberOfPackets {
                    sent: self.get_no_sent_packets(port_no),
                    recd: self.get_no_seen_packets(port_no)
                };
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_status" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": port_status });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                if !CONFIG.replay {
                    self.pe_to_cm.send(PeToCmPacket::Status((port_no, is_border, number_of_packets, port_status))).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + &self.cell_id.get_name() })?
                }
            },
            
            // recv from neighbor
            PortToPePacket::Packet((port_no, packet)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.process_packet_from_port(port_no, packet).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("process_packet ") + &self.cell_id.get_name() })?
            },
            PortToPePacket::Ready(port_no) => {
                self.pe_port_sync(port_no)?;
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
        let broken_outbuf = &mut self.get_outbuf_mut(broken_port_no).clone();
        let new_parent_outbuf = &mut self.get_outbuf_mut(new_parent);
        new_parent_outbuf.append(&mut remaining_sent);
        new_parent_outbuf.append(broken_outbuf); 
        self.get_outbuf_mut(broken_port_no).clear(); // Because I had to clone the buffer
    }
    fn process_packet_from_cm(&mut self, user_mask: Mask, packet: Packet) -> Result<(), Error> {
        let _f = "process_packet_from_cm";
        let uuid = packet.get_tree_uuid().for_lookup();  // Strip AIT info for lookup
        let entry = self.routing_table.get_entry(uuid).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
        match packet.get_ait_state() {
            AitState::AitD |
            AitState::Entl |
            AitState::Tick |
            AitState::Tock |
            AitState::Tack |
            AitState::Teck => return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into()), // Not allowed here
            AitState::SnakeD => {}, // Handled in listen_cm()
            AitState::Normal |
            AitState::Ait => {
                { 
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_cm {
                        let uuid = packet.get_uuid();
                        let ait_state = packet.get_ait_state();
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_packet_from_cm" };
                        let trace = json!({ "cell_id": self.cell_id, "uuid": uuid, "ait_state": ait_state, 
                            "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    if CONFIG.debug_options.pe_pkt_recv {
                        let msg_type = MsgType::msg_type(&packet);
                        match msg_type {
                            MsgType::Manifest => println!("PacketEngine {}: {} got from cm {} {}", self.cell_id, _f, msg_type, user_mask),
                            _ => (),
                        }
                    }
                }
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, &packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
        }
        Ok(())
    }
    fn send_packet_to_outbuf(&mut self, recv_port_no: PortNo, port_no: PortNo, packet: Packet) -> Result<(), Error> {
        let _f = "send_packet_to_outbuf";
        let mut reroute_port_no = self.reroute[port_no.as_usize()];
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_or_cm_packet" };
                let trace = json!({ "cell_id": self.cell_id, "recv_port_no": recv_port_no, "port_no": port_no, "reroute_port_no": reroute_port_no, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if reroute_port_no == PortNo(0) {
            reroute_port_no = port_no;
        } else {
            let broken_outbuf = &mut self.get_outbuf_mut(port_no).clone();
            if broken_outbuf.len() > 0 {
                let reroute_outbuf = self.get_outbuf_mut(reroute_port_no);
                reroute_outbuf.append(broken_outbuf);
                self.get_outbuf_mut(port_no).clear();
            }
        }
        if reroute_port_no == PortNo(0) {
            self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
            self.pe_port_sync(recv_port_no)?;
        } else {
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_or_sync" };
                    let trace = json!({ "cell_id": self.cell_id, "recv_port_no": recv_port_no, "port_no": port_no, "reroute_port_no": reroute_port_no, "packet": packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let has_room = self.add_to_outbuf_back(recv_port_no, port_no, packet);
            self.send_packet_flow_control(port_no)?;
            if has_room {
                self.pe_port_sync(recv_port_no)?;
            }
        }
        Ok(())
    }
    fn send_packet_flow_control(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_packet_flow_control";
        let cell_id = self.cell_id;
        let first_item = self.pop_first_outbuf(port_no);
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_first_item" };
                let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "first_item": first_item });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if let Some((pong_sent, recv_port_no, packet)) = first_item {
            if !pong_sent {
                return Err(PacketEngineError::Pong { func_name: _f, cell_id, recv_port_no }.into());
            }
            self.add_sent_packet(recv_port_no, packet.clone());
            if *recv_port_no == 0 {
                self.pe_to_ports.get(&port_no)
                    .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: *recv_port_no }.into())?
                    .send(packet)?;
            }            
            let outbuf = self.get_outbuf_mut(recv_port_no);
            let last_item = outbuf.get(MAX_SLOTS as usize);
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_last_item" };
                    let trace = json!({ "cell_id": cell_id, "port": recv_port_no, "last_item": last_item });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            if let Some((pong_sent, last_recv_port_no, last_packet)) = last_item {
                if !pong_sent {
                    let last_recv_port_no = *last_recv_port_no; // Needed to avoid https://github.com/rust-lang/rust/issues/59159
                    outbuf[MAX_SLOTS as usize] = (true, last_recv_port_no, last_packet.clone());
                    self.pe_port_sync(last_recv_port_no)?;
                } else {
                    return Err(PacketEngineError::Pong { func_name: _f, cell_id, recv_port_no: *last_recv_port_no }.into());
                }
            }
        }
        self.pe_port_sync(port_no)?;
        Ok(())
    }
    fn pe_port_sync(&mut self, recv_port_no: PortNo) -> Result<(), Error> {
        let _f = "pe_port_sync";
        if *recv_port_no == 0 { return Ok(()); } // Until I implement flow control for cellagent
        let (count, packet) = self.in_buffers[recv_port_no.as_usize()]; 
        let count = count - 1;
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "in_count" };
                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "count": count, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if count == 0 { // Count is zero, so send pong
            self.set_inbuf(recv_port_no, 0, &Default::default())?;
            let outbuf = self.out_buffers.get_mut(recv_port_no.as_usize()).expect("PacketEngine: pe_port_sync outbuf must be defined");
            let packet_opt = match outbuf.pop_front() {
                Some((pong_sent, port_no, packet)) => {
                    if !pong_sent {
                        return Err(PacketEngineError::Pong { func_name: _f, cell_id: self.cell_id, recv_port_no }.into());
                    }
                    self.pe_port_sync(port_no)?;
                    Some(packet)
                },
                None => { None }
            };
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                    let packet_str = match packet_opt {
                        Some(packet) => packet.stringify()?,
                        None => "None".to_owned()
                    };
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "send_sync" };
                    let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet_str });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.pe_to_ports_sync.get(&recv_port_no)
                .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: *recv_port_no }.into())?
                .send(packet_opt.clone())?;
            packet_opt.map(|packet| self.add_sent_packet(recv_port_no, packet));
        }
        Ok(())
    }
    // TODO: Make sure I don't have a race condition because I'm dropping the lock on the routing table
    // Potential hazard here; CA may have sent a routing table update.  I can't just hold the lock on the table
    // when I block waiting for a tree update because of a deadlock with listen_cm_loop.
    fn process_packet_from_port(&mut self, recv_port_no: PortNo, packet: Packet) -> Result<(), Error> {
        let _f = "process_packet_from_port";
        // Got a packet from the other side, so clear state
        self.add_seen_packet_count(recv_port_no);
        self.clear_sent_packets(recv_port_no);
        self.set_inbuf(recv_port_no, 1, &packet)?; // Err if inbuf is not Default
        {
            if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
                let msg_type = MsgType::msg_type(&packet);
                match packet.get_ait_state() {
                    AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} msg type {} {}", self.cell_id, *recv_port_no, _f, self.get_outbuf_size(recv_port_no), msg_type, packet.get_ait_state()),
                    _ => ()
                }
            }
        }
        match packet.get_ait_state() {
            AitState::Entl |
            AitState::Teck |
            AitState::Tack |
            AitState::Tock |
            AitState::Tick => {
                return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into())
            },
            AitState::SnakeD => { // Goes to cm
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_snaked" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.send_packet_to_outbuf(recv_port_no, PortNo(0), packet)?;
            },
            AitState::Ait  => { // Goes to cm until we have multi-hop AIT
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet" };
                        let trace = json!({ "cell_id": &self.cell_id, "ait_state": AitState::Ait, "port": recv_port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.send_packet_to_outbuf(recv_port_no, PortNo(0), packet)?;
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
                                let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "err": err.to_string(), "packet": packet });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.send_packet_to_outbuf(recv_port_no, PortNo(0), packet)?;
                        return Ok(());
                    }
                };
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_packet" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "entry": entry, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                if entry.is_in_use() {
                    // Put packets on the right port's queue
                    let mask = entry.get_mask();
                    self.forward(recv_port_no, entry, mask, &packet).context(PacketEngineError::Chain { func_name: "process_packet", comment: S("forward ") + &self.cell_id.get_name() })?;
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
                let trace = json!({ "cell_id": &self.cell_id, "recv_port_no": recv_port_no, "user_mask": user_mask, 
                    "entry": entry, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        // count how many ports the packet is sent on; used for knowing when to pong and for snake handling
        let count = if packet.get_tree_uuid().for_lookup() == self.connected_tree_uuid {
            // No snake for hop-by-hop messages
            // Send with CA flow control (currently none); control msgs bypass flow control on ports
            let mask = user_mask.and(entry.get_mask());
            let port_nos = mask.get_port_nos();
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward_connected_tree" };
                    let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no,
                        "user_mask": user_mask, "entry_mask": entry.get_mask(), "port_nos": port_nos,
                        "packet": packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            for &port_no in port_nos.iter() {
                if port_no == PortNo(0) { 
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone())))?;
                } else {
                    let pe_to_port = self.pe_to_ports.get(&port_no).expect("PacketEngine forward pe_to_port must be defined");
                    pe_to_port.send(packet.clone())?;  // Control message so just send
                }
            }
            0
        } else {
            let parent = entry.get_parent();
            if recv_port_no != parent {
                // Send to root if recv port is not parent
                if *parent == 0 {
                    // deliver to CModel
                    {
                        if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_as_parent" };
                            let trace = json!({ "cell_id": &self.cell_id, "recv_port": recv_port_no, "entry": entry, "packet": packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        if CONFIG.debug_options.all || CONFIG.debug_options.pe_pkt_send {
                            let msg_type = MsgType::msg_type(&packet);
                            match msg_type {
                                MsgType::Discover => (),
                                _ => {
                                    let uuid = packet.get_uuid();
                                    {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_rootward_dbg" };
                                        let trace = json!({ "cell_id": self.cell_id, "uuid": &uuid, "msg_type": &msg_type, "parent_port": &parent });
                                        add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                                    }
                                    if msg_type == MsgType::Manifest { println!("PacketEngine {} forwarding manifest rootward", self.cell_id); }
                                    println!("PacketEngine {}: {} [{}] {} {}", self.cell_id, _f, *parent, msg_type, uuid);
                                },
                            }
                        }
                    }
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone())))?;
                    0
                } else {
                    // Forward rootward
                    {
                        if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_ports_rootward" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": recv_port_no, "packet": packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.send_pong_if_room(recv_port_no, parent, &packet)?;
                    1
                }
            } else {
                // Send leafward if recv port is parent
                let mask = user_mask.and(entry.get_mask());
                let port_nos = mask.get_port_nos();
                let mut count = 0;
                // Only side effects so use explicit loop instead of map
                for port_no in port_nos.iter().cloned() {
                    if *port_no == 0 {
                        // deliver to CModel
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_leafward_uptree" };
                                let trace = json!({ "cell_id": &self.cell_id, "mask": mask, "recv_port_no": recv_port_no, "port_no": port_no, "packet": packet_ref.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone())))?;
                    } else {
                        count = count + 1;  // Only count ports other than 0
                        // forward to neighbor
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_leafward" };
                                let trace = json!({ "cell_id": &self.cell_id, "mask": mask, "recv_port_no": recv_port_no, "port_no": port_no, "packet": packet.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                            if CONFIG.debug_options.all || CONFIG.debug_options.flow_control {
                                let msg_type = MsgType::msg_type(&packet);
                                match packet.get_ait_state() {
                                    AitState::Normal => println!("PacketEngine {}: port {} {} outbuf size {} msg type {} {}", self.cell_id, *port_no, _f, self.get_outbuf_size(port_no), msg_type, packet.get_ait_state()),
                                    _ => ()
                                }
                            }
                        }
                        self.send_pong_if_room(recv_port_no, port_no, &packet)?; // Should only send once after delivered to all out ports
                    }
                }
                count
            }
        };
        if *recv_port_no != 0 {
            self.set_inbuf(recv_port_no, count, &packet)?; // Err if inbuf isn't Default
        }
        if packet.is_snake() {
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_snake" };
                    let trace = json!({ "cell_id": &self.cell_id, "recv_port_no": recv_port_no, "count": count, "is_snake": true, "packet": packet_ref.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.pe_to_cm.send(PeToCmPacket::Snake((recv_port_no, count, packet)))?; 
        }
        Ok(())
    }
    // Send reply if there is room for my packet in the buffer of the out port
    fn send_pong_if_room(&mut self, recv_port_no: PortNo, port_no: PortNo, packet: &Packet) ->
            Result<(), Error> {
        let _f = "send_pong_if_room";
        let has_room = self.add_to_outbuf_back(recv_port_no, port_no, packet.clone());
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_outbuf_if_room" };
                let trace = json!({ "cell_id": &self.cell_id, "has_room": has_room, "recv_port_no": recv_port_no, "port_no": port_no, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if has_room {  // Send pong if packet went into first half of outbuf
            self.send_packet_flow_control(recv_port_no)?;
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
    #[fail(display = "PacketEngineError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "PacketEngineError::Inbuf {} {} attempt to fill full buffer on port {}", func_name, cell_id, port_no)]
    Inbuf { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "PacketEngineError::Pong {} Pong not sent for first packet {} on port on cell {}", func_name, recv_port_no, cell_id)]
    Pong { func_name: &'static str, cell_id: CellID, recv_port_no: PortNo },
    #[fail(display = "PacketEngineError::Sender {}: No sender for port {} on cell {}", func_name, port_no, cell_id)]
    Sender { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, table_uuid: Uuid, packet_uuid: Uuid }
}
