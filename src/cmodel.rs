use std::{fmt,
          thread,
          thread::JoinHandle};

use failure::{Error, ResultExt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message::{Message, MsgType};
use crate::ec_message_formats::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCmPacket,
                                CmToPePacket, CmToCaBytes};
use crate::name::{Name, CellID};
use crate::packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use crate::utility::{PortNo, S, TraceHeader, TraceHeaderParams, TraceType, write_err};

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_assemblers: PacketAssemblers,
}
impl Cmodel {
    pub fn get_name(&self) -> String { self.cell_id.get_name() }
    //pub fn get_cell_id(&self) -> &CellID { &self.cell_id }
    // NEW
    pub fn new(cell_id: CellID) -> Cmodel {
        Cmodel { cell_id, packet_assemblers: PacketAssemblers::new() }
    }

    // INIT (CmFromCa, CmFromPe)
    // WORKER (CModel)
    pub fn initialize(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe,
                      cm_from_pe: CmFromPe, cm_to_ca: CmToCa) -> Result<(), Error> {
        let _f = "initialize";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.listen_ca(cm_from_ca, cm_to_pe, cm_to_ca.clone())?;
        self.listen_pe(cm_from_pe, cm_to_ca)?;
        Ok(())
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe, cm_to_ca: CmToCa) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_ca";
        let cmodel = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {} listen_ca_loop", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = cmodel.listen_ca_loop(&cm_from_ca, &cm_to_pe, &cm_to_ca).map_err(|e| write_err("cmodel listen_ca", &e));
            if CONFIG.continue_on_error { let _ = cmodel.listen_ca(cm_from_ca, cm_to_pe, cm_to_ca); }
        })?;
        Ok(join_handle)
    }

    // SPAWN THREAD (listen_pe_loop)
    fn listen_pe(&self, cm_from_pe: CmFromPe, cm_to_ca: CmToCa) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_pe";
        let mut cmodel = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {} listen_pe_loop", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = cmodel.listen_pe_loop(&cm_from_pe, &cm_to_ca).map_err(|e| write_err("cmodel listen_pe", &e));;
            if CONFIG.continue_on_error { let _ = cmodel.listen_pe(cm_from_pe, cm_to_ca); }
        })?;
        Ok(join_handle)
    }

    // WORKER (CmFromCa)
    fn listen_ca_loop(&self, cm_from_ca: &CmFromCa, cm_to_pe: &CmToPe, cm_to_ca: &CmToCa) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = cm_from_ca.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                    match &msg {
                        CaToCmBytes::Bytes((_, _, _, bytes)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes" };
                            let trace = json!({"cell_id": &self.cell_id, "msg": bytes.to_string()? });
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
                            let trace = json!({"cell_id": &self.cell_id, "port_no": port_no, "app_msg": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        CaToCmBytes::TunnelUp((sender_id, bytes)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_ca_bytes(up tunnel)" };
                            let trace = json!({"cell_id": &self.cell_id, "sender id": sender_id, "app_msg": bytes.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                }
            }
            match msg {
                // just forward to PE
                CaToCmBytes::Reroute((broken_port, new_parent, number_of_packets)) => {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_reroute" };
                        let trace = json!({ "cell_id": &self.cell_id, "broken_port": broken_port, "new_parent": new_parent, "no_packets": number_of_packets });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    cm_to_pe.send(CmToPePacket::Reroute((broken_port, new_parent, number_of_packets)))?;
                },
                CaToCmBytes::Entry(entry) => {
                    if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_pe_entry" };
                        let trace = json!({ "cell_id": &self.cell_id, "entry": entry });
                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                    cm_to_pe.send(CmToPePacket::Entry(entry))?;
                },
                CaToCmBytes::Status(status_msg) => {
                    cm_to_ca.send(CmToCaBytes::Status(status_msg))?;
                }
                CaToCmBytes::TunnelPort(tunnel_msg) => {
                    cm_to_ca.send(CmToCaBytes::TunnelPort(tunnel_msg))?;
                }
                CaToCmBytes::TunnelUp(tunnel_msg) => {
                    cm_to_ca.send(CmToCaBytes::TunnelUp(tunnel_msg))?;
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
                            let trace = json!({ "cell_id": &self.cell_id, "msg": packet.to_string()? });
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                        cm_to_pe.send(CmToPePacket::Packet((user_mask, packet)))?;
                    }
                },
            }
        }
    }

    // WORKER (CmFromPe)
    fn listen_pe_loop(&mut self, cm_from_pe: &CmFromPe, cm_to_ca: &CmToCa) -> Result<(), Error> {
        let _f = "listen_pe_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = cm_from_pe.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                    match &msg {
                        PeToCmPacket::Packet((_, packet)) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_from_pe_packet" };
                            let trace = json!({ "cell_id": self.cell_id, "msg": packet.to_string()? });
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
            match msg {
                // just forward to CA
                PeToCmPacket::Status((port_no, is_border, number_of_packets, status)) => {
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.cm {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_to_ca_status" };
                            let trace = json!({ "cell_id": &self.cell_id, "port": port_no, "is_border": is_border, "no_packets": number_of_packets, "status": status});
                            let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    cm_to_ca.send(CmToCaBytes::Status((port_no, is_border, number_of_packets, status)))?
                },

                // de-packetize
                PeToCmPacket::Packet((port_no, packet)) => {
                    self.process_packet(cm_to_ca, port_no, packet)?
                }
            };
        }
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

    fn process_packet(&mut self, cm_to_ca: &CmToCa, port_no: PortNo, packet: Packet) -> Result<(), Error> {
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
            cm_to_ca.send(msg)?;
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
