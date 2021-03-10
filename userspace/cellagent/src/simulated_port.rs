use std::fmt;

use crossbeam::crossbeam_channel as mpsc;

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{LinkToPortPacket, PortToLinkPacket,
                                PortToPePacket, PortToPe};
use crate::name::{Name, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{Port, PortStatus};
use crate::utility::{S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;

#[derive(Clone)]
pub struct SimulatedPort {
    port_id: PortID,
    failover_info: FailoverInfo,
    port_to_link: PortToLink,
    port_from_link: PortFromLink,
}

impl SimulatedPort {
    pub fn new(port_id: PortID, port_to_link: PortToLink, port_from_link: PortFromLink) -> SimulatedPort {
      	SimulatedPort{ port_id, port_to_link, port_from_link, failover_info: FailoverInfo::new(port_id) }
    }
    pub fn listen(&mut self, port: &mut Port, port_to_pe: PortToPe) -> Result<(), Error> {
        let _f = "listen";
        let mut msg: LinkToPortPacket;
            loop {
                msg = self.recv().context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " recv from link"})?;
		        {
                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                        match &msg {
                            LinkToPortPacket::Packet(packet) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_packet" };
                                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.stringify()? });
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            },
                            LinkToPortPacket::Status(status) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_status" };
                                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "status": status, "msg": msg});
                                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            },
			            }
                    }
                }
		        match msg {
                    LinkToPortPacket::Status(status) => {
			            match status {
                            PortStatus::Connected => port.set_connected(),
                            PortStatus::Disconnected => port.set_disconnected()
			            };
			            {
                            if CONFIG.trace_options.all || CONFIG.trace_options.port {
				                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
				                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "status": status });
				                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
			            }
			            port_to_pe.send(PortToPePacket::Status((port.get_port_no(), port.is_border(), status))).context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " send status to pe"})?;
                    }
                    LinkToPortPacket::Packet(mut packet) => {
			            let ait_state = packet.get_ait_state();
			            match ait_state {
                            AitState::AitD |
                            AitState::Ait => return Err(SimulatedPortError::Ait { func_name: _f, port_id: self.port_id, ait_state }.into()),

                            AitState::Tick => (), // TODO: Send AitD to packet engine
                            AitState::Entl |
                            AitState::SnakeD |
                            AitState::Normal => {
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                port_to_pe.send(PortToPePacket::Packet((port.get_port_no(), packet)))?;
                            },
                            AitState::Teck |
                            AitState::Tack => {
                                packet.next_ait_state()?;
                                {
                                    if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                        let ait_state = packet.get_ait_state();
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                self.direct_send(packet.clone())?;
                            }
                            AitState::Tock => {
                                packet.next_ait_state()?;
                                {
                                    if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                        let ait_state = packet.get_ait_state();
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                self.direct_send(packet.clone())?;
                                packet.make_ait_send();
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.stringify()? });
                                        add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                port_to_pe.send(PortToPePacket::Packet((port.get_port_no(), packet)))?;
                            },
			            }
                    }
		        }
            }
    }
    pub fn send(&mut self, mut packet: Packet) -> Result<(), Error> {
        let _f = "send";
	    let ait_state = packet.get_ait_state();
		match ait_state {
	        AitState::AitD |
            AitState::Tick |
		    AitState::Tock |
		    AitState::Tack |
		    AitState::Teck => return Err(SimulatedPortError::Ait { func_name: _f, port_id: self.port_id, ait_state }.into()), // Not allowed here 
		    AitState::Ait => { packet.next_ait_state()?; },
            AitState::Entl | // Only needed for simulator, should be handled by port
            AitState::SnakeD |
            AitState::Normal => ()
        }
		self.direct_send(packet)
    }
    fn recv(&mut self) -> Result<LinkToPortPacket, Error> {
        let packet = self.port_from_link.recv()?;
        self.failover_info.clear_saved_packet();
        Ok(packet)
    }
    fn direct_send(&mut self, packet: Packet) -> Result<(), Error> {
        self.failover_info.save_packet(&packet);
        Ok(self.port_to_link.send(packet).context(SimulatedPortError::Chain {func_name: "new",comment: S("")})?)
    }
}

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
pub enum SimulatedPortError {
    #[fail(display = "SimulatedPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "SimulatedPortError::Ait {} {} is not allowed here on port {}", func_name, port_id, ait_state)]
    Ait { func_name: &'static str, port_id: PortID, ait_state: AitState },
}
