use std::{fmt,
          thread};

use failure::{Error, ResultExt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message::{Message, MsgType};
use crate::ec_message_formats::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCmPacket,
                                CmToPePacket, CmToCaBytes};
use crate::name::{Name, CellID};
use crate::packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use crate::utility::{PortNo, S, TraceHeader, TraceHeaderParams, TraceType};

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_assemblers: PacketAssemblers,
    cm_to_ca: CmToCa,
    cm_to_pe: CmToPe,
}
impl Cmodel {
    pub fn get_name(&self) -> String { self.cell_id.get_name() }
    pub fn get_cell_id(&self) -> &CellID { &self.cell_id }
    // NEW
    pub fn new(cell_id: CellID, cm_to_ca: CmToCa, cm_to_pe: CmToPe) -> Cmodel {
        Cmodel { cell_id, packet_assemblers: PacketAssemblers::new(), cm_to_ca, cm_to_pe }
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
                    CaToCmBytes::Bytes((_, _, _, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes" };
                        let trace = json!({"cell_id": &self.cell_id, "msg": bytes });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Entry(entry) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(entry)" };
                        let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Reroute((broken_port, new_parent, number_of_packets)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(reroute)" };
                        let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port, "new_parent": new_parent, "no_packets": number_of_packets });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::Status((port_no, is_border, _number_of_packets, status)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(status)" };
                        let trace = json!({"cell_id": &self.cell_id, "port_no": port_no, "is_border": is_border, "status": status });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    },
                    CaToCmBytes::TunnelPort((port_no, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(port tunnel)" };
                        let trace = json!({"cell_id": &self.cell_id, "port_no": port_no, "app_msg": bytes });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    CaToCmBytes::TunnelUp((sender_id, bytes)) => {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(up tunnel)" };
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
            CaToCmBytes::Bytes((tree_id, is_ait, user_mask, bytes)) => {
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
            
                let packets = Packetizer::packetize(&uuid, &bytes);
                let first = packets.get(0).expect("No packets from packetizer");
                let dpi_is_ait = first.is_ait();
                let sender_msg_seq_no = first.get_sender_msg_seq_no();
                let packet_count = first.get_count();
                {
                    if CONFIG.debug_options.all || CONFIG.debug_options.cm_from_ca {
                        println!("Cmodel {}: {} packetize - is_ait {} sender_msg_seq_no {} count {}", self.cell_id, _f, dpi_is_ait, *sender_msg_seq_no, packet_count);
                    }
                }
                for packet in packets {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_packet" };
                        let trace = json!({ "cell_id": &self.cell_id, "msg": packet });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    self.cm_to_pe.send(CmToPePacket::Packet((user_mask, packet)))?;
                }
            },
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
                        let trace = json!({ "cell_id": self.cell_id, "packet": packet });
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
                self.cm_to_ca.send(CmToCaBytes::Status((port_no, is_border, number_of_packets, status)))?
            },
        
            // de-packetize
            PeToCmPacket::Packet((port_no, packet)) => {
                self.process_packet(port_no, packet)?
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
        let sender_msg_seq_no = packet.get_sender_msg_seq_no();
        let mut packet_assembler = self.packet_assemblers.remove(&sender_msg_seq_no).unwrap_or(PacketAssembler::new(sender_msg_seq_no)); // autovivification
        let (last_packet, packets) = packet_assembler.add(packet.clone()); // Need clone only because of trace

        if last_packet {
            let is_ait = packets[0].is_ait();
            let uuid = packet.get_tree_uuid();
            let bytes = Packetizer::unpacketize(&packets).context(CmodelError::Chain { func_name: _f, comment: S("") })?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                    let msg: Box<dyn Message> = serde_json::from_str(&bytes.to_string()?)?;
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_bytes" };
                    let trace = json!({ "cell_id": &self.cell_id, "msg": msg.to_string() });
                    let _ = add_to_trace(TraceType::Debug, trace_params, &trace, _f); // sender side, dup
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
            }
            let msg = CmToCaBytes::Bytes((port_no, is_ait, uuid, bytes));
            self.cm_to_ca.send(msg)?;
        }
        Ok(())
    }
}

impl fmt::Display for Cmodel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("\nCmodel {}", self.cell_id.get_name());
        write!(f, "{}", s)
    }
}

// Errors
#[derive(Debug, Fail)]
pub enum CmodelError {
    #[fail(display = "CmodelError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
