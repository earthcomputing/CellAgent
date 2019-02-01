use std::{fmt,
          thread,
          thread::JoinHandle,
          sync::{atomic::AtomicBool, Arc, atomic::Ordering::SeqCst}};

use crate::config::{CONTINUE_ON_ERROR, TRACE_OPTIONS, PortNo};
use crate::dal;
use crate::dal::{fork_trace_header, update_trace_header};
use crate::tcp_message_types::{PortToNoc, PortFromNoc};
use crate::ec_message_types::{PortToLink, PortFromLink, PortToPe, PortFromPe, LinkToPortPacket, PortToPePacket,
              PeToPortPacket};
use crate::name::{Name, PortID, CellID};
use crate::utility::{PortNumber, S, write_err, TraceHeader, TraceHeaderParams, TraceType};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Copy, Clone, Serialize)]
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

#[derive(Debug, Clone)]
pub struct Port {
    id: PortID,
    port_number: PortNumber,
    is_border: bool,
    is_connected: Arc<AtomicBool>,
    port_to_pe: PortToPe,
}
impl Port {
    pub fn new(cell_id: CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
               port_to_pe: PortToPe) -> Result<Port, Error> {
        let port_id = PortID::new(cell_id, port_number).context(PortError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
        Ok(Port{ id: port_id, port_number, is_border,
            is_connected: Arc::new(AtomicBool::new(is_connected)),
            port_to_pe})
    }
    pub fn get_id(&self) -> PortID { self.id }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//  pub fn get_port_number(&self) -> PortNumber { self.port_number }
//  pub fn get_is_connected(&self) -> Arc<AtomicBool> { self.is_connected.clone() }
    pub fn is_connected(&self) -> bool { self.is_connected.load(SeqCst) }
    pub fn set_connected(&mut self) { self.is_connected.store(true, SeqCst); }
    pub fn set_disconnected(&mut self) { self.is_connected.store(false, SeqCst); }
    pub fn is_border(&self) -> bool { self.is_border }
    pub fn noc_channel(&self, port_to_noc: PortToNoc,
            port_from_noc: PortFromNoc, port_from_pe: PortFromPe) -> Result<JoinHandle<()>, Error> {
        self.port_to_pe.send(PortToPePacket::Status((self.get_port_no(), self.is_border, PortStatus::Connected))).context(PortError::Chain { func_name: "outside_channel", comment: S(self.id.get_name()) + " send to pe"})?;
        self.listen_noc_for_pe(port_from_noc)?;
        let join_handle = self.listen_pe_for_noc(port_to_noc, port_from_pe)?;
        Ok(join_handle)
    }

    // SPAWN THREAD (listen_noc_for_pe_loop)
    fn listen_noc_for_pe(&self, port_from_noc: PortFromNoc) -> Result<(), Error> {
        let _f = "listen_noc_for_pe";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_noc_for_pe_loop", self.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_noc_for_pe_loop(&port_from_noc).map_err(|e| write_err("port", &e));
            if CONTINUE_ON_ERROR { let _ = port.listen_noc_for_pe(port_from_noc); }
        })?;
        Ok(())
    }

    // WORKER (PortFromNoc)
    fn listen_noc_for_pe_loop(&self, port_from_noc: &PortFromNoc) -> Result<(), Error> {
        let _f = "listen_noc_for_pe_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = port_from_noc.recv()?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "id": self.get_id().get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            //println!("Port to pe other_index {}", *other_index);
            self.port_to_pe.send(PortToPePacket::Tcp((self.port_number.get_port_no(), msg))).context(PortError::Chain { func_name: "listen_outside_for_pe", comment: S(self.id.get_name()) + " send to pe"})?;
        }
    }

    // SPAWN THREAD (listen_pe_for_noc_loop)
    fn listen_pe_for_noc(&self, port_to_noc: PortToNoc, port_from_pe: PortFromPe)
            -> Result<JoinHandle<()>, Error> {
        let _f = "listen_pe_for_noc";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_pe_for_noc_loop", self.get_id().get_name());
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe_for_noc_loop(&port_to_noc, &port_from_pe).map_err(|e| write_err("port", &e));
            if CONTINUE_ON_ERROR { let _ = port.listen_pe_for_noc(port_to_noc, port_from_pe); }
        })?;
        Ok(join_handle)
    }

    // WORKER (PortFromPe)
    fn listen_pe_for_noc_loop(&self, port_to_noc: &PortToNoc, port_from_pe: &PortFromPe)
            -> Result<(), Error> {
        let _f = "listen_pe_for_noc_loop";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.port_noc {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let msg = port_from_pe.recv().context(PortError::Chain { func_name: "listen_pe_for_outside", comment: S(self.id.get_name()) + " recv from pe"})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.port_noc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "id": self.get_id().get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            //println!("Port {}: waiting for packet from pe", port.id);
            let tuple = match msg {
                PeToPortPacket::Tcp(tuple) => tuple,
                _ => return Err(PortError::NonTcp { func_name: "listen_pe_for_noc", port_no: *self.port_number.get_port_no() }.into())
            };
            //println!("Port to Noc other_index {}", *tuple.0);
            port_to_noc.send(tuple).context(PortError::Chain { func_name: "listen_pe_for_outside", comment: S(self.id.get_name()) + " send to noc"})?;
        }
    }

    // SPAWN THREAD (listen_link, listen_pe)
    pub fn link_channel(&self, port_to_link: PortToLink, port_from_link: PortFromLink,
                        port_from_pe: PortFromPe) {
        let _f = "link_channel";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_link", self.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_link(&port_from_link).map_err(|e| write_err("port", &e));
            if CONTINUE_ON_ERROR { }
        }).expect("thread failed");

        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_pe", self.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe(&port_to_link, &port_from_pe).map_err(|e| write_err("port", &e));
            if CONTINUE_ON_ERROR { }
        }).expect("thread failed");
    }

    // WORKER (PortFromLink)
    fn listen_link(&mut self, port_from_link: &PortFromLink) -> Result<(), Error> {
        let _f = "listen_link";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        //println!("PortID {}: port_no {}", self.id, port_no);
        loop {
            //println!("Port {}: waiting for status or packet from link", port.id);
            let msg = port_from_link.recv().context(PortError::Chain { func_name: "listen_link", comment: S(self.id.get_name()) + " recv from link"})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "id": self.get_id().get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            match msg {
                LinkToPortPacket::Status(status) => {
                    match status {
                        PortStatus::Connected => self.set_connected(),
                        PortStatus::Disconnected => self.set_disconnected()
                    };
                    self.port_to_pe.send(PortToPePacket::Status((self.port_number.get_port_no(), self.is_border, status))).context(PortError::Chain { func_name: "listen_pe_for_outside", comment: S(self.id.get_name()) + " send status to pe"})?;
                }
                LinkToPortPacket::Packet(packet) => {
                    //println!("Port {}: got from link {} {}", self.id, *my_index, packet);
                    self.port_to_pe.send(PortToPePacket::Packet((self.port_number.get_port_no(), packet))).context(PortError::Chain { func_name: "listen_pe_for_outside", comment: S(self.id.get_name()) + " send packet to pe"})?;
                    //println!("Port {}: sent from link to pe {}", self.id, packet);
                }
            }
        }
    }

    // WORKER (PortFromPe)
    fn listen_pe(&self, port_to_link: &PortToLink, port_from_pe: &PortFromPe) -> Result<(), Error> {
        let _f = "listen_pe";
        {
            if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            //println!("Port {}: waiting for packet from pe", id);
            let msg = port_from_pe.recv().context(PortError::Chain { func_name: "listen_pe", comment: S(self.id.get_name()) + " recv from port"})?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                    let trace = json!({ "id": self.get_id().get_name(), "msg": msg });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let packet = match msg {
                PeToPortPacket::Packet(packet) => packet,
                _ => return Err(PortError::Tcp { func_name: "listen_pe", port_no: *self.port_number.get_port_no() }.into())
            };
            //println!("Port {}: got other_index from pe {}", self.id, *packet.0);
            port_to_link.send(packet).context(PortError::Chain { func_name: "listen_pe", comment: S(self.id.get_name()) + " send to link"})?;
            //println!("Port {}: sent from pe to link {}", id, packet);
        }
    }
}
impl fmt::Display for Port { 
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
    #[fail(display = "PortError::NonTcp {}: Non TCP message received on port {}, with is a border port", func_name, port_no)]
    NonTcp { func_name: &'static str, port_no: u8 },
    #[fail(display = "PortError::Tcp {}: TCP message received on port {}, with is not a border port", func_name, port_no)]
    Tcp { func_name: &'static str, port_no: u8 }
}
