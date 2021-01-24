use std::fmt;

use crossbeam::crossbeam_channel as mpsc;

use crate::app_message_formats::{PortToCa};
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{PortToPePacket, PortToPe};
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{InteriorPortLike, PortData, PortStatus};
use crate::simulated_border_port::{SimulatedBorderPort};
use crate::utility::{PortNumber, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;

#[derive(Clone, Debug)]
pub struct DuplexPortLinkChannel {
    pub port_from_link: PortFromLink,
    pub port_to_link: PortToLink,
}

#[derive(Clone, Debug)]
pub struct SimulatedInteriorPort {
    port: PortData<SimulatedInteriorPort, SimulatedBorderPort>,
    failover_info: FailoverInfo,
    duplex_port_link_channel: DuplexPortLinkChannel,
}
impl SimulatedInteriorPort {
    pub fn new(port: PortData<SimulatedInteriorPort, SimulatedBorderPort>, duplex_port_link_channel: DuplexPortLinkChannel) -> Result<SimulatedInteriorPort, Error> {
        let port_id = port.get_id();
        Ok( SimulatedInteriorPort {
            port,
            duplex_port_link_channel,            
            failover_info: FailoverInfo::new(port_id),
        })
    }
    fn recv(&mut self) -> Result<LinkToPortPacket, Error> {
        let _f = "recv";
        let link_to_port_packet = self.duplex_port_link_channel.port_from_link.recv()?;
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
		let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_receive" };
		let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "link_to_port_packet": link_to_port_packet });
		add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
	}
        self.failover_info.clear_saved_packet();
        Ok(link_to_port_packet)
    }
    fn direct_send(&mut self, packet: &Packet) -> Result<(), Error> {
        let _f = "direct_send";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
		let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_direct_send" };
		let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "packet": packet });
		add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
	}
        self.failover_info.save_packet(&packet);
        Ok(self.duplex_port_link_channel.port_to_link.send(*packet).context(SimulatedInteriorPortError::Chain {func_name: "new",comment: S("")})?)
    }
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
	    AitState::Teck => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.port.get_id(), ait_state }.into()), // Not allowed here
	    AitState::Ait => { packet.next_ait_state()?; },
            AitState::Entl | // Only needed for simulator, should be handled by simulated_internal_port
            AitState::SnakeD |
            AitState::Normal => ()
        }
	self.direct_send(packet)
    }
    fn listen(self: &mut Self, port_to_pe: PortToPe) -> Result<(), Error> {
        let _f = "listen";
        let mut msg: LinkToPortPacket;
            loop {
                msg = self.recv().context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.port.get_id().get_name()) + " recv from link"})?;
		        {
                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                        match &msg {
                            LinkToPortPacket::Packet(packet) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_packet" };
                                let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "packet":packet.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            },
                            LinkToPortPacket::Status(status) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_status" };
                                let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "status": status, "msg": msg});
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            },
			            }
                    }
                }
		        match msg {
                    LinkToPortPacket::Status(status) => {
			            match status {
                            PortStatus::Connected => self.port.set_connected(),
                            PortStatus::Disconnected => self.port.set_disconnected()
			            };
			            {
                            if CONFIG.trace_options.all || CONFIG.trace_options.port {
				                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
				                let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "status": status });
				                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
			            }
			            port_to_pe.send(PortToPePacket::Status((self.port.get_port_no(), self.port.is_border(), status))).context(SimulatedInteriorPortError::Chain { func_name: _f, comment: S(self.port.get_id().get_name()) + " send status to pe"})?;
                    }
                    LinkToPortPacket::Packet(mut packet) => {
			            let ait_state = packet.get_ait_state();
			            match ait_state {
                            AitState::AitD |
                            AitState::Ait => return Err(SimulatedInteriorPortError::Ait { func_name: _f, port_id: self.port.get_id(), ait_state }.into()),

                            AitState::Tick => (), // TODO: Send AitD to packet engine
                            AitState::Entl |
                            AitState::SnakeD |
                            AitState::Normal => {
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                port_to_pe.send(PortToPePacket::Packet((self.port.get_port_no(), packet)))?;
                            },
                            AitState::Teck |
                            AitState::Tack => {
                                packet.next_ait_state()?;
                                {
                                    if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                        let ait_state = packet.get_ait_state();
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                        let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
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
                                        let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                self.direct_send(&packet)?;
                                packet.make_ait_send();
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                port_to_pe.send(PortToPePacket::Packet((self.port.get_port_no(), packet)))?;
                            },
			            }
                    }
		        }
            }
    }
}

// Link to Port
pub type PACKET = Packet;
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacket {
    Status(PortStatus),
    Packet(PACKET),
}
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
//pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;

// Port to Link
pub type PortToLinkPacket = Packet; // SimulatedPacket
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
//pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;

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
    #[fail(display = "SimulatedInteriorPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "SimulatedInteriorPortError::Ait {} {} is not allowed here on port {}", func_name, port_id, ait_state)]
    Ait { func_name: &'static str, port_id: PortID, ait_state: AitState },
}
