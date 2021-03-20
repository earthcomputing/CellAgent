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
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet, UniqueMsgId};
use crate::utility::{ByteArray, PortNo, PortNumber, S, TraceHeader, TraceHeaderParams, TraceType,
                     write_err};

#[derive(Clone, Debug)]
pub struct DuplexPortPeChannel {
    pub port_from_pe: PortFromPe,
    pub port_to_pe: PortToPe,
}

#[derive(Clone, Debug)]
pub struct DuplexPortCaChannel {
    pub port_from_ca: PortFromCa,
    pub port_to_ca: PortToCa,
}

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

pub trait CommonPortLike: 'static {
    fn get_base_port(&self) -> &BasePort;
    fn get_base_port_mut(&mut self) -> &mut BasePort;
    fn get_id(&self) -> PortID { return self.get_base_port().get_id(); }
    fn get_cell_id(&self) -> CellID { return self.get_base_port().get_cell_id(); }
    fn get_port_no(&self) -> PortNo { return self.get_base_port().get_port_no(); }
//  fn get_port_number(&self) -> PortNumber;
    fn get_whether_connected(&self) -> bool;
    fn set_connected(&mut self);
    fn set_disconnected(&mut self);
}

pub trait InteriorPortLike: 'static + Clone + Sync + Send + CommonPortLike {
    fn is_border(&self) -> bool { return false; } // SHOULDN'T BE NEEDED
    fn send(self: &mut Self, packet: &mut Packet) -> Result<(), Error>;
    fn listen_and_forward_to(self: &mut Self, port_to_pe: PortToPe) -> Result<(), Error>;
    fn listen_link_and_pe(&mut self) {
        let _f = "listen_link_and_pe_loops";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let port_clone = self.clone();
        let thread_name = format!("Port {} listen_link", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_link_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { port.listen_link_loop().map_err(|e| write_err("port", &e)).ok();  }
        }).expect("thread failed");
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_pe", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { port.listen_pe_loop().map_err(|e| write_err("port", &e)).ok(); }
        }).expect("thread failed");
    }

    // THESE COULD BE PRIVATE
    fn listen(self: &mut Self) -> Result<(), Error> {
        self.listen_and_forward_to(self.get_duplex_port_pe_channel().port_to_pe)
    }
    fn get_duplex_port_pe_channel(&self) -> DuplexPortPeChannel {
        return if let DuplexPortPeOrCaChannel::Interior(duplex_port_pe_channel) = self.get_base_port().get_duplex_port_pe_or_ca_channel() {duplex_port_pe_channel} else {panic!("Expected an interior port")};
    }
    fn listen_link_loop(&mut self) -> Result<(), Error> {
        let _f = "listen_link_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        #[cfg(any(feature = "cell", feature = "simulator"))] {
            return self.listen();
        }
        #[cfg(feature = "noc")]
        return Ok(()) // For now, needs to be fleshed out!
    }
    // WORKER (PortFromPe)
    fn listen_pe_loop(&mut self) -> Result<(), Error> {
        let _f = "listen_pe_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            //println!("Port {}: waiting for packet from pe", id);
            let mut packet = self.get_duplex_port_pe_channel().port_from_pe.recv().context(PortError::Chain { func_name: _f, comment: S(self.get_id().get_name()) + " port_from_pe"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_pe" };
                    let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "packet":packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            {
                if CONFIG.trace_options.all | CONFIG.trace_options.port {
                    let ait_state = packet.get_ait_state();
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_link" };
                    let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "ait_state": ait_state, "packet": packet.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.send(&mut packet)?;
        }
    }
}

pub trait BorderPortLike: 'static + Clone + Sync + Send + CommonPortLike {
    fn is_border(&self) -> bool { return true; } // SHOULDN'T BE NEEDED
    fn send(self: &Self, bytes: &mut ByteArray) -> Result<(), Error>;
    fn listen_and_forward_to(self: &mut Self, port_to_ca: PortToCa) -> Result<(), Error>;
    fn listen_noc_and_ca(&self) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_noc_and_ca";
        let status = PortToCaMsg::Status(self.get_port_no(), PortStatus::Connected);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "status": PortStatus::Connected });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_to_ca = self.get_duplex_port_ca_channel().port_to_ca;
        port_to_ca.send(status).context(PortError::Chain { func_name: "noc_channel", comment: S(self.get_id().get_name()) + " send to pe"})?;
        self.clone().listen_noc()?;
        let join_handle = self.listen_ca()?;
        Ok(join_handle)
    }

    // THESE COULD BE PRIVATE
    fn listen(self: &mut Self) -> Result<(), Error> {
        self.listen_and_forward_to(self.get_duplex_port_ca_channel().port_to_ca)
    }
    fn get_duplex_port_ca_channel(&self) -> DuplexPortCaChannel {
        return if let DuplexPortPeOrCaChannel::Border(duplex_port_ca_channel) = self.get_base_port().get_duplex_port_pe_or_ca_channel() {duplex_port_ca_channel} else {panic!("Expected a border port")};
    }
    // SPAWN THREAD (listen_noc_for_pe_loop)
    fn listen_noc(&self) -> Result<(), Error> {
        let _f = "listen_noc";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_noc_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.clone().listen_noc(); }
        })?;
        Ok(())
    }

    // WORKER (PortFromNoc)
    fn listen_noc_loop(&mut self) -> Result<(), Error> {
        let _f = "listen_noc_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        return self.listen();
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_ca";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_ca_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.listen_ca(); }
        })?;
        Ok(join_handle)
    }

    // WORKER (PortFromPe)
    fn listen_ca_loop(&self) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let mut bytes = self.get_duplex_port_ca_channel().port_from_ca.recv().context(PortError::Chain { func_name: _f, comment: S(self.get_id().get_name()) + " recv from ca"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_ca" };
                    let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "bytes": bytes.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            (*self).send(&mut bytes)?;
        }
    }
}

#[derive(Debug, Clone)]
pub struct BasePort {
    cell_id: CellID, // Used in trace records
    id: PortID,
    port_number: PortNumber,
    is_border: bool,
    duplex_port_pe_or_ca_channel: DuplexPortPeOrCaChannel,
}
impl BasePort {
    pub fn new(cell_id: CellID, port_number: PortNumber, is_border: bool,
               duplex_port_pe_or_ca_channel: DuplexPortPeOrCaChannel) -> Result<BasePort, Error> {
        let port_id = PortID::new(cell_id, port_number).context(PortError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
        Ok(BasePort { cell_id, id: port_id, port_number, is_border,
                      duplex_port_pe_or_ca_channel,
        })
    }
    pub fn get_id(&self) -> PortID { self.id }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//  pub fn get_port_number(&self) -> PortNumber { self.port_number }
    pub fn is_border(&self) -> bool { self.is_border }
    pub fn get_duplex_port_pe_or_ca_channel(&self) -> DuplexPortPeOrCaChannel { return self.duplex_port_pe_or_ca_channel.clone(); }
}
impl fmt::Display for BasePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
// HOW TO GET CONNECTION STATUS PRINTED AGAIN??        
//        let is_connected = self.get_whether_connected();  HOW TO GET THIS PRINTED AGAIN??
        let mut s = format!("BasePort {} {}", self.port_number, self.id);
        if self.is_border { s = s + " is boundary  port,"; }
        else              { s = s + " is ECLP port,"; }
//        if is_connected   { s = s + " is connected"; }
//        else              { s = s + " is not connected"; }
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub enum Port<InteriorPortType: 'static + Clone + InteriorPortLike,
              BorderPortType: 'static + Clone + BorderPortLike,
         > {
    Border(Box<BorderPortType>),
    Interior(Box<InteriorPortType>),
}

#[derive(Debug, Clone)]
pub enum DuplexPortPeOrCaChannel {
    Interior(DuplexPortPeChannel),
    Border(DuplexPortCaChannel),
}

pub trait InteriorPortFactoryLike<InteriorPortType: InteriorPortLike>: Clone {
    fn new_port(&self, cell_id: CellID, id: PortID, port_number: PortNumber, duplex_port_pe_channel: DuplexPortPeChannel) -> Result<InteriorPortType, Error>;
    fn get_port_seed(&self) -> &PortSeed;
    fn get_port_seed_mut(&mut self) -> &mut PortSeed;
}

pub trait BorderPortFactoryLike<BorderPortType: BorderPortLike>: Clone {
    fn new_port(&self, cell_id: CellID, id: PortID, port_number: PortNumber, duplex_port_ca_channel: DuplexPortCaChannel) -> Result<BorderPortType, Error>;
    fn get_port_seed(&self) -> &PortSeed;
    fn get_port_seed_mut(&mut self) -> &mut PortSeed;
}

#[derive(Debug, Clone)]
pub struct PortSeed {
    // This could also contain is_border, but we get that from other information available to NalCell::new
}
impl PortSeed {
    pub fn new() -> PortSeed {
        PortSeed {
        }
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
