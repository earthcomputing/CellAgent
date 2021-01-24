use crossbeam::crossbeam_channel as mpsc;

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::app_message_formats::{PortToCaMsg, PortToCa, APP};
use crate::name::{Name};
use crate::port::{BorderPortLike, PortData, PortStatus};
use crate::simulated_internal_port::{SimulatedInteriorPort};
use crate::utility::{ByteArray, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;

#[derive(Clone, Debug)]
pub struct DuplexPortNocChannel {
    pub port_from_noc: PortFromNoc,
    pub port_to_noc: PortToNoc,
}

#[derive(Clone, Debug)]
pub struct SimulatedBorderPort {
    port: PortData<SimulatedInteriorPort, SimulatedBorderPort>,
    duplex_port_noc_channel: DuplexPortNocChannel,
}

impl SimulatedBorderPort {
    pub fn new(port: PortData<SimulatedInteriorPort, SimulatedBorderPort>, duplex_port_noc_channel: DuplexPortNocChannel) -> SimulatedBorderPort {
        SimulatedBorderPort{ port, duplex_port_noc_channel}
    }
    fn recv(&self) -> Result<NocToPortMsg, Error> {
       Ok(self.duplex_port_noc_channel.port_from_noc.recv()?)
    }
    fn direct_send(&self, bytes: &ByteArray) -> Result<(), Error> {
        let _f = "send_to_noc";
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_noc" };
                let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "bytes": bytes.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
       Ok(self.duplex_port_noc_channel.port_to_noc.send(bytes.clone()).context(SimulatedBorderPortError::Chain {func_name: "new",comment: S("")})?)
    }
}

impl BorderPortLike for SimulatedBorderPort {
    fn send(&self, bytes: &mut ByteArray) -> Result<(), Error> {
        let _f = "send";
	self.direct_send(bytes)
    }
    fn listen(&mut self, port_to_ca: PortToCa) -> Result<(), Error> {
        let _f = "listen";
        loop {
            let msg = self.duplex_port_noc_channel.port_from_noc.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_noc_app" };
                    let trace = json!({ "cell_id": self.port.get_cell_id(),"id": self.port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_app" };
                    let trace = json!({ "cell_id": self.port.get_cell_id(), "id": self.port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            port_to_ca.send(PortToCaMsg::AppMsg(self.port.get_port_no(), msg)).context(SimulatedBorderPortError::Chain { func_name: "listen_noc_for_pe", comment: S(self.port.get_id().get_name()) + " send app msg to pe"})?;
        }
    }
}

// Noc to Port
pub type NocToPortMsg = APP;
pub type NocToPort = mpsc::Sender<NocToPortMsg>;
//pub type NocPortError = mpsc::SendError<NocToPortMsg>;

// Port to Noc
pub type PortToNocMsg = APP;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;
//pub type PortNocError = mpsc::SendError<PortToNocPacket>;

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum SimulatedBorderPortError {
    #[fail(display = "SimulatedBorderPortError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
    #[fail(display = "SimulatedBorderPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
