use std::{
    fmt,
    collections::{HashMap, },
};

use crossbeam::crossbeam_channel as mpsc;
use crate::blueprint::{Blueprint, };
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{PortToPePacketOld, PortToPeOld};
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{CommonPortLike, InteriorPortLike, PortSeed, BasePort, InteriorPortFactoryLike, 
    PortStatus, PortStatusOld, DuplexPortPeOrCaChannel, DuplexPortPeChannel};
use crate::utility::{CellNo, PortNo, PortNumber, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

#[derive(Clone, Debug)]
pub struct DuplexPortLinkChannel {
    port_from_link_old: PortFromLinkOld,
    port_to_link_old: PortToLinkOld,
}
impl DuplexPortLinkChannel {
    pub fn new(port_from_link_old: PortFromLinkOld, port_to_link_old: PortToLinkOld) -> DuplexPortLinkChannel {
        DuplexPortLinkChannel { port_from_link_old, port_to_link_old }
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedInteriorPort {
    base_port: BasePort,
    failover_info: FailoverInfo,
    is_connected: bool,
    duplex_port_link_channel: Option<DuplexPortLinkChannel>,
}

impl SimulatedInteriorPort {
    fn recv(&mut self) -> Result<LinkToPortPacketOld, Error> {
        let _f = "recv";
        match &self.duplex_port_link_channel {
            Some(connected_duplex_port_link_channel) => {
                let link_to_port_packet_old = connected_duplex_port_link_channel.port_from_link_old.recv()?;
                {
                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_receive" };
                        let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "link_to_port_packet": link_to_port_packet_old });
                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
	            }
                self.failover_info.clear_saved_packet();
                Ok(link_to_port_packet_old)
            },
            None => Err(SimulatedInteriorPortError::RecvDisconnected { func_name: _f, port_no: self.base_port.get_port_no(), cell_id: self.base_port.get_cell_id()}.into()),
        }
    }
    fn direct_send(&mut self, packet: &Packet) -> Result<(), Error> {
        let _f = "direct_send";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_direct_send" };
                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "packet": packet });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
	    }
        self.failover_info.save_packet(&packet);
        match &self.duplex_port_link_channel {
            Some(connected_duplex_port_link_channel) => Ok(connected_duplex_port_link_channel.port_to_link_old.send(*packet).context(SimulatedInteriorPortError::Chain {func_name: "new",comment: S("")})?),
            None => Err(SimulatedInteriorPortError::SendDisconnected { func_name: _f, port_no: self.base_port.get_port_no(), cell_id: self.base_port.get_cell_id()}.into()),
        }
    }
}
impl fmt::Display for SimulatedInteriorPort {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(_f, "SimulatedInteriorPort {}: Failover info {}", self.base_port.get_id(), self.failover_info)
    }
}

impl CommonPortLike for SimulatedInteriorPort {
    fn get_base_port(&self) -> &BasePort {
        return &(*self).base_port;
    }
    fn get_base_port_mut(&mut self) -> &mut BasePort {
        return &mut (*self).base_port;
    }
    fn get_whether_connected(&self) -> bool { return self.is_connected; }
    fn set_connected(&mut self) -> () { self.is_connected = true; }
    fn set_disconnected(&mut self) -> () { self.is_connected = false; }
}

impl InteriorPortLike for SimulatedInteriorPort {
    fn send(self: &mut Self, packet: &mut Packet) -> Result<(), Error> {
        let _f = "send";
        let ait_state = packet.get_ait_state();
        match ait_state {
            AitState::AitD |
            AitState::Tick |
            AitState::Tock |
            AitState::Tack |
            AitState::Teck |
            AitState::Tuck |
            AitState::Tyck => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.base_port.get_id(), ait_state }.into()), // Not allowed here
            AitState::Ait => { packet.next_ait_state()?; },
            AitState::Init | // Only needed for simulator, should be handled by simulated_internal_port
            AitState::SnakeD |
            AitState::Normal => ()
        }
	    self.direct_send(packet)
    }
    fn listen_and_forward_to(self: &mut Self, port_to_pe_old: PortToPeOld) -> Result<(), Error> {
        let _f = "listen_and_forward_to";
        let mut msg: LinkToPortPacketOld;
        loop {
            msg = self.recv().context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " recv from link"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    match &msg {
                        LinkToPortPacketOld::Packet(packet) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_packet" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                        LinkToPortPacketOld::Status(status) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_status" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "status": status, "msg": msg});
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                    }
                }
            }
            match msg {
                LinkToPortPacketOld::Status(status) => {
                    match status {
                        PortStatusOld::Connected => self.set_connected(),
                        PortStatusOld::Disconnected => self.set_disconnected()
                    };
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "status": status });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    port_to_pe_old.send(PortToPePacketOld::Status((self.base_port.get_port_no(), self.base_port.is_border(), status))).context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " send status to pe"})?;
                }
                LinkToPortPacketOld::Packet(mut packet) => {
                    let ait_state = packet.get_ait_state();
                    match ait_state {
                        AitState::AitD |
                        AitState::Ait => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.base_port.get_id(), ait_state }.into()),

                        AitState::Tick => (), // TODO: Send AitD to packet engine
                        AitState::Init |
                        AitState::SnakeD |
                        AitState::Normal => {
                            {
                                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            port_to_pe_old.send(PortToPePacketOld::Packet((self.base_port.get_port_no(), packet)))?;
                        },
                        AitState::Teck |
                        AitState::Tack |
                        AitState::Tuck |
                        AitState::Tyck => {
                            packet.next_ait_state()?;
                            {
                                if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
                        }
                        AitState::Tock => {
                            packet.next_ait_state()?;
                            {
                                if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
                            packet.make_ait_send();
                            {
                                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            port_to_pe_old.send(PortToPePacketOld::Packet((self.base_port.get_port_no(), packet)))?;
                        },
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedInteriorPortFactory {
    port_seed: PortSeed,
    cell_no_map: HashMap<String, CellNo>,
    blueprint: Blueprint,
    duplex_port_link_channel_cell_port_map: HashMap<CellNo, HashMap<PortNo, DuplexPortLinkChannel>>,
}

impl SimulatedInteriorPortFactory {
    pub fn new(port_seed: PortSeed, cell_no_map: HashMap<String, CellNo>, blueprint: Blueprint, 
               duplex_port_link_channel_cell_port_map: HashMap<CellNo, HashMap<PortNo, DuplexPortLinkChannel>>) 
                    -> SimulatedInteriorPortFactory {
        SimulatedInteriorPortFactory { port_seed, cell_no_map, blueprint, duplex_port_link_channel_cell_port_map }
    }
}

impl InteriorPortFactoryLike<SimulatedInteriorPort> for SimulatedInteriorPortFactory {
    fn new_port(&self, cell_id: CellID, port_id: PortID, port_number: PortNumber, duplex_port_pe_channel: DuplexPortPeChannel) -> Result<SimulatedInteriorPort, Error> {
        let cell_no = self.cell_no_map[&cell_id.get_name()];
        let port_no = port_number.get_port_no();
        let duplex_port_link_channel_port_map = &self.duplex_port_link_channel_cell_port_map[&cell_no];
        let duplex_port_link_channel = if duplex_port_link_channel_port_map.contains_key(&port_no) {
            Some(duplex_port_link_channel_port_map[&port_no].clone())
        } else {
            None
        };
        Ok( SimulatedInteriorPort {
            base_port: BasePort::new(
                cell_id,
                port_number,
                false,
                DuplexPortPeOrCaChannel::Interior(duplex_port_pe_channel),
            )?,
            is_connected: duplex_port_link_channel_port_map.contains_key(&port_no),
            duplex_port_link_channel,
            failover_info: FailoverInfo::new(port_id),
        })
    }
    fn get_port_seed(&self) -> &PortSeed {
        return &self.port_seed;
    }
    fn get_port_seed_mut(&mut self) -> &mut PortSeed {
        return &mut self.port_seed;
    }
}
// Link to Port Old
type PACKET = Packet;
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacket {
    Status(PortStatus),
    Packet(PACKET),
}
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;

// Port to Link
pub type PortToLinkPacket = PACKET; // SimulatedPacket
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;

// Link to Port Old
type PACKETOLD = Packet;
pub type PortToLinkOld = mpsc::Sender<PortToLinkPacketOld>;
pub type PortFromLinkOld = mpsc::Receiver<LinkToPortPacketOld>;
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacketOld {
    Status(PortStatusOld),
    Packet(PACKETOLD),
}
pub type LinkToPortOld = mpsc::Sender<LinkToPortPacketOld>;

// Port to Link
pub type PortToLinkPacketOld = Packet; // SimulatedPacket
pub type LinkFromPortOld = mpsc::Receiver<PortToLinkPacketOld>;

#[derive(Debug, Copy, Clone, Serialize)]
pub struct FailoverInfo {
    port_id: PortID,
    packet_opt: Option<Packet>
}
impl FailoverInfo {
    pub fn new(port_id: PortID) -> FailoverInfo { 
        FailoverInfo { port_id, packet_opt: Default::default() }
    }
    pub fn if_sent(&self) -> bool { self.packet_opt.is_some() }
    pub fn if_recd(&self) -> bool { self.packet_opt.is_none() }
    pub fn get_saved_packet(&self) -> Option<Packet> { self.packet_opt }
    // Call on every data packet send
    fn save_packet(&mut self, packet: &Packet) {
        self.packet_opt = Some(packet.clone());
    }
    // Call on every data packet receive
    fn clear_saved_packet(&mut self) {
        self.packet_opt = None;
    }
}
impl fmt::Display for FailoverInfo {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let packet_out = match self.packet_opt {
            Some(p) => p.stringify().expect("Failover Display: Stringify packet must succeed"),
            None => "None".to_string()
        };
        write!(_f, "PortID {} Sent {}, Recd {}, Packet {:?}", self.port_id, self.if_sent(), self.if_recd(), packet_out)
    }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum SimulatedInteriorPortError {
    #[fail(display = "SimulatedInteriorPortError::RecvDisconnected {} Attempt to receive on disconnected port {} of cell {}", func_name, port_no,  cell_id)]
    RecvDisconnected { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::SendDisconnected {} Attempt to send on disconnected port {} of cell {}", func_name, port_no,  cell_id)]
    SendDisconnected { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "SimulatedInteriorPortError::Ait {} {} is not allowed here on port {}", func_name, port_id, ait_state)]
    Ait { func_name: &'static str, port_id: PortID, ait_state: AitState },
}
