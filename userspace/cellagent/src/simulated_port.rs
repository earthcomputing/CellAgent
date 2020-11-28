use std::sync::mpsc;
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{LinkToPortPacket, PortToLinkPacket,
                                PortToPePacket, PortToPe};
use crate::name::{Name};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{Port, PortStatus};
use crate::utility::{S, TraceHeader, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;

pub struct SimulatedPort {
  port_to_link: PortToLink,
  port_from_link: PortFromLink,
}

impl SimulatedPort {
    pub fn new(port_to_link: PortToLink, port_from_link: PortFromLink) -> SimulatedPort {
      	SimulatedPort{ port_to_link, port_from_link}
    }
    pub fn listen(&self, port: &mut Port, port_to_pe: PortToPe) -> Result<(), Error> {
        let _f = "listen";
        let mut msg: LinkToPortPacket;
            loop {
                msg = self.recv().context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " recv from link"})?;
		        {
                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                        match &msg {
                            LinkToPortPacket::Packet(packet) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_packet" };
                                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.to_string()? });
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            },
                            LinkToPortPacket::Status(status) => {
                                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_link_status" };
                                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "status": status, "msg": msg});
                                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
				                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            }
			            }
			            port_to_pe.send(PortToPePacket::Status((port.get_port_no(), port.is_border(), status))).context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " send status to pe"})?;
                    }
                    LinkToPortPacket::Packet(mut packet) => {
			            let ait_state = packet.get_ait_state();
			            match ait_state {
                            AitState::AitD |
                            AitState::Ait => return Err(SimulatedPortError::Ait { func_name: _f, ait_state }.into()),

                            AitState::Tick => (), // TODO: Send AitD to packet engine
                            AitState::Entl |
                            AitState::Normal => {
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.to_string()? });
                                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.to_string()? });
                                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
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
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.to_string()? });
                                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                self.direct_send(packet.clone())?;
                                packet.make_ait_send();
                                {
                                    if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_packet" };
                                        let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet":packet.to_string()? });
                                        let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                    }
                                }
                                port_to_pe.send(PortToPePacket::Packet((port.get_port_no(), packet)))?;
                            },
			            }
                    }
		        }
            }
    }
    pub fn send(&self, mut packet: Packet) -> Result<(), Error> {
        let _f = "send";
	        let ait_state = packet.get_ait_state();

            {
		match ait_state {
	            AitState::AitD |
                    AitState::Tick |
		    AitState::Tock |
		    AitState::Tack |
		    AitState::Teck => return Err(SimulatedPortError::Ait { func_name: _f, ait_state }.into()), // Not allowed here
                
		    AitState::Ait => {
                        packet.next_ait_state()?;
            	    },
                    AitState::Entl | // Only needed for simulator, should be handled by port
                    AitState::Normal => ()
                }
		self.direct_send(packet)
            }    }
    fn recv(&self) -> Result<LinkToPortPacket, Error> {
       Ok(self.port_from_link.recv()?)
    }
    fn direct_send(&self, packet: Packet) -> Result<(), Error> {
       Ok(self.port_to_link.send(packet).context(SimulatedPortError::Chain {func_name: "new",comment: S("")})?)
    }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum SimulatedPortError {
    #[fail(display = "SimulatedPortError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
    #[fail(display = "SimulatedPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
