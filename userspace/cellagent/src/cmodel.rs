use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          thread,
          thread::JoinHandle,
};

use failure::{Error, ResultExt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{Message, MsgType};
use crate::ec_message_formats::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCm, PeFromCm, PeToPort, PeFromPort, 
                                PeToCmPacket, CmToPePacket, CmToCaBytes};
use crate::name::{Name, CellID, TreeID};
use crate::packet_engine::{PacketEngine};
use crate::packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use crate::utility::{PortNo, S, TraceHeader, TraceHeaderParams, TraceType};

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_engine: PacketEngine,
    packet_assemblers: PacketAssemblers,
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
                  cm_to_ca, cm_to_pe },
         pe_join_handle)
    }

    // SPAWN THREAD (cm.initialize)
    pub fn start(&self, cm_from_ca: CmFromCa, cm_from_pe: CmFromPe) -> JoinHandle<()> {
        let _f = "start_cmodel";
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
            let _ = cm.initialize(cm_from_ca, cm_from_pe);
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
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
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
         {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                match &msg {
                    CaToCmBytes::Bytes((_, _, _, _, seq_no, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes" };
                        let trace = json!({"cell_id": &self.cell_id, "msg_len": bytes.len(), "msg_no": seq_no });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Delete(uuid) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_delete" };
                        let trace = json!({ "cell_id": &self.cell_id, "uuid": uuid });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Entry(entry) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Reroute((broken_port, new_parent, number_of_packets)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_reroute" };
                        let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port, "new_parent": new_parent, "no_packets": number_of_packets });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Status((port_no, is_border, _number_of_packets, status)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_status" };
                        let trace = json!({"cell_id": &self.cell_id, "port_no": port_no, "is_border": is_border, "status": status });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::TunnelPort((port_no, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_port_tunnel" };
                        let trace = json!({"cell_id": &self.cell_id, "port_no": port_no, "app_msg": bytes });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    CaToCmBytes::TunnelUp((sender_id, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_up_tunnel" };
                        let trace = json!({"cell_id": &self.cell_id, "sender id": sender_id, "app_msg": bytes });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
            }
        }
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
                            let trace = json!({ "cell_id": &self.cell_id, "user_mask": user_mask, "msg": packet.to_string()? });
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
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                match &packet {
                    PeToCmPacket::Packet((_, packet)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_packet" };
                        let trace = json!({ "cell_id": self.cell_id, "packet": packet.to_string()? });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    PeToCmPacket::Status((port_no, is_border, number_of_packets, status)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_status" };
                        let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status});
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
            }
        }
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
                self.process_packet(port_no, packet)?;
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
                    let msg: Box<dyn Message> = serde_json::from_str(&bytes.to_string()?)?;
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_bytes_last" };
                    let trace = json!({ "cell_id": &self.cell_id, "port": port_no, 
                        "is_ait": is_ait, "tree_uuid": uuid, "msg": msg });
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
}
