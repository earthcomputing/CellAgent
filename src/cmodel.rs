use std::fmt;
use std::thread::JoinHandle;

use failure::{Error, ResultExt};

use config::{CONTINUE_ON_ERROR, DEBUG_OPTIONS, PortNo};
use dal;
use message::MsgType;
use message_types::{CaToCmBytes, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCmPacket,
                    CmToPePacket, CmToCaBytes};
use name::{Name, CellID};
use packet::{Packet, PacketAssembler, PacketAssemblers, Packetizer};
use utility::{S, TraceHeader, TraceHeaderParams, TraceType, write_err};

const MODULE: &'static str = "cmodel.rs";
#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
    packet_assemblers: PacketAssemblers,
}
impl Cmodel {
    pub fn new(cell_id: &CellID) -> Cmodel {
        Cmodel { cell_id: cell_id.clone(), packet_assemblers: PacketAssemblers::new() }
    }
    pub fn initialize(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe, cm_from_pe: CmFromPe, cm_to_ca: CmToCa,
                      mut trace_header: TraceHeader) -> Result<(), Error> {
        self.listen_ca(cm_from_ca, cm_to_pe, trace_header.fork_trace())?;
        self.listen_pe(cm_from_pe, cm_to_ca, trace_header.fork_trace())?;
        Ok(())
    }
    fn listen_ca(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe,
                 mut outer_trace_header: TraceHeader) -> Result<JoinHandle<()>, Error> {
        let f = "listen_ca";
        let cmodel = self.clone();
        let join_handle = ::std::thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header.fork_trace();
            let _ = cmodel.listen_ca_loop(&cm_from_ca, &cm_to_pe, inner_trace_header).map_err(|e| write_err("cmodel listen_ca", e.into()));
            if CONTINUE_ON_ERROR { let _ = cmodel.listen_ca(cm_from_ca, cm_to_pe, outer_trace_header); }
        });
        Ok(join_handle)
    }
    fn listen_pe(&self, cm_from_pe: CmFromPe, cm_to_ca: CmToCa, mut outer_trace_header: TraceHeader) -> Result<JoinHandle<()>, Error> {
        let f = "listen_pe";
        let mut cmodel = self.clone();
        let join_handle = ::std::thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header.fork_trace();
            let _ = cmodel.listen_pe_loop(&cm_from_pe, &cm_to_ca, inner_trace_header).map_err(|e| write_err("cmodel listen_pe", e.into()));;
            if CONTINUE_ON_ERROR { let _ = cmodel.listen_pe(cm_from_pe, cm_to_ca, outer_trace_header); }
        });
        Ok(join_handle)
    }
    fn listen_ca_loop(&self, cm_from_ca: &CmFromCa, cm_to_pe: &CmToPe,
                      trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "listen_ca_loop";
        loop {
            match cm_from_ca.recv()? {
                CaToCmBytes::Entry(entry) => cm_to_pe.send(CmToPePacket::Entry(entry)),
                CaToCmBytes::Bytes((tree_id, is_ait, user_mask, is_blocking, bytes)) => {
                    if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.cm_from_ca {   //Debug print
                        let msg = MsgType::msg_from_bytes(&bytes)?;
                        let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "cm_bytes_from_ca" };
                        let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.value() });
                        if DEBUG_OPTIONS.cm_from_ca {
                            match msg.get_msg_type() {
                                MsgType::Discover => (),
                                MsgType::DiscoverD => {
                                    if msg.get_tree_id().unwrap().is_name("Tree:C:2") {
                                        println!("Cmodel {}: {} received {}", self.cell_id, f, msg);
                                    }
                                },
                                _ => {
                                    println!("Cmodel {}: {} received {}", self.cell_id, f, msg);
                                }
                            }
                        }
                        let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
                    }
                    let mut uuid = tree_id.get_uuid();
                    if is_ait { uuid.make_ait(); }
                    let packets = Packetizer::packetize(&uuid, &bytes, is_blocking);
                    for packet in packets {
                        cm_to_pe.send(CmToPePacket::Packet((user_mask, packet)))?;
                    }
                    Ok(())
                },
                CaToCmBytes::Tcp((port_number, msg)) => cm_to_pe.send(CmToPePacket::Tcp((port_number, msg))),
                CaToCmBytes::Unblock => cm_to_pe.send(CmToPePacket::Unblock)
            }?;
        }
    }
    fn listen_pe_loop(&mut self, cm_from_pe: &CmFromPe, cm_to_ca: &CmToCa,
                      trace_header: &mut TraceHeader) -> Result<(), Error> {
        loop {
            match cm_from_pe.recv()? {
                PeToCmPacket::Status((port_no,bool, port_status)) => cm_to_ca.send(CmToCaBytes::Status((port_no,bool, port_status)))?,
                PeToCmPacket::Packet((port_no, packet)) => self.process_packet(cm_to_ca, port_no, packet, trace_header)?,
                PeToCmPacket::Tcp((port_no, tcp_msg)) => cm_to_ca.send(CmToCaBytes::Tcp((port_no, tcp_msg)))?
            };
        }
    }
    fn process_packet(&mut self, cm_to_ca: &CmToCa, port_no: PortNo, packet: Packet,
                      trace_header: &mut TraceHeader) -> Result<(), Error> {
        let f = "process_packet";
        let msg_id = packet.get_msg_id();
        let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
        let (last_packet, packets) = packet_assembler.add(packet);
        if last_packet {
            let uuid = packet.get_tree_uuid();
            let bytes = Packetizer::unpacketize(packets).context(CmodelError::Chain { func_name: f, comment: S("") })?;
            if DEBUG_OPTIONS.trace_all || DEBUG_OPTIONS.cm_from_ca {   //Debug print
                let msg = MsgType::msg_from_bytes(&bytes)?;
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "cm_bytes_to_ca" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.value() });
                if DEBUG_OPTIONS.cm_from_ca {
                    match msg.get_msg_type() {
                        MsgType::Discover => (),
                        MsgType::DiscoverD => {
                            if msg.get_tree_id().unwrap().is_name("Tree:C:2") {
                                println!("Cmodel {}: {} received {}", self.cell_id, f, msg);
                            }
                        },
                        _ => {
                            println!("Cmodel {}: {} received {}", self.cell_id, f, msg);
                        }
                    }
                }
                let _ = dal::add_to_trace(trace_header, TraceType::Debug, trace_params, &trace, f);
            }
            let is_ait = packets[0].is_ait();
            cm_to_ca.send(CmToCaBytes::Bytes((port_no, is_ait, uuid, bytes)))?;
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
