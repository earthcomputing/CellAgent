use std::{
    fmt,
    collections::{HashMap, },
};

use crossbeam::crossbeam_channel as mpsc;
use crate::blueprint::{Blueprint, };
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{PortToPePacketOld, PortToPeOld};
use crate::link::{LinkStatus};
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{CommonPortLike, InteriorPortLike, PortSeed, BasePort, InteriorPortFactoryLike, 
                  PortStatusOld, DuplexPortPeOrCaChannel, DuplexPortPeChannel};
use crate::utility::{CellNo, PortNo, PortNumber, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

#[derive(Clone, Debug)]
pub struct DuplexPortLinkChannel {
    port_from_link: PortFromLink,
    port_to_link: PortToLink,
}
impl DuplexPortLinkChannel {
    pub fn new(port_from_link: PortFromLink, port_to_link: PortToLink) -> DuplexPortLinkChannel {
        DuplexPortLinkChannel { port_from_link, port_to_link }
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
    fn recv_from_link(&mut self) -> Result<LinkToPortPacket, Error> {
        let _f = "recv";
        match &self.duplex_port_link_channel {
            Some(connected_duplex_port_link_channel) => {
                let msg = connected_duplex_port_link_channel.port_from_link.recv()?;
                Ok(msg)
            },
            None => Err(SimulatedInteriorPortError::ChannelClosed { func_name: _f, port_no: self.base_port.get_port_no(), cell_id: self.base_port.get_cell_id()}.into()),
        }
    }
    fn direct_send(&mut self, packet: &Packet) -> Result<(), Error> {
        let _f = "direct_send";
        {
            if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
               (!packet.is_entl() || CONFIG.trace_options.entl)  {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "direct_send" };
                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait state": packet.get_ait_state(), "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
	    }
        self.failover_info.save_packet(&packet);
        match &self.duplex_port_link_channel {
            Some(connected_duplex_port_link_channel) => {
                Ok(connected_duplex_port_link_channel.port_to_link.send(*packet).context(SimulatedInteriorPortError::Chain {func_name: "new",comment: S("")})?)
            },
            None => Err(SimulatedInteriorPortError::SendDisconnected { func_name: _f, port_no: self.base_port.get_port_no(), cell_id: self.base_port.get_cell_id()}.into()),
        }
    }
}
impl fmt::Display for SimulatedInteriorPort {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let is_connected = if self.is_connected { "" } else { " not" };
        write!(_f, "SimulatedInteriorPort {}: is{} Failover info {}", self.base_port.get_id(), is_connected, self.failover_info)
    }
}

impl CommonPortLike for SimulatedInteriorPort {
    fn get_base_port(&self) -> &BasePort {
        return &self.base_port;
    }
    fn get_whether_connected(&self) -> bool { return self.is_connected; }
    fn set_connected(&mut self) -> () { self.is_connected = true; }
    fn set_disconnected(&mut self) -> () { self.is_connected = false; }
}

impl InteriorPortLike for SimulatedInteriorPort {
    fn send_to_link(self: &mut Self, packet: &mut Packet) -> Result<(), Error> {
        let _f = "send_to_link";
        {
            if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
               (!packet.is_entl() || CONFIG.trace_options.entl) {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_from_port" };
                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait state": packet.get_ait_state(), "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
	    }
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
            AitState::Init | 
            AitState::SnakeD |
            AitState::Normal => ()
        }
	    self.direct_send(packet)
    }
    fn listen_link(self: &mut Self, port_to_pe: &PortToPeOld) -> Result<(), Error> {
        let _f = "listen_link";
        loop {
            let msg = self.recv_from_link().context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " recv from link"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    match &msg {
                        LinkToPortPacket::Packet(packet) => {
                            if !packet.is_entl() || CONFIG.trace_options.entl {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_packet" };
                                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": packet.get_ait_state(), "packet": packet.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
                        },
                        LinkToPortPacket::Status(status) => {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_status" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "status": status, "msg": msg});
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        },
                    }
                }
            }
            match msg {
                LinkToPortPacket::Status(status) => {
                    match status {
                        LinkStatus::Connected => self.set_connected(),
                        LinkStatus::Disconnected => self.set_disconnected()
                    };
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "status": status });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    let status_old = match status {
                        LinkStatus::Connected => PortStatusOld::Connected,
                        LinkStatus::Disconnected => PortStatusOld::Disconnected
                    };
                    port_to_pe.send(PortToPePacketOld::Status((self.base_port.get_port_no(), self.base_port.is_border(), status_old))).context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " send status to pe"})?;
                }
                LinkToPortPacket::Packet(mut packet) => {
                    self.failover_info.clear_saved_packet();
                    let ait_state = packet.get_ait_state();
                    match ait_state {
                        AitState::Ait  |
                        AitState::AitD => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.base_port.get_id(), ait_state }.into()),

                        AitState::Init   |
                        AitState::SnakeD |
                        AitState::Normal => {
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": packet.get_ait_state(), "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            port_to_pe.send(PortToPePacketOld::Packet((self.base_port.get_port_no(), packet)))?;
                        },
                        AitState::Teck => {
                            packet.next_ait_state()?;
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link_tack" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
                        }
                        AitState::Tack => {
                            packet.next_ait_state()?;
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link_tuck" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
                        }
                        AitState::Tuck => {
                            packet.next_ait_state()?;
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link_tyck" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
                            packet.make_ait();
                           {
                            if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                            (!packet.is_entl() || CONFIG.trace_options.entl) {
                             let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_ait_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                           port_to_pe.send(PortToPePacketOld::Packet((self.base_port.get_port_no(), packet)))?;                           
                        }
                        AitState::Tyck => {
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_aitd_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": packet.get_ait_state(), "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            packet.next_ait_state()?;
                            // TODO: Send AITD as acknowledgement that transfer completed correctly
                            let mut tick_packet: Packet = Default::default();
                            tick_packet.make_tick();
                            self.direct_send(&tick_packet)?;
                        }
                        AitState::Tick | 
                        AitState::Tock => {
                            packet.next_ait_state()?;
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.direct_send(&packet)?;
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
        let duplex_port_link_channel = duplex_port_link_channel_port_map.get(&port_no).cloned();
        Ok( SimulatedInteriorPort {
            base_port: BasePort::new(
                cell_id,
                port_number,
                false,
                DuplexPortPeOrCaChannel::Interior(duplex_port_pe_channel),
            )?,
            is_connected: false,
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

// Link to Port
type PACKET = Packet;
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacket {
    Status(LinkStatus),
    Packet(PACKET),
}
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;

// Port to Link
pub type PortToLinkPacket = Packet; // SimulatedPacket
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;

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
    ChannelClosed { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::SendDisconnected {} Attempt to send on disconnected port {} of cell {}", func_name, port_no,  cell_id)]
    SendDisconnected { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "SimulatedInteriorPortError::Ait {} state {} is not allowed here on port {}", func_name, ait_state, port_id)]
    Ait { func_name: &'static str, port_id: PortID, ait_state: AitState },
}
