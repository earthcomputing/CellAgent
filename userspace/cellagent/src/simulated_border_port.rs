use crossbeam::crossbeam_channel as mpsc;
use either::Either;

use std::{
    collections::{HashMap, },
    marker::{PhantomData},
};

use crate::blueprint::{Blueprint};
use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::app_message_formats::{PortToCaMsg, PortToCa, NocToPortMsg, PortToNocMsg};
use crate::name::{Name, PortID, CellID};
use crate::port::{CommonPortLike, BorderPortLike, PortSeed, BasePort, BorderPortFactoryLike, PortStatus, DuplexPortPeOrCaChannel, DuplexPortCaChannel};
use crate::utility::{CellNo, PortNo, PortNumber, ByteArray, S, TraceHeaderParams, TraceType};
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
    base_port: BasePort,
    is_connected: bool,
    duplex_port_noc_channel: Option<DuplexPortNocChannel>,
}

#[derive(Clone, Debug)]
pub struct SimulatedBorderPortFactory {
    port_seed: PortSeed,
    cell_no_map: HashMap<String, CellNo>,
    blueprint: Blueprint,
    duplex_port_noc_channel_cell_port_map: HashMap<CellNo, HashMap<PortNo, DuplexPortNocChannel>>,
}

impl SimulatedBorderPortFactory {
    pub fn new(port_seed: PortSeed, cell_no_map: HashMap<String, CellNo>, blueprint: Blueprint, duplex_port_noc_channel_cell_port_map: HashMap<CellNo, HashMap::<PortNo, DuplexPortNocChannel>>, phantom: PhantomData<SimulatedBorderPort>) -> SimulatedBorderPortFactory {
        SimulatedBorderPortFactory { port_seed, cell_no_map, blueprint, duplex_port_noc_channel_cell_port_map }
    }
}

impl BorderPortFactoryLike<SimulatedBorderPort> for SimulatedBorderPortFactory {
    fn new_port(&self, cell_id: CellID, id: PortID, port_number: PortNumber, duplex_port_ca_channel: DuplexPortCaChannel) -> Result<SimulatedBorderPort, Error> {
        let cell_no = self.cell_no_map[&cell_id.get_name()];
        let port_no = port_number.get_port_no();
        let ref duplex_port_noc_channel_port_map = (*self).duplex_port_noc_channel_cell_port_map[&cell_no];
        Ok(SimulatedBorderPort{
            base_port: BasePort::new(
                cell_id,
                port_number,
                true,
                DuplexPortPeOrCaChannel::Border(duplex_port_ca_channel),
            )?,
            is_connected: true,
            duplex_port_noc_channel: if duplex_port_noc_channel_port_map.contains_key(&port_no) {
                Some(duplex_port_noc_channel_port_map[&port_no].clone())
            } else {
                None
            },
        })
    }
    fn get_port_seed(&self) -> &PortSeed {
        return &(*self).port_seed;
    }
    fn get_port_seed_mut(&mut self) -> &mut PortSeed {
        return &mut (*self).port_seed;
    }
}


impl SimulatedBorderPort {
    fn recv(&self) -> Result<NocToPortMsg, Error> {
       Ok(self.duplex_port_noc_channel.as_ref().unwrap().port_from_noc.recv()?)
    }
    fn direct_send(&self, bytes: &ByteArray) -> Result<(), Error> {
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
}

impl CommonPortLike for SimulatedBorderPort {
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
impl BorderPortLike for SimulatedBorderPort {
    fn send(&self, bytes: &mut ByteArray) -> Result<(), Error> {
        let _f = "send";
	self.direct_send(bytes)
    }
    fn listen(&mut self, port_to_ca: PortToCa) -> Result<(), Error> {
        let _f = "listen";
        loop {
            let msg = self.duplex_port_noc_channel.as_ref().unwrap().port_from_noc.recv()?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_noc_app" };
                    let trace = json!({ "cell_id": self.base_port.get_cell_id(),"id": self.base_port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_app" };
                    let trace = json!({ "cell_id": self.base_port.get_cell_id(), "id": self.base_port.get_id().get_name(), "msg": msg });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            port_to_ca.send(PortToCaMsg::AppMsg(self.base_port.get_port_no(), msg)).context(SimulatedBorderPortError::Chain { func_name: "listen_noc_for_pe", comment: S(self.base_port.get_id().get_name()) + " send app msg to pe"})?;
        }
    }
}

// Noc to Port
//pub type NocPortError = mpsc::SendError<NocToPortMsg>;

// Port to Noc
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
