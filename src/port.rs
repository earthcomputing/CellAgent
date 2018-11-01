use std::fmt;
use std::thread;
use std::thread::JoinHandle;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc};
use std::sync::atomic::Ordering::SeqCst;

use config::{CONTINUE_ON_ERROR, PortNo};
use dal;
use message_types::{PortToLink, PortFromLink, PortToPe, PortFromPe, LinkToPortPacket, PortToPePacket,
              PeToPortPacket, PortToNoc, PortFromNoc};
use name::{Name, PortID, CellID};
use utility::{PortNumber, S, write_err, TraceHeader, TraceHeaderParams, TraceType};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Copy, Clone, Serialize)]
pub enum PortStatus {
    Connected,
    Disconnected,
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
    pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
               port_to_pe: PortToPe) -> Result<Port, Error> {
        let port_id = PortID::new(cell_id, port_number).context(PortError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
        Ok(Port{ id: port_id, port_number, is_border,
            is_connected: Arc::new(AtomicBool::new(is_connected)),
            port_to_pe})
    }
    pub fn get_id(&self) -> &PortID { &self.id }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//	pub fn get_port_number(&self) -> PortNumber { self.port_number }
//	pub fn get_is_connected(&self) -> Arc<AtomicBool> { self.is_connected.clone() }
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
    fn listen_noc_for_pe(&self, port_from_noc: PortFromNoc, trace_header: TraceHeader) -> Result<(), Error> {
        let thread_name = format!("PacketEngine {} to PortSet", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            let ref mut child_trace_header = trace_header.fork_trace();
            let port = self.clone();
            let _ = port.listen_noc_for_pe_loop(&port_from_noc, child_trace_header).map_err(|e| write_err("port", e));
            let _ = port.listen_noc_for_pe(port_from_noc);
        });
        join_handle?;
        Ok(())
    }

    // WORKER (PortFromNoc)
    fn listen_noc_for_pe_loop(&self, port_from_noc: &PortFromNoc, mut trace_header: TraceHeader) -> Result<(), Error> {
        let _f = "listen_noc_for_pe_loop";
        {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = port_from_noc.recv()?;
            {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": msg });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
            }
            //println!("Port to pe other_index {}", *other_index);
            self.port_to_pe.send(PortToPePacket::Tcp((self.port_number.get_port_no(), msg))).context(PortError::Chain { func_name: "listen_outside_for_pe", comment: S(self.id.get_name()) + " send to pe"})?;
        }
    }

    // SPAWN THREAD (listen_pe_for_noc_loop)
    fn listen_pe_for_noc(&self, port_to_noc: PortToNoc, port_from_pe: PortFromPe, trace_header: TraceHeader) -> Result<JoinHandle<()>, Error> {
        let thread_name = format!("PacketEngine {} to PortSet", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            let ref mut child_trace_header = trace_header.fork_trace();
            let port = self.clone();
            let _ = port.listen_pe_for_noc_loop(&port_to_noc, &port_from_pe, child_trace_header).map_err(|e| write_err("port", e));
            if CONTINUE_ON_ERROR { let _ = port.listen_pe_for_noc(port_to_noc, port_from_pe); }
        });
        Ok(join_handle?)
    }

    // WORKER (PortFromPe)
    fn listen_pe_for_noc_loop(&self, port_to_noc: &PortToNoc, port_from_pe: &PortFromPe, mut trace_header: TraceHeader) -> Result<(), Error> {
        let _f = "listen_pe_for_noc_loop";
        {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = port_from_pe.recv().context(PortError::Chain { func_name: "listen_pe_for_outside", comment: S(self.id.get_name()) + " recv from pe"})?;
            {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": msg });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
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
    pub fn link_channel(&self, port_to_link: PortToLink, port_from_link: PortFromLink, port_from_pe: PortFromPe, trace_header: TraceHeader) {
        let thread_name = format!("PacketEngine {} to PortSet", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            let ref mut child_trace_header = trace_header.fork_trace();
            let mut port = self.clone();
            let _ = port.listen_link(port_from_link, child_trace_header).map_err(|e| write_err("port", e));
        });
        join_handle?;

        let thread_name = format!("PacketEngine {} to PortSet", self.cell_id.get_name());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            let ref mut child_trace_header = trace_header.fork_trace();
            let port = self.clone();
            let _ = port.listen_pe(port_to_link, port_from_pe, child_trace_header).map_err(|e| write_err("port", e));
        });
        join_handle?;
    }

    // WORKER (PortFromLink)
    fn listen_link(&mut self, port_from_link: PortFromLink, mut trace_header: TraceHeader) -> Result<(), Error> {
        let _f = "listen_link";
        {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
        }
        //println!("PortID {}: port_no {}", self.id, port_no);
        loop {
            //println!("Port {}: waiting for status or packet from link", port.id);
            let msg = port_from_link.recv().context(PortError::Chain { func_name: "listen_link", comment: S(self.id.get_name()) + " recv from link"})?;
            {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.clone() });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
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
    fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe, mut trace_header: TraceHeader) -> Result<(), Error> {
        let _f = "listen_pe";
        {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "cell_id": &self.cell_id, "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            //println!("Port {}: waiting for packet from pe", id);
            let msg = port_from_pe.recv().context(PortError::Chain { func_name: "listen_pe", comment: S(self.id.get_name()) + " recv from port"})?;
            {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "cell_id": &self.cell_id, "msg": &msg.clone() });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, _f);
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
