use crossbeam::crossbeam_channel as mpsc;

use std::{
    collections::{HashMap, },
    fmt,
};

use crate::blueprint::{Blueprint};
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::app_message_formats::{PortToCaMsg, PortToCa, NocToPortMsg, PortToNocMsg};
use crate::name::{Name, PortID, CellID};
use crate::port::{CommonPortLike, BorderPortLike, PortSeed, BasePort, BorderPortFactoryLike, DuplexPortPeOrCaChannel, DuplexPortCaChannel};
use crate::utility::{CellNo, PortNo, PortNumber, ByteArray, S, TraceHeaderParams, TraceType};
use crate::uuid_ec::{AitState};

pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;

#[derive(Clone, Debug)]
pub struct DuplexPortNocChannel {
    port_from_noc: PortFromNoc,
    port_to_noc: PortToNoc,
}
impl DuplexPortNocChannel {
    pub fn new(port_from_noc: PortFromNoc, port_to_noc: PortToNoc) -> DuplexPortNocChannel {
        DuplexPortNocChannel { port_from_noc, port_to_noc }
    }
}
impl fmt::Display for DuplexPortNocChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Channels between Port and NOC")
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedBorderPort {
    base_port: BasePort,
    is_connected: bool,
    duplex_port_noc_channel: Option<DuplexPortNocChannel>,
}

impl SimulatedBorderPort {
    fn recv(&self) -> Result<NocToPortMsg, Error> {
        let _f = "recv";
        Ok(self.duplex_port_noc_channel.as_ref().unwrap().port_from_noc.recv()?)
    }
}
impl fmt::Display for SimulatedBorderPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let not_connected = if self.is_connected { "" } else {" not"};
        let channel_not_set = match self.duplex_port_noc_channel {
            Some(_) => "",
            None => " not"
        };
        write!(f, "{} is{} connected and the channel is{} defined", self.base_port, not_connected, channel_not_set)
    }
}
impl CommonPortLike for SimulatedBorderPort {
    fn get_base_port(&self) -> &BasePort {
        return &self.base_port;
    }
    fn get_whether_connected(&self) -> bool { return self.is_connected; }
    fn set_connected(&mut self) -> () { self.is_connected = true; }
    fn set_disconnected(&mut self) -> () { self.is_connected = false; }
}

impl BorderPortLike for SimulatedBorderPort {
    fn send(&self, bytes: &mut ByteArray) -> Result<(), Error> {
        let _f = "send_to_noc";
        {
            if CONFIG.trace_options.all | CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_noc" };
                let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "bytes": bytes.stringify()? });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
       Ok(self.duplex_port_noc_channel.as_ref().unwrap().port_to_noc.send(bytes.clone()).context(SimulatedBorderPortError::Chain {func_name: "new",comment: S("")})?)
    }
    fn listen_and_forward_to(&mut self, port_to_ca: PortToCa) -> Result<(), Error> {
        let _f = "listen_and_forward_to";
        loop {
            let msg = self.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_noc_app" };
                    let trace = json!({ "cell_id": self.base_port.get_cell_id(),"id": self.base_port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_ca_app" };
                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            port_to_ca.send(PortToCaMsg::AppMsg(self.base_port.get_port_no(), msg)).context(SimulatedBorderPortError::Chain { func_name: "listen_noc_for_pe", comment: S(self.base_port.get_id().get_name()) + " send app msg to pe"})?;
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedBorderPortFactory {
    port_seed: PortSeed,
    cell_no_map: HashMap<String, CellNo>,
    blueprint: Blueprint,
    duplex_port_noc_channel_cell_port_map: HashMap<CellNo, HashMap<PortNo, DuplexPortNocChannel>>,
}

impl SimulatedBorderPortFactory {
    pub fn new(port_seed: PortSeed, cell_no_map: HashMap<String, CellNo>, blueprint: Blueprint, 
            duplex_port_noc_channel_cell_port_map: HashMap<CellNo, HashMap::<PortNo, DuplexPortNocChannel>>) 
                -> SimulatedBorderPortFactory {
        SimulatedBorderPortFactory { port_seed, cell_no_map, blueprint, duplex_port_noc_channel_cell_port_map }
    }
}

impl BorderPortFactoryLike<SimulatedBorderPort> for SimulatedBorderPortFactory {
    fn new_port(&self, cell_id: CellID, _port_id: PortID, port_number: PortNumber, 
            duplex_port_ca_channel: DuplexPortCaChannel) -> Result<SimulatedBorderPort, Error> {
        let cell_no = self.cell_no_map[&cell_id.get_name()];
        let port_no = port_number.get_port_no();
        let duplex_port_noc_channel_port_map = &(*self).duplex_port_noc_channel_cell_port_map[&cell_no];
        Ok(SimulatedBorderPort{
            base_port: BasePort::new(
                cell_id,
                port_number,
                true,
                DuplexPortPeOrCaChannel::Border(duplex_port_ca_channel),
            )?,
            is_connected: true,
            duplex_port_noc_channel: duplex_port_noc_channel_port_map.get(&port_no).cloned(),
        })
    }
    fn get_port_seed(&self) -> &PortSeed {
        return &(*self).port_seed;
    }
    fn get_port_seed_mut(&mut self) -> &mut PortSeed {
        return &mut (*self).port_seed;
    }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum SimulatedBorderPortError {
    #[fail(display = "SimulatedBorderPortError::Ait {} {} is not allowed here", func_name, ait_state)]
    Ait { func_name: &'static str, ait_state: AitState },
    #[fail(display = "SimulatedBorderPortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
