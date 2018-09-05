use std::fmt;
use std::thread::JoinHandle;

use message_types::{LinkToPort, LinkFromPort, LinkToPortPacket};
use name::{Name, LinkID, PortID};
use port::{PortStatus};
use utility::{S, write_err};

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Clone)]
pub struct Link {
	id: LinkID,
	is_connected: bool,		      //     Left Port        Link        Right Port
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
			link_to_rite: LinkToPort, link_from_rite: LinkFromPort ) 
				-> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "start_threads";
        self.to_left = Some(link_to_left.clone());
        self.to_rite = Some(link_to_rite.clone());
		let left_handle = self.listen(link_to_left.clone(), link_from_left,
									  link_to_rite.clone()).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " left"})?;
		let rite_handle = self.listen(link_to_rite, link_from_rite,
                                      link_to_left).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) + " rite"})?;
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
    fn listen_port(&self, link_from: LinkFromPort, link_to: LinkToPort) -> Result<JoinHandle<()>, Error> {
        let link = self.clone();
        let join_handle = ::std::thread::spawn( move || {
            let _ = link.listen_loop(&link_from, &link_to).map_err(|e| write_err("link", e.into()));
            let _ = link.listen_port(link_from, link_to);
        });
        Ok(join_handle)
    }
    fn listen_loop(&self, link_from: &LinkFromPort, link_to: &LinkToPort) -> Result<(), Error> {
        let _f = "listen_loop";
        loop {
            let packet = link_from.recv().context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) })?;
            link_to.send(LinkToPortPacket::Packet(packet)).context(LinkError::Chain { func_name: _f, comment: S(self.id.clone()) })?;
        }
    }
}
impl fmt::Display for Link { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
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
