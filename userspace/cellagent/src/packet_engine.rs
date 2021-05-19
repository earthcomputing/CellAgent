/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{collections::{HashMap, HashSet, VecDeque}, fmt, fmt::Write, str, sync::{Arc, Mutex}, thread, thread::JoinHandle};

use crate::{config::{CONFIG}};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{MsgType};
use crate::ec_message_formats::{PeFromCm, PeToCm, PeToCmPacket,
                                PeToPort, PeFromPort, PortToPePacket, PeToPortPacket,
                                CmToPePacket};
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
type Buffer = VecDeque<(bool, PortNo, Packet)>;
type PacketArray = Vec<Buffer>;
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
    sent_packets: PacketArray,  // Packets that may need to be resent
    out_buffers: PacketArray,   // Packets waiting to go on the out port
    in_buffer: Vec<Buffer>,
    reroute: Reroute,
    pe_to_cm: PeToCm,
    pe_to_ports: HashMap<PortNo, PeToPort>,
}

impl PacketEngine {
    // NEW
    pub fn new(cell_id: CellID, connected_tree_id: TreeID, 
               pe_to_cm: PeToCm, 
               pe_to_ports: HashMap<PortNo, PeToPort>,
               border_port_nos: &HashSet<PortNo>) -> PacketEngine {
        let routing_table = RoutingTable::new(cell_id);
        let routing_table_mutex = Arc::new(Mutex::new(routing_table.clone()));
        let count = [0; MAX_SLOTS as usize];
        PacketEngine {
            cell_id,
            connected_tree_uuid: connected_tree_id.get_uuid(),
            routing_table,
            routing_table_mutex,  // Needed so I can print the routing table from main
            border_port_nos: border_port_nos.clone(),
            no_seen_packets: count,
            no_sent_packets: count,
            sent_packets: vec![Default::default(); MAX_SLOTS], // Slots need to be allocated ahead of time
            out_buffers: vec![Default::default(); MAX_SLOTS],
            in_buffer: vec![Default::default(); MAX_SLOTS],
            reroute: [Default::default(); MAX_SLOTS],
            pe_to_cm,
            pe_to_ports,
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
    fn _get_outbuf(&self, port_no: PortNo) -> &Buffer {
        self.out_buffers.get(port_no.as_usize()).expect("PacketEngine: get_outbuf must succeed")
    }
    fn get_outbuf_mut(&mut self, port_no: PortNo) -> &mut Buffer {
        self.out_buffers.get_mut(port_no.as_usize()).expect("PacketEngine: get_outbuf must succeed")
    }
    fn _get_size(array: &PacketArray, port_no: PortNo) -> usize {
        array[port_no.as_usize()].len()
    }
    fn _get_outbuf_size(&self, port_no: PortNo) -> usize {
        PacketEngine::_get_size(&self.out_buffers, port_no)
    }
    fn _get_outbuf_first_type(&self, port_no: PortNo) -> Option<MsgType> {
        self._get_outbuf(port_no)
            .get(0)
            .map(|(_, _, packet)| MsgType::msg_type(packet))
    }
    fn _get_outbuf_first_ait_state(&self, port_no: PortNo) -> Option<AitState> {
        self._get_outbuf(port_no)
            .get(0)
            .map(|(_, _, packet)| packet.get_ait_state())
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
    fn _get_no_seen_packets(&self, port_no: PortNo) -> usize {
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
    fn add_to_out_buffer_back(&mut self, recv_port_no: PortNo, port_no: PortNo, packet: Packet) -> bool {
        let _f = "add_to_out_buff_back";
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
                }
                {
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
    fn listen_port(&mut self, msg: PortToPePacket) -> Result<(), Error> {
        let _f = "listen_port";
        match msg {
            #[cfg(feature = "api-new")]
            PortToPePacket::Activity((port_no, data)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_activity" };
                        let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "activity data": data });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                let p = port_no.as_usize();
                // Check activity data
                if self.activity_data[p] == data {
                    // Do something interesting
                }
            },
            #[cfg(feature = "api-new")]
            PortToPePacket::Increment((port_no, outbuf)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_increment" };
                        let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "outbuf": outbuf });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
            },
            PortToPePacket::Packet((port_no, packet)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.process_packet_from_port(port_no, packet)?;
            },
            PortToPePacket::Status(status_msg) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_from_port_status" };
                        let trace = json!({ "cell_id": &self.cell_id,  "port": status_msg.0, "status_msg": status_msg});
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }

                self.pe_to_cm.send(PeToCmPacket::Status(status_msg)).context(PacketEngineError::Chain { func_name: "listen_port", comment: S("send status to ca ") + &self.cell_id.get_name() })?
            }
        }
        Ok(())
    }
    fn process_packet_from_port(&mut self, recv_port_no: PortNo, packet: Packet) -> Result<(), Error> {
        let _f = "process_packet_from_port";
        self.add_seen_packet_count(recv_port_no);
        self.clear_sent_packets(recv_port_no);
        match packet.get_ait_state() {
            AitState::Teck | // Forward packet if end-to-end AIT, but now we have hop by hop
            AitState::Tack | // "
            AitState::Tuck | // "
            AitState::Tyck | // "
            AitState::Tick | // "
            AitState::Tock | // "
            AitState::Init => {
                return Err(PacketEngineError::Ait { func_name: _f, ait_state: packet.get_ait_state() }.into())
            },
            AitState::SnakeD => {
                {
                    if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_snaked" };
                        let trace = json!({ "cell_id": &self.cell_id, "recv_port": recv_port_no, "packet": packet.stringify()? });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet)))?;
            },
            AitState::AitD => (), // TODO: Send to cm once cell agent knows how to handle it
            AitState::Ait | // Implements hop-by-hop AIT
            AitState::Normal => { // Forward packet
                let uuid = packet.get_tree_uuid().for_lookup();
                let entry = match self.routing_table.get_entry(uuid) {
                    Ok(e) => e,
                    Err(err) => {
                        // deliver to CellAgent when tree not recognized
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_packet_err" };
                                let trace = json!({ "cell_id": &self.cell_id, "recv_port": recv_port_no, "err": err.to_string(), "packet": packet });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet))).context(PacketEngineError::Chain { func_name: "forward", comment: S("rootcast packet to ca ") + &self.cell_id.get_name() })?;
                        return Ok(())
                    }
                };
                { 
                    {
                        let ait_state = packet.get_ait_state();
                        if CONFIG.debug_options.all | CONFIG.debug_options.pe_process_pkt {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_process_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "ait_state": ait_state,
                                "port_no": &recv_port_no, "entry": &entry, "packet": packet.stringify()? });
                            add_to_trace(TraceType::Debug, trace_params, &trace, _f);
                        }
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
            AitState::Init |
            AitState::Tick |
            AitState::Tock |
            AitState::Tyck |
            AitState::Tuck |
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
                }
                let port_no = PortNo(0);
                self.forward(port_no, entry, user_mask, &packet).context(PacketEngineError::Chain { func_name: _f, comment: S(self.cell_id.get_name()) })?;
            }
        }
        Ok(())
    }
    fn send_packet_to_port_or_cm(&mut self, recv_port_no: PortNo, packet: &Packet) -> Result<(), Error> {
        let _f = "send_packet";
        let mut reroute_port_no = self.reroute[recv_port_no.as_usize()];
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_or_cm_packet" };
                let trace = json!({ "cell_id": self.cell_id, "recv_port_no": recv_port_no, "reroute_port_no": reroute_port_no, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if reroute_port_no == PortNo(0) {
            reroute_port_no = recv_port_no;
        } else {
            let broken_outbuf = &mut self.get_outbuf_mut(recv_port_no).clone();
            if broken_outbuf.len() > 0 {
                let reroute_outbuf = self.get_outbuf_mut(reroute_port_no);
                reroute_outbuf.append(broken_outbuf);
                self.get_outbuf_mut(recv_port_no).clear();
            }
        }
        if recv_port_no == PortNo(0) {
            self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone())))?;
        } else {
            self.add_sent_packet(reroute_port_no, packet.clone());
            self.pe_to_ports.get(&reroute_port_no)
                .ok_or::<Error>(PacketEngineError::Sender { cell_id: self.cell_id, func_name: _f, port_no: reroute_port_no }.into())?
                .send(PeToPortPacket::Packet(packet.clone()))?;
        }
        Ok(())
    }
    fn send_packet_flow_control(&mut self, port_no: PortNo) -> Result<(), Error> {
        let _f = "send_packet_flow_control";
        let first = self.pop_first_outbuf(port_no);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.pe_port {
                let first_str = match first {
                    Some((pong_sent, recv_port_no, packet)) => format!("pong_sent: {}, recv_port_no: {}, packet: {}", pong_sent, recv_port_no, packet.stringify()?),
                    None => "None".to_owned()
                };
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_first_in_buffer" };
                let trace = json!({ "cell_id": self.cell_id, "port_no": port_no, "first": first_str });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if let Some((_, _, packet)) = first {
            let outbuf = self.get_outbuf_mut(port_no);
            let last_item = outbuf.get(MAX_SLOTS);
            if let Some((pong_sent, recv_port_no, last_packet)) = last_item {
                if !pong_sent {
                    let recv_port_no = *recv_port_no; // Needed to avoid https://github.com/rust-lang/rust/issues/59159>
                    outbuf[MAX_SLOTS] = (true, recv_port_no, last_packet.clone());
                    self.send_packet_flow_control(recv_port_no)?;
                }
            }
            self.send_packet_to_port_or_cm(port_no, &packet)?;
        }
        Ok(())
    }
    fn forward(&mut self, recv_port_no: PortNo, entry: RoutingTableEntry, user_mask: Mask, packet_ref: &Packet)
            -> Result<(), Error> {
        let _f = "forward_old";
        let packet = packet_ref.clone();
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_forward" };
                let trace = json!({ "cell_id": &self.cell_id, "recv_port_no": recv_port_no, "user_mask": user_mask, 
                    "entry_mask": entry.get_mask(), "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let count = if packet.get_tree_uuid().is_control() {
            // No snake for hop-by-hop messages
            // Send with CA flow control (currently none)
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
                // Should add_to_buffer_front, but that may confuse my "when to send pong" algorithm
                if port_no == PortNo(0) { 
                    self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone())))?;
                } else {
                    let pe_to_port = self.pe_to_ports.get(&port_no).expect("PacketEngine forward pe_to_port must be defined");
                    pe_to_port.send(PeToPortPacket::Packet(packet.clone()))?;  // Control message so just send
                }
            }
            0
        } else {
            if recv_port_no != entry.get_parent() {
                // Send to root if recv port is not parent
                let parent = entry.get_parent();
                if *parent == 0 {
                    // deliver to CModel
                    {
                        if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_cm_as_parent" };
                            let trace = json!({ "cell_id": &self.cell_id, "recv_port": recv_port_no, "entry": entry, "packet": packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
                        self.pe_to_cm.send(PeToCmPacket::Packet((recv_port_no, packet.clone()))).context(PacketEngineError::Chain { func_name: _f, comment: S("leafcast packet to ca ") + &self.cell_id.get_name() })?;
                    } else {
                        count = count + 1;  // Only count ports other than 0
                        // forward to neighbor
                        {
                            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_to_port_leafward" };
                                let trace = json!({ "cell_id": &self.cell_id, "mask": mask, "recv_port_no": recv_port_no, "port_no": port_no, "packet": packet.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        }
                        self.send_pong_if_room(recv_port_no, port_no, &packet)?;
                    }
                }
                count
            }
        };
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
        let has_room = self.add_to_out_buffer_back(recv_port_no, port_no, packet.clone());
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.pe {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "pe_send_pong" };
                let trace = json!({ "cell_id": &self.cell_id, "recv_port_no": recv_port_no, "port_no": port_no, "has_room": has_room, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        if has_room {  // Send pong if packet went into first half of outbuf
            self.send_packet_flow_control(port_no)?;
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
    #[fail(display = "PacketEngineError::InBuffer {} port {} on cell {} count is zero", func_name, port_no, cell_id)]
    InBuffer { func_name: &'static str, cell_id: CellID, port_no: PortNo},
    #[fail(display = "PacketEngineError::Sender {}: No sender for port {} on cell {}", func_name, port_no, cell_id)]
    Sender { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "PacketEngineError::Uuid {}: CellID {}: type {} entry uuid {}, packet uuid {}", func_name, cell_id, msg_type, table_uuid, packet_uuid)]
    Uuid { func_name: &'static str, cell_id: CellID, msg_type: MsgType, table_uuid: Uuid, packet_uuid: Uuid }
}
