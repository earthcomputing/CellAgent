use crossbeam::{TryRecvError};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{LinkToPortPacket, PortToLink, PortFromLink,
                                PortToPePacket, PortToPe, 
                                PortFromPe, PortFromPeSync};
use crate::name::{Name, PortID};
use crate::packet::{Packet}; // Eventually use SimulatedPacket
use crate::port::{Port, PortStatus};
use crate::utility::{PortNo, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};


#[derive(Clone)]
pub struct SimulatedPort {
    port: Port,
    port_to_link: PortToLink,
    port_from_link: PortFromLink,
    port_from_pe: PortFromPe
}

impl SimulatedPort {
    pub fn new(port: &Port, port_to_link: PortToLink, port_from_link: PortFromLink, port_from_pe: PortFromPe) -> SimulatedPort {
      	SimulatedPort{ port: port.clone(), port_to_link, port_from_link, port_from_pe}
    }
    pub fn listen_link(&mut self, port: &mut Port, port_to_pe: &PortToPe,
            port_from_pe_sync: &PortFromPeSync) -> Result<(), Error> {
        let _f = "listen_link";
        let port_no = port.get_port_no();
        loop {
            let msg = self.port_from_link.recv().context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " recv from link"})?;
            match msg {
                LinkToPortPacket::Status(status) => {
                    match status {
                        PortStatus::Connected => {
                            // Initialize connection
                            let my_uuid = port.get_id().get_uuid();
                            let my_packet = Packet::new_entl_packet(my_uuid);
                            self.port_to_link.send(my_packet)?;
                            port.set_connected();
                        },
                        PortStatus::Disconnected => {
                            // TODO: Figure out what to do here
                            port.set_disconnected();
                        }
                    };
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_to_pe_status" };
                            let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "status": status });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    port_to_pe.send(PortToPePacket::Status((port_no, port.is_border(), status))).context(SimulatedPortError::Chain { func_name: _f, comment: S(port.get_id().get_name()) + " send status to pe"})?;
                }
                LinkToPortPacket::Packet(mut packet) => {
                    let ait_state = packet.get_ait_state();
                    match ait_state {
                        AitState::Entl => { // Initialize connection
                            let my_uuid = port.get_id().get_uuid();
                            let other_uuid = packet.get_uuid();
                            if my_uuid < other_uuid {
                                let mut tick = Packet::new_entl_packet(Default::default());
                                tick.make_packet_tick();
                                self.port_to_link.send(tick)?;
                            } else if my_uuid == other_uuid {
                                return Err(SimulatedPortError::Uuid { func_name: _f, port_no }.into());
                            }
                        },
                        AitState::AitD => return Err(SimulatedPortError::Ait { func_name: _f, ait_state }.into()),
                        AitState::SnakeD |
                        AitState::Normal => {
                            {
                                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_to_pe_normal" };
                                    let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "packet": packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.send_to_pe_and_wait(port, port_to_pe, &port_from_pe_sync, &mut packet)?
                        },
                        AitState::Ait |
                        AitState::Teck |
                        AitState::Tack => {
                            packet.next_ait_state()?;
                            {
                                if CONFIG.trace_options.all | CONFIG.trace_options.port {
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_to_link_tack" };
                                    let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            self.send_to_link(port.get_id(), packet.clone())?;
                        }
                        AitState::Tock => {
                            let new_packet = Packet::new_ping_packet();
                            self.send_to_link(port.get_id(), new_packet)?;
                             {
                                if  (CONFIG.trace_options.all || CONFIG.trace_options.port) & !packet.is_entl() { // Don't trace entl packets
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_to_pe_packet" };
                                    let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            if !packet.is_entl() {
                                packet.make_packet_ait_send();
                                self.send_to_pe_and_wait(port, port_to_pe, &port_from_pe_sync, &mut packet)?;
                            }
                        },
                        AitState::Tick => { // TODO: Send AitD to packet engine
                            {
                                if false & (CONFIG.trace_options.all | CONFIG.trace_options.port) { // Don't trace entl packets
                                    let ait_state = packet.get_ait_state();
                                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_from_link_tick" };
                                    let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                }
                            }
                            match self.port_from_pe.try_recv() {
                                Ok(p) => { 
                                    {
                                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                                            let ait_state = packet.get_ait_state();
                                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_from_pe_packet" };
                                            let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                        }
                                    }
                                    self.port_to_link.send(p)?;
                                },
                                Err(e) => match e {
                                    TryRecvError::Empty => {
                                        packet.next_ait_state()?;
                                        self.port_to_link.send(packet)?; // No data to send so send back ENTL packet           
                                    },
                                    TryRecvError::Disconnected => return Err(SimulatedPortError::Disconnected { func_name: _f, port_no: port.get_port_no() }.into())
                                }
                            }
                        }, 
                   }
                }
            }
        }
    }
    // Can't send the next event until the packet has been moved to the out port queue
    fn send_to_pe_and_wait(&self, port: &Port, port_to_pe: &PortToPe, port_from_pe_sync: &PortFromPeSync, packet: &mut Packet) -> Result<(), Error> {
        let _f = "send_and_wait";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let ait_state = packet.get_ait_state();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_send_to_pe_and_wait" };
                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet": packet.clone().stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        port_to_pe.send(PortToPePacket::Packet((port.get_port_no(), packet.clone())))?;
        let new_packet = if packet.is_entl() {
            packet.next_ait_state()?; 
            packet.clone()
        } else {
            Packet::new_ping_packet() // Otherwise send TICK
        };
        let packet_to_send = port_from_pe_sync.recv()?
            .or(Some(new_packet))
            .unwrap();
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let ait_state = packet_to_send.get_ait_state();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_sent_to_pe_and_waited" };
                let trace = json!({ "cell_id": port.get_cell_id(), "id": port.get_id().get_name(), "ait_state": ait_state, "packet": packet_to_send.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.send_to_link(port.get_id(), packet_to_send)?;
        Ok(())
    }
    pub fn send(&self, port_id: PortID, mut packet: Packet) -> Result<(), Error> {
        let _f = "send";
	    let ait_state = packet.get_ait_state();
		match ait_state {
	        AitState::AitD |
            AitState::Tick |
		    AitState::Tock |
		    AitState::Tack |
		    AitState::Teck => return Err(SimulatedPortError::Ait { func_name: _f, ait_state }.into()), // Not allowed here 
		    AitState::Ait => { packet.next_ait_state()?; },
            AitState::Entl |
            AitState::SnakeD |
            AitState::Normal => ()
        }
		self.send_to_link(port_id, packet)
    }
    fn send_to_link(&self, port_id: PortID, packet: Packet) -> Result<(), Error> {
        let _f = "send_to_link";
        {
            if !packet.is_entl() & (CONFIG.trace_options.all || CONFIG.trace_options.port) {
                let ait_state = packet.get_ait_state();
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "simulated_port_send_to_link" };
                let trace = json!({ "cell_id": self.port.get_cell_id(), "ait_state": ait_state, "port_id": port_id, "packet": packet.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
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
    #[fail(display = "SimulatedPortError::Disconnected {} {}", func_name, port_no)]
    Disconnected { func_name: &'static str, port_no: PortNo },
    #[fail(display = "SimulatedPortError::Uuid {} {}", func_name, port_no)]
    Uuid { func_name: &'static str, port_no: PortNo }
}
