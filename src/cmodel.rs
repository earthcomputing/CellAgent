use std::fmt;
use std::thread;
use std::thread::JoinHandle;

use failure::{Error, ResultExt};

use config::{CONTINUE_ON_ERROR, DEBUG_OPTIONS, TRACE_OPTIONS, PortNo};
use dal;
use dal::{fork_trace_header, update_trace_header};
use message::MsgType;
use message_types::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCmPacket,
                    CmToPePacket, CmToCaBytes};
use name::{Name, CellID};
use packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use utility::{S, TraceHeader, TraceHeaderParams, TraceType, write_err};

const CENTRAL_TREE : &str = "Tree:C:2"; // MAGIC

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_assemblers: PacketAssemblers,
}
impl Cmodel {
    pub fn get_name(&self) -> &str { self.cell_id.get_name() }
    //pub fn get_cell_id(&self) -> &CellID { &self.cell_id }
    // NEW
    pub fn new(cell_id: &CellID) -> Cmodel {
        Cmodel { cell_id: cell_id.clone(), packet_assemblers: PacketAssemblers::new() }
    }

    // INIT (CmFromCa, CmFromPe)
    // WORKER (CModel)
    pub fn initialize(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe,
                      cm_from_pe: CmFromPe, cm_to_ca: CmToCa) -> Result<(), Error> {
        let _f = "initialize";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        self.listen_ca(cm_from_ca, cm_to_pe)?;
        self.listen_pe(cm_from_pe, cm_to_ca)?;
        Ok(())
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_ca";
        let cmodel = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {} listen_ca_loop", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = cmodel.listen_ca_loop(&cm_from_ca, &cm_to_pe).map_err(|e| write_err("cmodel listen_ca", e.into()));
            if CONTINUE_ON_ERROR { let _ = cmodel.listen_ca(cm_from_ca, cm_to_pe); }
        })?;
        Ok(join_handle)
    }

    // SPAWN THREAD (listen_pe_loop)
    fn listen_pe(&self, cm_from_pe: CmFromPe, cm_to_ca: CmToCa) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_pe";
        let mut cmodel = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {} listen_pe_loop", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = cmodel.listen_pe_loop(&cm_from_pe, &cm_to_ca).map_err(|e| write_err("cmodel listen_pe", e.into()));;
            if CONTINUE_ON_ERROR { let _ = cmodel.listen_pe(cm_from_pe, cm_to_ca); }
        })?;
        Ok(join_handle)
    }

    // WORKER (CmFromCa)
    fn listen_ca_loop(&self, cm_from_ca: &CmFromCa, cm_to_pe: &CmToPe) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = cm_from_ca.recv()?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
                let trace = match &msg {
                    CaToCmBytes::Bytes(msg) => json!({"cell_id": &self.cell_id, "msg": (&msg.0, &msg.1, &msg.2, &msg.3, &msg.4[0..20])}),
                    _ => json!({ "cell_id": &self.cell_id, "msg": &msg })
                };
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_bytes_from_ca" };
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            match msg {
                // just forward to PE
                CaToCmBytes::Entry(entry) => cm_to_pe.send(CmToPePacket::Entry(entry)),
                CaToCmBytes::Tcp((port_number, msg)) => cm_to_pe.send(CmToPePacket::Tcp((port_number, msg))),
                CaToCmBytes::Unblock => cm_to_pe.send(CmToPePacket::Unblock),

                // packetize
                CaToCmBytes::Bytes((tree_id, is_ait, user_mask, is_blocking, bytes)) => {
                    if DEBUG_OPTIONS.cm_from_ca {
                        let dpi_msg = MsgType::msg_from_bytes(&bytes)?;
                        let dpi_msg_type = dpi_msg.get_msg_type();
                        let dpi_tree_id = dpi_msg.get_tree_id();
                        match dpi_msg_type {
                            MsgType::Discover => (),
                            MsgType::DiscoverD => {
                                if dpi_tree_id.is_name(CENTRAL_TREE) { println!("Cmodel {}: {} received {}", self.cell_id, _f, dpi_msg); }
                            },
                            _ => {
                                println!("Cmodel {}: {} received {}", self.cell_id, _f, dpi_msg);
                            }
                        }
                    }
                    // xmit msg
                    {
                        let mut uuid = tree_id.get_uuid();
                        if is_ait { uuid.make_ait(); }

                        let packets = Packetizer::packetize(&uuid, &bytes, is_blocking);
                        let first = packets[0];
                        let dpi_is_ait = first.is_ait();
                        let msg_id = first.get_msg_id();
                        let packet_count = first.get_count();
                        if DEBUG_OPTIONS.cm_from_ca { println!("Cmodel {}: {} packetize - is_ait {} msg_id {} count {}", self.cell_id, _f, dpi_is_ait, *msg_id, packet_count); }
                        for packet in packets {
                            cm_to_pe.send(CmToPePacket::Packet((user_mask, packet)))?;
                        }
                    }
                    Ok(())
                },
            }?;
        }
    }

    // WORKER (CmFromPe)
    fn listen_pe_loop(&mut self, cm_from_pe: &CmFromPe, cm_to_ca: &CmToCa) -> Result<(), Error> {
        let _f = "listen_pe_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = cm_from_pe.recv()?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "listen_pe_loop" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            match msg {
                // just forward to CA
                PeToCmPacket::Status((port_no,bool, port_status)) => cm_to_ca.send(CmToCaBytes::Status((port_no,bool, port_status)))?,
                PeToCmPacket::Tcp((port_no, tcp_msg)) => cm_to_ca.send(CmToCaBytes::Tcp((port_no, tcp_msg)))?,

                // de-packetize
                PeToCmPacket::Packet((port_no, packet)) => self.process_packet(cm_to_ca, port_no, packet)?
            };
        }
    }

/*
header.uuid
payload.msg_id
payload.size
payload.is_last
payload.is_blocking
payload.bytes
packet_count

packet_assembler::
msg_id: MsgID,
packets: Vec<Packet>,
*/

    fn process_packet(&mut self, cm_to_ca: &CmToCa, port_no: PortNo, packet: Packet) -> Result<(), Error> {
        let _f = "process_packet";
        let msg_id = packet.get_msg_id();
        let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id)); // autovivification
        let (last_packet, packets) = packet_assembler.add(packet);

        if last_packet {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.cm {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "cm_bytes_to_ca" };
                let trace = json!({ "cell_id": &self.cell_id, "packet": &packet });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f); // sender side, dup
            }
            let is_ait = packets[0].is_ait();
            let uuid = packet.get_tree_uuid();
            let bytes = Packetizer::unpacketize(packets).context(CmodelError::Chain { func_name: _f, comment: S("") })?;

            if DEBUG_OPTIONS.cm_from_ca {
                let packet_count = packets[0].get_count();
                let dpi_msg = MsgType::msg_from_bytes(&bytes)?;
                let dpi_msg_type = dpi_msg.get_msg_type();
                let dpi_tree_id = dpi_msg.get_tree_id();
                match dpi_msg_type {
                    MsgType::Discover => (),
                    MsgType::DiscoverD => {
                        if dpi_tree_id.is_name(CENTRAL_TREE) { println!("Cmodel {}: {} received {}", self.cell_id, _f, dpi_msg); }
                    },
                    _ => {
                        println!("Cmodel {}: {} received {} count {}", self.cell_id, _f, dpi_msg, packet_count);
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("\nCmodel {}", self.cell_id.get_name());
        write!(f, "{}", s)
    }
}

// Errors
#[derive(Debug, Fail)]
pub enum CmodelError {
    #[fail(display = "NameError::Chain {} {}", func_name, comment)]
        Chain { func_name: &'static str, comment: String },
}
