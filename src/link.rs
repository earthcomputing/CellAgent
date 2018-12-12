use std::{fmt, thread, thread::JoinHandle};

use crate::config::{CONTINUE_ON_ERROR, TRACE_OPTIONS};
use crate::dal;
use crate::dal::{fork_trace_header, update_trace_header};
use crate::message_types::{LinkToPort, LinkFromPort, LinkToPortPacket};
use crate::name::{Name, LinkID, PortID};
use crate::port::{PortStatus};
use crate::utility::{S, write_err, TraceHeader, TraceHeaderParams, TraceType};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Clone)]
pub struct Link {
    id: LinkID,
    is_connected: bool,              //     Left Port        Link        Right Port
    to_left: Option<LinkToPort>,
    to_rite: Option<LinkToPort>
}
impl Link {
    pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<Link, Error> {
        let id = LinkID::new(left_id, rite_id)?;
        Ok(Link { id, is_connected: true, to_left: None, to_rite: None })
    }
    pub fn get_id(&self) -> &LinkID { &self.id }
    pub fn start_threads(&mut self,
            link_to_left: LinkToPort, link_from_left: LinkFromPort,
            link_to_rite: LinkToPort, link_from_rite: LinkFromPort)
                -> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "start_threads";
        self.to_left = Some(link_to_left.clone());
        self.to_rite = Some(link_to_rite.clone());
        let left_handle = self.listen(link_to_left.clone(), link_from_left, link_to_rite.clone())
            .context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        let rite_handle = self.listen(link_to_rite, link_from_rite, link_to_left)
            .context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " rite"})?;
        Ok(vec![left_handle, rite_handle])
    }
    pub fn break_link(&mut self) -> Result<(), Error> {
        let _f = "break_link";
        self.is_connected = false;
        self.clone().to_left.expect("Cannot fail in break_link").send(LinkToPortPacket::Status(PortStatus::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        self.clone().to_rite.expect("Cannot fail in break_link").send(LinkToPortPacket::Status(PortStatus::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        Ok(())
    }
    fn listen(&self, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort)
            -> Result<JoinHandle<()>, Error> {
        let _f = "listen";
        status.send(LinkToPortPacket::Status(PortStatus::Connected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " send status to port"})?;
        let join_handle = self.listen_port(link_from, link_to)?;
        Ok(join_handle)
    }

    // SPAWN THREAD (listen_loop)
    fn listen_port(&self, link_from: LinkFromPort, link_to: LinkToPort) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_port";
        let link = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Link {} listen_loop", self.get_id());
        let join_handle = thread::Builder::new().name(thread_name.into()).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = link.listen_loop(&link_from, &link_to).map_err(|e| write_err("link", e.into()));
            if CONTINUE_ON_ERROR { let _ = link.listen_port(link_from, link_to); }
        })?;
        Ok(join_handle)
    }

    // WORKER (LinkFromPort)
    fn listen_loop(&self, link_from: &LinkFromPort, link_to: &LinkToPort) -> Result<(), Error> {
        let _f = "listen_loop";
        if TRACE_OPTIONS.all || TRACE_OPTIONS.link {
            let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
            let trace = json!({ "id": &self.get_id(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
            let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        loop {
            let msg = link_from.recv().context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) })?;
            if TRACE_OPTIONS.all || TRACE_OPTIONS.link {
                let ref trace_params = TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "recv" };
                let trace = json!({ "id": &self.get_id(), "msg": msg });
                let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
            link_to.send(LinkToPortPacket::Packet(msg)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) })?;
        }
    }
}
impl fmt::Display for Link { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Link {}", self.id.get_name().to_string());
        if self.is_connected { s = s + " is connected"; }
        else                 { s = s + " is not connected"; }
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum LinkError {
    #[fail(display = "LinkError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
