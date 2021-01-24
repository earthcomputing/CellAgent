use std::{
    clone::{Clone},
    fmt,
    marker::{PhantomData},
    thread,
    thread::JoinHandle,
};

use crossbeam::crossbeam_channel as mpsc;
use either::Either;

use crate::app_message_formats::{PortToCa, PortToCaMsg, PortFromCa};
use crate::config::CONFIG;
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message_formats::{PortToPe, PortFromPe};
use crate::name::{Name, PortID, CellID};
use crate::packet::{Packet, UniqueMsgId};
use crate::simulated_border_port::SimulatedBorderPort;
use crate::utility::{ByteArray, PortNo, PortNumber, S, TraceHeader, TraceHeaderParams, TraceType,
                     write_err};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum PortStatus {
    Connected,
    Disconnected,
}
impl fmt::Display for PortStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortStatus::Connected    => write!(f, "Connected"),
            PortStatus::Disconnected => write!(f, "Disconnected")
        }
    }
}

pub trait InteriorPortLike: Sync + Send {
    fn send(self: &mut Self, packet: &mut Packet) -> Result<(), Error>;
    fn listen(self: &mut Self, port_to_pe: PortToPe) -> Result<(), Error>;
}

pub trait BorderPortLike: Sync + Send {
    fn send(self: &Self, bytes: &mut ByteArray) -> Result<(), Error>;
    fn listen(self: &mut Self, port_to_ca: PortToCa) -> Result<(), Error>;
}

// IS THIS NEEDED?
pub struct InteriorPortObject<InteriorPortType: InteriorPortLike> {
    interior_port_like: InteriorPortType,
}

#[derive(Debug, Clone)]
pub struct PortData<InteriorPortType: Clone + InteriorPortLike, BorderPortType: Clone + BorderPortLike> {
    cell_id: CellID, // Used in trace records
    id: PortID,
    port_number: PortNumber,
    is_border: bool,
    is_connected: bool,
    port_to_pe_or_ca: Either<PortToPe, PortToCa>,
    interior_phantom: PhantomData<InteriorPortType>,
    border_phantom: PhantomData<BorderPortType>,
}
impl<InteriorPortType: 'static + Clone + InteriorPortLike, BorderPortType: 'static + Clone + BorderPortLike> PortData<InteriorPortType, BorderPortType> {
    pub fn new(cell_id: CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
               port_to_pe_or_ca: Either<PortToPe, PortToCa>) -> Result<PortData<InteriorPortType, BorderPortType>, Error> {
        let port_id = PortID::new(cell_id, port_number).context(PortError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
        Ok(PortData { cell_id, id: port_id, port_number, is_border,
                      is_connected,
                      port_to_pe_or_ca,
                      interior_phantom: PhantomData::<InteriorPortType>,
                      border_phantom: PhantomData::<BorderPortType>,
        })
    }
    pub fn get_id(&self) -> PortID { self.id }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//  pub fn get_port_number(&self) -> PortNumber { self.port_number }
    pub fn is_connected(&self) -> bool { self.is_connected }
    pub fn set_connected(&mut self) { self.is_connected = true; }
    pub fn set_disconnected(&mut self) { self.is_connected = false; }
    pub fn is_border(&self) -> bool { self.is_border }
    pub fn noc_channel(&self, simulated_border_port: BorderPortType,
            port_from_ca: PortFromCa) -> Result<JoinHandle<()>, Error> {
        let _f = "noc_channel";
        let status = PortToCaMsg::Status(self.get_port_no(), PortStatus::Connected);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "status": PortStatus::Connected });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_to_ca = self.port_to_pe_or_ca.clone().right().expect("Port to Ca sender must be set");
        port_to_ca.send(status).context(PortError::Chain { func_name: "noc_channel", comment: S(self.id.get_name()) + " send to pe"})?;
        self.clone().listen_noc(simulated_border_port.clone(), port_to_ca)?;
        let join_handle = self.listen_ca(simulated_border_port, port_from_ca)?;
        Ok(join_handle)
    }

    // SPAWN THREAD (listen_noc_for_pe_loop)
    fn listen_noc(self, simulated_border_port: BorderPortType, port_to_ca: PortToCa) -> Result<(), Error> {
        let _f = "listen_noc";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        let simulated_border_port_clone = simulated_border_port.clone();
        let port_to_ca_clone = port_to_ca.clone();
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_noc_loop(simulated_border_port, port_to_ca).map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.clone().listen_noc(simulated_border_port_clone, port_to_ca_clone); }
        })?;
        Ok(())
    }

    // WORKER (PortFromNoc)
    fn listen_noc_loop(&self, simulated_border_port: BorderPortType, port_to_ca: PortToCa) -> Result<(), Error> {
        let _f = "listen_noc_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        return simulated_border_port.clone().listen(port_to_ca);
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self, simulated_border_port: BorderPortType, port_from_ca: PortFromCa) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_ca";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_ca_loop(&simulated_border_port.clone(), &port_from_ca.clone()).map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.listen_ca(simulated_border_port.clone(), port_from_ca.clone()); }
        })?;
        Ok(join_handle)
    }

    // WORKER (PortFromPe)
    fn listen_ca_loop(&self, simulated_border_port: &BorderPortType, port_from_ca: &PortFromCa) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let mut bytes = port_from_ca.recv().context(PortError::Chain { func_name: _f, comment: S(self.id.get_name()) + " recv from ca"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_ca" };
                    let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "bytes": bytes.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            simulated_border_port.send(&mut bytes)?;
        }
    }

    // SPAWN THREAD (listen_link, listen_pe)
    pub fn link_channel(&self, simulated_port_or_ecnl_port: InteriorPortType,
                        port_from_pe: PortFromPe) {
        let _f = "link_channel";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let simulated_port_or_ecnl_port_clone = simulated_port_or_ecnl_port.clone();
        let thread_name = format!("Port {} listen_link", self.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_link_loop(simulated_port_or_ecnl_port_clone.clone()).map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { port.listen_link_loop(simulated_port_or_ecnl_port_clone).map_err(|e| write_err("port", &e)).ok();  }
        }).expect("thread failed");
        let port = self.clone();
        let child_trace_header = fork_trace_header(); 
        let thread_name = format!("Port {} listen_pe", self.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe_loop(simulated_port_or_ecnl_port.clone(), &port_from_pe.clone()).map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { port.listen_pe_loop(simulated_port_or_ecnl_port, &port_from_pe).map_err(|e| write_err("port", &e)).ok(); }
        }).expect("thread failed");
    }

    // WORKER (PortFromLink)
    fn listen_link_loop(&mut self, mut simulated_port_or_ecnl_port: InteriorPortType) -> Result<(), Error> {
        let _f = "listen_link_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_to_pe = self.port_to_pe_or_ca.clone().left().expect("Port: Sender to Pe must be set");
        #[cfg(any(feature = "cell", feature = "simulator"))] {
            return simulated_port_or_ecnl_port.listen(port_to_pe);
        }
        #[cfg(feature = "noc")]
        return Ok(()) // For now, needs to be fleshed out!
    }
    // WORKER (PortFromPe)
    fn listen_pe_loop(&self, mut simulated_port_or_ecnl_port: InteriorPortType, port_from_pe: &PortFromPe) -> Result<(), Error> {
        let _f = "listen_pe_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            //println!("Port {}: waiting for packet from pe", id);
            let mut packet = port_from_pe.recv().context(PortError::Chain { func_name: _f, comment: S(self.id.get_name()) + " port_from_pe"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_pe" };
                    let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "packet":packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.port {
                    let ait_state = packet.get_ait_state();
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                    let trace = json!({ "cell_id": self.cell_id, "id": self.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            simulated_port_or_ecnl_port.send(&mut packet)?;
        }
    }
}
impl<InteriorPortType: 'static + Clone + InteriorPortLike, BorderPortType: 'static + Clone + BorderPortLike> fmt::Display for PortData<InteriorPortType, BorderPortType> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let is_connected = self.is_connected();
        let mut s = format!("Port {} {}", self.port_number, self.id);
        if self.is_border { s = s + " is boundary  port,"; }
        else              { s = s + " is ECLP port,"; }
        if is_connected   { s = s + " is connected"; }
        else              { s = s + " is not connected"; }
        write!(f, "{}", s)
    }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PortError {
    #[fail(display = "PortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "PortError::NonApp {}: Non APP message received on port {}, with is a border port", func_name, port_no)]
    NonApp { func_name: &'static str, port_no: u8 },
    #[fail(display = "PortError::App {}: APP message received on port {}, with is not a border port", func_name, port_no)]
    App { func_name: &'static str, port_no: u8 }
}
