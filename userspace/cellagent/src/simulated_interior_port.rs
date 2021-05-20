/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
 #![allow(unused_import_braces)]
 use std::{
    fmt,
    thread,
    collections::HashMap,
};

use crossbeam::crossbeam_channel as mpsc;
use crate::blueprint::{Blueprint};
use crate::config::{CONFIG};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message_formats::{PortToPePacket};
use crate::link::{LinkStatus};
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
#[cfg(feature = "api-old")]
use crate::packet_engine::NumberOfPackets;
use crate::port::{CommonPortLike, FailoverInfo, InteriorPortLike, PortSeed, BasePort, InteriorPortFactoryLike, 
                  DuplexPortPeOrCaChannel, DuplexPortPeChannel};
use crate::port::PortStatus;
use crate::utility::{CellNo, PortNo, PortNumber, S, TraceHeaderParams, TraceType, write_err};
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
    init_packet: Packet,
    duplex_port_link_channel: Option<DuplexPortLinkChannel>,
}

impl SimulatedInteriorPort {
    pub fn new(base_port: BasePort, failover_info: FailoverInfo, is_connected: bool, 
        duplex_port_link_channel: Option<DuplexPortLinkChannel>) -> SimulatedInteriorPort {
            SimulatedInteriorPort { base_port, failover_info, is_connected, 
                duplex_port_link_channel, init_packet: Packet::make_init_packet() }
    }
    fn get_port_no(&self) -> PortNo { self.base_port.get_port_no() }
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
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), 
                            "ait state": packet.get_ait_state(), "packet": packet.stringify()? });
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
    fn get_whether_connected(&self) -> bool { self.is_connected }
    fn set_connected(&mut self) { self.is_connected = true; }
    fn set_disconnected(&mut self) { self.is_connected = false; }
}

impl InteriorPortLike for SimulatedInteriorPort {
    fn listen_link_and_pe(&mut self) {
        let _f = "listen_link_and_pe_loops";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let port_clone = self.clone();
        let thread_name = format!("Port {} listen_link", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen().map_err(|e| write_err("port listen link", &e));
            if CONFIG.continue_on_error { port.listen().map_err(|e| write_err("port continue listen link", &e)).ok();  }
        }).expect("thread failed");
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_pe", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe_loop().map_err(|e| write_err("port listen pe", &e));
            if CONFIG.continue_on_error { port.listen_pe_loop().map_err(|e| write_err("port continue listen pe", &e)).ok(); }
        }).expect("thread failed");
    }
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
    fn listen_link(self: &mut Self, port_pe: &DuplexPortPeChannel) -> Result<(), Error> {
        let _f = "listen_link";
        let port_to_pe = port_pe.get_port_to_pe();
        let port_no = self.get_port_no();
        loop {
            let msg = self.recv_from_link().context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " recv from link"})?;
            self.base_port.update_activity_data();
            match msg {
                LinkToPortPacket::Status(link_status) => {
                    #[cfg(feature = "api-new")]
                    {                   
                        match status {
                            LinkStatus::Connected => {
                                self.set_connected();
                                port_to_pe.send(PortToPePacket::Status((port_no, self.base_port.is_border(), PortStatus::Connected)))?;
                                self.direct_send(&self.init_packet.clone())?;
                            },
                            LinkStatus::Disconnected => {
                                self.set_disconnected();
                                let failover_info = FailoverInfo::new(port_no); // TODO: provide actual failover info
                                self.init_packet = Packet::make_init_packet(); // To get a different random value
                                port_to_pe.send(PortToPePacket::Status((port_no, self.base_port.is_border(), PortStatus::Disconnected(failover_info))))?;
                            }
                        };
                    }
                    #[cfg(feature = "api-old")]
                    {
                        let status = match link_status {
                            LinkStatus::Connected => {
                                self.set_connected();
                                PortStatus::Connected
                            },
                            LinkStatus::Disconnected => {
                                self.set_disconnected();
                            PortStatus::Disconnected
                            }
                        };
                        #[cfg(feature = "api-old")]
                        let status_msg = (port_no, false, status, NumberOfPackets::new());
                        #[cfg(feature = "api-new")]
                        let status_msg = (port_no, self.base_port.is_border(), status);
                        port_to_pe.send(PortToPePacket::Status(status_msg)).context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.base_port.get_id().get_name()) + " send status to pe"})?;
                    }   
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                            let trace = json!({ "cell_id": self.base_port.get_cell_id(), "port_no": port_no, 
                                "activity_data": self.base_port.get_activity_data(), "init_packet": self.init_packet, "link status": link_status });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                }
                LinkToPortPacket::Packet(mut packet) => {
                    self.failover_info.clear_saved_packet();
                    let ait_state = packet.get_ait_state();
                    match ait_state {
                        AitState::Ait  |
                        AitState::AitD => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.base_port.get_id(), ait_state }.into()),

                        AitState::Init => {
                            let recd_init_val = packet.get_unique_msg_id();
                            let mut my_init_packet = self.init_packet;
                            let my_init_val = my_init_packet.get_unique_msg_id();
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "init_from_link" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), 
                                        "ait_state": packet.get_ait_state(), "recd_init_val": recd_init_val, "my_init_val": my_init_val });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            if my_init_val > recd_init_val {
                                my_init_packet.make_tick();
                                self.direct_send(&my_init_packet)?;
                            } else if my_init_val == recd_init_val {
                                return Err(SimulatedInteriorPortError::Init { func_name: _f, port_no, cell_id: self.get_cell_id() }.into() )
                            }
                        }
                        AitState::SnakeD |
                        AitState::Normal => {
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "activity_data": self.base_port.get_activity_data(), "ait_state": packet.get_ait_state(), "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            #[cfg(feature = "api-old")]
                            port_to_pe.send(PortToPePacket::Packet((self.base_port.get_port_no(), packet)))?;
                            #[cfg(feature = "api-new")]
                            port_to_pe.send(PortToPePacket::Packet((port_no, packet)))?;
                        },
                        AitState::Teck => {
                            packet.next_ait_state()?;
                            self.direct_send(&packet)?;
                        }
                        AitState::Tack => {
                            packet.next_ait_state()?;
                            self.direct_send(&packet)?;
                        }
                        AitState::Tuck => {
                            packet.next_ait_state()?;
                            self.direct_send(&packet)?;
                            packet.make_ait();
                           {
                            if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                            (!packet.is_entl() || CONFIG.trace_options.entl) {
                             let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_ait_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "activity_data": self.base_port.get_activity_data(), "ait_state": ait_state, "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            #[cfg(feature = "api-old")]
                            port_to_pe.send(PortToPePacket::Packet((self.base_port.get_port_no(), packet)))?;
                            #[cfg(feature = "api-new")]
                            port_to_pe.send(PortToPePacket::Packet((self.base_port.get_port_no(), packet)))?;
                        }
                        AitState::Tyck => {
                            packet.next_ait_state()?;
                            {
                                if (CONFIG.trace_options.all || CONFIG.trace_options.port) && 
                                   (!packet.is_entl() || CONFIG.trace_options.entl) {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_aitd_packet" };
                                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "activity_data": self.base_port.get_activity_data(), "ait_state": packet.get_ait_state(), "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            port_to_pe.send(PortToPePacket::Packet((self.base_port.get_port_no(), packet)))?;
                            let mut tick_packet: Packet = Default::default();
                            tick_packet.make_tick();
                            self.direct_send(&tick_packet)?;
                        }
                        AitState::Tick | 
                        AitState::Tock => {
                            packet.next_ait_state()?;
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
    fn new_port(&self, cell_id: CellID, port_id: PortID, port_number: PortNumber, duplex_port_pe_channel: DuplexPortPeChannel) 
            -> Result<SimulatedInteriorPort, Error> {
        let _f = "new_port";
        let cell_no = self.cell_no_map[&cell_id.get_name()];
        let port_no = port_number.get_port_no();
        let duplex_port_link_channel_port_map = &self.duplex_port_link_channel_cell_port_map[&cell_no];
        let duplex_port_link_channel = duplex_port_link_channel_port_map.get(&port_no).cloned();
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "create_port" };
                let trace = json!({ "cell_id": cell_id, "port_id": port_id, "port_no": port_no });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        Ok(SimulatedInteriorPort::new(BasePort::new(cell_id, port_number, false,
                                                    DuplexPortPeOrCaChannel::Interior(duplex_port_pe_channel))?,
            FailoverInfo::new(port_number.get_port_no()), false, duplex_port_link_channel))
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

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum SimulatedInteriorPortError {
    #[fail(display = "SimulatedInteriorPortError::Ait {} state {} is not allowed here on port {}", func_name, ait_state, port_id)]
    Ait { func_name: &'static str, port_id: PortID, ait_state: AitState },
    #[fail(display = "SimulatedInteriorPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "SimulatedInteriorPortError::RecvDisconnected {} Attempt to receive on disconnected port {} of cell {}", func_name, port_no,  cell_id)]
    ChannelClosed { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::Init {} Init values are equal on port {} of cell {}", func_name, port_no,  cell_id)]
    Init { func_name: &'static str, cell_id: CellID, port_no: PortNo },
    #[fail(display = "SimulatedInteriorPortError::SendDisconnected {} Attempt to send on disconnected port {} of cell {}", func_name, port_no,  cell_id)]
    SendDisconnected { func_name: &'static str, cell_id: CellID, port_no: PortNo },
 }
