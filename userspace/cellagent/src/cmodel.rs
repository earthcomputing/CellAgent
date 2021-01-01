use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          collections::hash_map::Entry::{Occupied, Vacant},
          thread,
          thread::JoinHandle,
};

use failure::{Error, ResultExt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::MsgType;
use crate::ec_message_formats::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCm, PeFromCm, PeToPort, PeFromPort, 
                                PeToCmPacket, CmToPePacket, CmToCaBytes};
use crate::name::{Name, CellID, TreeID};
use crate::packet_engine::{PacketEngine};
use crate::packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer, PacketUniquifier};
use crate::snake::Snake;
use crate::utility::{ByteArray, PortNo, S, TraceHeader, TraceHeaderParams, TraceType};
use crate::uuid_ec::AitState;

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_engine: PacketEngine,
    packet_assemblers: PacketAssemblers,
    snakes: HashMap<PacketUniquifier, Snake>,
    cm_to_ca: CmToCa,
    cm_to_pe: CmToPe,
}
impl Cmodel {
    pub fn get_name(&self) -> String { self.cell_id.get_name() }
    pub fn get_cell_id(&self) -> &CellID { &self.cell_id }
    // NEW
    pub fn new(cell_id: CellID, connected_tree_id: TreeID, pe_to_cm: PeToCm, cm_to_ca: CmToCa,
               pe_from_ports: PeFromPort, pe_to_ports: HashMap<PortNo, PeToPort>,
               border_port_nos: &HashSet<PortNo>, 
               cm_to_pe: CmToPe, pe_from_cm: PeFromCm) -> (Cmodel, JoinHandle<()>) {
        let packet_engine = PacketEngine::new(cell_id, connected_tree_id,
                                              pe_to_cm, pe_to_ports, &border_port_nos);
        let pe_join_handle = packet_engine.start(pe_from_cm, pe_from_ports);
        (Cmodel { cell_id,
                  packet_engine,
                  packet_assemblers: PacketAssemblers::new(),
                  snakes: Default::default(),
                  cm_to_ca, cm_to_pe },
         pe_join_handle)
    }

    // SPAWN THREAD (cm.initialize)
    pub fn start(&self, cm_from_ca: CmFromCa, cm_from_pe: CmFromPe) -> JoinHandle<()> {
        let _f = "start";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_start_cmodel" };
                let trace = json!({ "cell_id": self.get_cell_id() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let mut cm = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {}", self.get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            match cm.initialize(cm_from_ca, cm_from_pe) {
                Ok(_) => (),
                Err(e) => {
                    println!("Cmodel {}: {} error {}", cm.cell_id, _f, e);
                }
            }
            if CONFIG.continue_on_error { } // Don't automatically restart cmodel if it crashes
        }).expect("cmodel thread failed")
    }

    // INIT (CmFromCa, CmFromPe)
    // WORKER (CModel)
    pub fn initialize(&mut self, cm_from_ca: CmFromCa, cm_from_pe: CmFromPe) -> Result<(), Error> {
        let _f = "initialize";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), 
                    "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.listen(cm_from_ca, cm_from_pe)?;
        Ok(())
    }
    fn listen(&mut self, cm_from_ca: CmFromCa, cm_from_pe: CmFromPe) -> Result<(), Error> {
        let _f = "listen";
        loop {
            select! {
                recv(cm_from_ca) -> recvd => {
                    let msg = recvd.context(CmodelError::Chain { func_name: _f, comment: S("cm from ca") })?;
                    self.listen_ca(msg).context(CmodelError::Chain { func_name: _f, comment: S("listen ca") })?;
                },
                recv(cm_from_pe) -> recvd => {
                    let msg = recvd.context(CmodelError::Chain { func_name: _f, comment: S("cm from pe") })?;
                    self.listen_pe(msg).context(CmodelError::Chain { func_name: _f, comment: S("listen pe") })?;
                }
            }
        }
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self, msg: CaToCmBytes) -> Result<(), Error> {
        let _f = "listen_ca";
        match msg {
            // just forward to PE
            CaToCmBytes::Reroute((broken_port, new_parent, number_of_packets)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_reroute" };
                        let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port, "new_parent": new_parent, "no_packets": number_of_packets });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.cm_to_pe.send(CmToPePacket::Reroute((broken_port, new_parent, number_of_packets)))?;
            },
            CaToCmBytes::Delete(uuid) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_delete" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.cm_to_pe.send(CmToPePacket::Delete(uuid))?;
            },
            CaToCmBytes::Entry(entry) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                self.cm_to_pe.send(CmToPePacket::Entry(entry))?;
            },
            CaToCmBytes::Status(status_msg) => {
                self.cm_to_ca.send(CmToCaBytes::Status(status_msg))?;
            }
            CaToCmBytes::TunnelPort(tunnel_msg) => {
                self.cm_to_ca.send(CmToCaBytes::TunnelPort(tunnel_msg))?;
            }
            CaToCmBytes::TunnelUp(tunnel_msg) => {
                self.cm_to_ca.send(CmToCaBytes::TunnelUp(tunnel_msg))?;
            }
        
            // packetize
            CaToCmBytes::Bytes((tree_id, is_ait, is_snake, user_mask, seq_no, bytes)) => {
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.cm_from_ca {
                        let dpi_msg = MsgType::msg_from_bytes(&bytes)?;
                        let dpi_msg_type = dpi_msg.get_msg_type();
                        match dpi_msg_type {
                            MsgType::Discover => (),
                            _ => {
                                println!("Cmodel {}: {} received {}", self.cell_id, _f, dpi_msg);
                            }
                        }
                    }
                }
                // xmit msg
                let mut uuid = tree_id.get_uuid();
                if is_ait { uuid.make_ait_send(); }
                if is_snake { uuid.make_snake(); }
                let packets = Packetizer::packetize(&uuid, seq_no, &bytes).context(CmodelError::Chain { func_name: _f, comment: S("") })?;
                let first = packets.get(0).expect("No packets from packetizer");
                let dpi_is_ait = first.is_ait();
                let sender_msg_seq_no = first.get_unique_msg_id();
                let packet_count = first.get_count();
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.cm_from_ca {
                        println!("Cmodel {}: {} packetize - is_ait {} sender_msg_seq_no {} count {}", self.cell_id, _f, dpi_is_ait, *sender_msg_seq_no, packet_count);
                    }
                }
                for packet in packets {
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_packet" };
                            let trace = json!({ "cell_id": &self.cell_id, "user_mask": user_mask, "is_ait": is_ait, "is_snake": is_snake,
                                "packet": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.cm_to_pe.send(CmToPePacket::Packet((user_mask, packet)))?;
                }
            }
        }
        Ok(())
    }

    // WORKER (CmFromPe)
    fn listen_pe(&mut self, packet: PeToCmPacket) -> Result<(), Error> {
        let _f = "listen_pe";
        match packet {
            // just forward to CA
            PeToCmPacket::Status((port_no, is_border, number_of_packets, status)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_status" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status});
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                if !CONFIG.replay {
                    self.cm_to_ca.send(CmToCaBytes::Status((port_no, is_border, number_of_packets, status)))?;
                }
            },
        
            // de-packetize
            PeToCmPacket::Packet((port_no, packet)) => {
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "packet": packet.to_string()? });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                if packet.get_ait_state() == AitState::SnakeD {
                    let bytes = packet.get_bytes();
                    let serialized = ByteArray::new_from_bytes(&bytes).to_string()?;
                    let uniquifier: PacketUniquifier = serde_json::from_str(&serialized)?;
                    let snakes_len = self.snakes.len();  // Keep borrow checker happy
                    match self.snakes.entry(uniquifier) {
                        Vacant(_) => return Err(CmodelError::SnakeD { func_name: _f, uniquifier }.into()),
                        Occupied(mut snake_entry) => {
                            let snake = snake_entry.get_mut();
                            let new_count = snake.decrement_count();
                            {
                                if CONFIG.trace_options.all || CONFIG.trace_options.snake {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_snaked" };
                                    let trace = json!({ "cell_id": &self.cell_id, "port_no": port_no, "new_count": new_count, "no_snakes": snakes_len, "snake": snake });
                                    let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                        if new_count == 0 {
                                let snake = self.snakes.remove(&uniquifier).expect("Know value is set from match");
                                let ack_port_no = snake.get_ack_port_no();
                                if ack_port_no != PortNo(0) {
                                    let snaked_packet = Packet::make_snake_ack(uniquifier)?;
                                    self.cm_to_pe.send(CmToPePacket::SnakeD((ack_port_no, snaked_packet)))?;
                                }
                            }
                        },
                    }
                } else {
                    self.process_packet(port_no, packet)?;
                }
            },
            PeToCmPacket::Snake((ack_port_no, count, packet)) => {
                let uniquifier = packet.get_uniquifier();
                if count > 0 {
                    let snake = Snake::new(ack_port_no, count, packet);
                    if let Some(_) = self.snakes.insert(uniquifier, snake) {
                       return Err(CmodelError::Snake { func_name: _f, old_value: uniquifier }.into() )
                    }
                } else {
                    let snaked_packet = Packet::make_snake_ack(uniquifier)?;
                    self.cm_to_pe.send(CmToPePacket::SnakeD((ack_port_no, snaked_packet)))?;
                }
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.snake {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_snake" };
                        let trace = json!({ "cell_id": &self.cell_id, "uniquifier": uniquifier, "count": count, "ack_port_no": ack_port_no, "no_snakes": self.snakes.len() });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
            }
        };
        Ok(())
    }

/*
header.uuid
payload.sender_msg_seq_no
payload.size
payload.is_last
payload.bytes
packet_count

packet_assembler::
sender_msg_seq_no: SenderMsgSeqNo,
packets: Vec<Packet>,
*/

    fn process_packet(&mut self, port_no: PortNo, packet: Packet) -> Result<(), Error> {
        let _f = "process_packet";
        let sender_msg_seq_no = packet.get_unique_msg_id();
        let packet_assembler = self.packet_assemblers.entry(sender_msg_seq_no).or_insert(PacketAssembler::new(sender_msg_seq_no)); // autovivification
        let (last_packet, packets) = packet_assembler.add(packet.clone()); // Need clone only because of trace
        let is_ait = packets[0].is_ait();
        let uuid = packet.get_tree_uuid();
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_bytes" };
                let trace = json!({ "cell_id": &self.cell_id, "port": port_no, 
                    "is_ait": is_ait, "tree_uuid": uuid, "last_packet": last_packet });
                let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f); // sender side, dup
            }
        }
        if last_packet {
            let bytes = Packetizer::unpacketize(&packets).context(CmodelError::Chain { func_name: _f, comment: S("") })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_bytes_last" };
                    let trace = json!({ "cell_id": &self.cell_id, "port": port_no, 
                        "is_ait": is_ait, "tree_uuid": uuid, "bytes": bytes.to_string()? });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f); // sender side, dup
                }
            }
            {
                if CONFIG.debug_options.all || CONFIG.debug_options.cm_to_ca {
                    let packet_count = packets[0].get_count();
                    let dpi_msg = MsgType::msg_from_bytes(&bytes)?;
                    let dpi_msg_type = dpi_msg.get_msg_type();
                    match dpi_msg_type {
                        MsgType::Discover => (),
                        _ => {
                            println!("Cmodel {}: {} received {} count {}", self.cell_id, _f, dpi_msg, packet_count);
                        }
                    }
                }
            }
            let msg = CmToCaBytes::Bytes((port_no, is_ait, uuid, bytes));
            if !CONFIG.replay {
                self.cm_to_ca.send(msg)?;
            }
        }
        Ok(())
    }
    pub fn get_packet_engine(&self) -> &PacketEngine { &self.packet_engine }
}

impl fmt::Display for Cmodel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nCmodel {}", self.cell_id.get_name());
        write!(s, "\n{}", self.packet_engine)?;
        write!(f, "{}", s)
    }
}

// Errors
#[derive(Debug, Fail)]
pub enum CmodelError {
    #[fail(display = "CmodelError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "CmodelError::Snake {} duplicate packet in snakes {}", func_name, old_value)]
    Snake { func_name: &'static str, old_value: PacketUniquifier },
    #[fail(display = "CmodelError::SnakeD {} Missing snake {}", func_name, uniquifier)]
    SnakeD { func_name: &'static str, uniquifier: PacketUniquifier }
}
