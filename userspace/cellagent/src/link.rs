use std::{fmt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::simulated_interior_port::{LinkFromPortOld, LinkToPortOld, LinkToPortPacketOld};
use crate::name::{Name, LinkID, PortID};
use crate::port::{PortStatusOld};
use crate::utility::{S, TraceHeaderParams, TraceType};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Clone, Debug)]
pub struct DuplexLinkPortChannel {
    pub link_from_port_old: LinkFromPortOld,
    pub link_to_port_old: LinkToPortOld,
}

#[derive(Clone, Debug)]
pub struct LinkToPortsOld {
    pub left_old: LinkToPortOld,
    pub rite_old: LinkToPortOld,
}

#[derive(Clone, Debug)]
pub struct LinkFromPortsOld {
    pub left_old: LinkFromPortOld,
    pub rite_old: LinkFromPortOld,
}

#[derive(Debug, Clone)]
pub struct Link {
    id: LinkID,
    is_connected: bool,              //     Left Port        Link        Rite Port
    link_to_ports: LinkToPortsOld,
}
impl Link {
    pub fn new(left_id: PortID, rite_id: PortID,
               link_to_ports: LinkToPortsOld) -> Result<Link, Error> {
        let _f = "new";
        let id = LinkID::new(left_id, rite_id)?;
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.link {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_connected" };
                let trace = json!({ "id": id });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        link_to_ports.left_old.send(LinkToPortPacketOld::Status(PortStatusOld::Connected)).context(LinkError::Chain { func_name: _f, comment: S(id) + " send status to port"})?;
        link_to_ports.rite_old.send(LinkToPortPacketOld::Status(PortStatusOld::Connected)).context(LinkError::Chain { func_name: _f, comment: S(id) + " send status to port"})?;
        Ok(Link {
            id,
            is_connected: true,
            link_to_ports: LinkToPortsOld {
                left_old: link_to_ports.left_old,
                rite_old: link_to_ports.rite_old,
            },
        })
    }
    pub fn get_id(&self) -> LinkID { self.id }
    pub fn listen(&mut self, link_from_ports: LinkFromPortsOld)
                  -> Result<(), Error> {
        let _f = "listen";
        loop {
            select! {
                recv(link_from_ports.left_old) -> recvd => {
                    let packet = recvd.context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " receive from left"})?;
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.link {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_from_left_port" };
                            let trace = json!({ "id": &self.get_id(), "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_to_rite_port" };
                            let trace = json!({ "id": &self.get_id(), "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.link_to_ports.rite_old.send(LinkToPortPacketOld::Packet(packet)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " send to rite"})?;
                },
                recv(link_from_ports.rite_old) -> recvd => {
                    let packet = recvd.context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " receive from rite"})?;
                    {
                        if CONFIG.trace_options.all || CONFIG.trace_options.link {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_from_rite_port" };
                            let trace = json!({ "id": &self.get_id(), "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_to_left_port" };
                            let trace = json!({ "id": &self.get_id(), "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                    }
                    self.link_to_ports.left_old.send(LinkToPortPacketOld::Packet(packet)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " send to left"})?;
                }
            }
        }
    }
    pub fn break_link(&mut self) -> Result<(), Error> {
        let _f = "break_link";
        self.is_connected = false;
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.link {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_to_port_disconnected" };
                let trace = json!({ "id": &self.get_id(), "status": LinkToPortPacketOld::Status(PortStatusOld::Disconnected) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.link_to_ports.left_old.send(LinkToPortPacketOld::Status(PortStatusOld::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        self.link_to_ports.rite_old.send(LinkToPortPacketOld::Status(PortStatusOld::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        Ok(())
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
