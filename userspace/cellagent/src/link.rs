/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{fmt};

use crate::config::{CONFIG};
use crate::dal::{add_to_trace};
use crate::simulated_interior_port::{LinkFromPort, LinkToPort, LinkToPortPacket};
use crate::name::{Name, LinkID, PortID};
use crate::utility::{S, TraceHeaderParams, TraceType};

#[derive(Debug, Copy, Clone, Serialize)]
pub enum LinkStatus {
    Connected,
    Disconnected
}
impl fmt::Display for LinkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkStatus::Connected    => write!(f, "Connected"),
            LinkStatus::Disconnected => write!(f, "Disconnected")
        }
    }
}
// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Clone, Debug)]
pub struct DuplexLinkPortChannel {
    link_from_port: LinkFromPort,
    link_to_port: LinkToPort,
}
impl DuplexLinkPortChannel {
    pub fn new(link_from_port: LinkFromPort, link_to_port: LinkToPort) -> DuplexLinkPortChannel {
        DuplexLinkPortChannel { link_from_port, link_to_port }
    }
    pub fn get_link_from_port(&self) -> &LinkFromPort { &self.link_from_port }
    pub fn get_link_to_port(&self) -> &LinkToPort { &self.link_to_port }
}

#[derive(Clone, Debug)]
pub struct LinkToPorts {
    left: LinkToPort,
    rite: LinkToPort,
}
impl LinkToPorts {
    pub fn new(left: LinkToPort, rite: LinkToPort) -> LinkToPorts {
        LinkToPorts { left, rite }
    }
}

#[derive(Clone, Debug)]
pub struct LinkFromPorts {
    left: LinkFromPort,
    rite: LinkFromPort,
}
impl LinkFromPorts {
    pub fn new( left: LinkFromPort, rite: LinkFromPort) -> LinkFromPorts {
        LinkFromPorts { left, rite }
    }
}

#[derive(Debug, Clone)]
pub struct Link {
    id: LinkID,
    is_connected: bool,              //     Left Port        Link        Rite Port
    link_to_ports: LinkToPorts,
}
impl Link {
    pub fn new(left_id: PortID, rite_id: PortID,
               link_to_ports: LinkToPorts) -> Result<Link, Error> {
        let _f = "new";
        let id = LinkID::new(left_id, rite_id)?;
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.link {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "link_connected" };
                let trace = json!({ "id": id });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        link_to_ports.left.send(LinkToPortPacket::Status(LinkStatus::Connected)).context(LinkError::Chain { func_name: _f, comment: S(id) + " send status to port"})?;
        link_to_ports.rite.send(LinkToPortPacket::Status(LinkStatus::Connected)).context(LinkError::Chain { func_name: _f, comment: S(id) + " send status to port"})?;
        Ok(Link {
            id,
            is_connected: true,
            link_to_ports: LinkToPorts {
                left: link_to_ports.left,
                rite: link_to_ports.rite,
            },
        })
    }
    pub fn get_id(&self) -> LinkID { self.id }
    pub fn listen(&mut self, link_from_ports: LinkFromPorts)
                  -> Result<(), Error> {
        let _f = "listen";
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1));
            select! {
                recv(link_from_ports.left) -> recvd => {
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
                    self.link_to_ports.rite.send(LinkToPortPacket::Packet(packet)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " send to rite"})?;
                },
                recv(link_from_ports.rite) -> recvd => {
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
                    self.link_to_ports.left.send(LinkToPortPacket::Packet(packet)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " send to left"})?;
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
                let trace = json!({ "id": &self.get_id(), "status": LinkToPortPacket::Status(LinkStatus::Disconnected) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        self.link_to_ports.left.send(LinkToPortPacket::Status(LinkStatus::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
        self.link_to_ports.rite.send(LinkToPortPacket::Status(LinkStatus::Disconnected)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
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
