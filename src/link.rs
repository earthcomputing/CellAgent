use std::fmt;
use std::thread::JoinHandle;
use serde_json;

use message_types::{LinkToPort, LinkFromPort, LinkToPortPacket};
use name::{Name, LinkID, PortID};
use port::{PortStatus};
use utility::write_err;

#[derive(Debug, Clone, Serialize)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		      //     Left Port        Link        Right Port
}
impl Link {
	pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<Link, Error> {
		let id = LinkID::new(left_id, rite_id)?;
		::utility::append2file(serde_json::to_string(&id).context(LinkError::Chain { func_name: "new", comment: ""})?).context(LinkError::Chain { func_name: "new", comment: ""})?;
		Ok(Link { id: id, is_broken: false, is_connected: true })
	}
//	pub fn get_id(&self) -> &LinkID { &self.id }
	pub fn start_threads(&self, 
			link_to_left: LinkToPort, link_from_left: LinkFromPort,
			link_to_rite: LinkToPort, link_from_rite: LinkFromPort ) 
				-> Result<Vec<JoinHandle<()>>, Error> {
		let left_handle = self.listen(link_to_left.clone(), link_from_left, link_to_rite.clone()).context(LinkError::Chain { func_name: "start_threads", comment: "left"})?;
		let rite_handle = self.listen(link_to_rite, link_from_rite, link_to_left).context(LinkError::Chain { func_name: "start_threads", comment: "rite"})?;
		Ok(vec![left_handle, rite_handle])
	}
	fn listen(&self, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort) 
			-> Result<JoinHandle<()>, Error> {
		//status.send(LinkToPortPacket::Status(PortStatus::Connected)).context(LinkError::Chain { func_name: "listen", comment: "send status to port"})?;
        let link = self.clone();
		let join_handle = ::std::thread::spawn( move || {
            let _ = link.listen_loop(status, link_from, link_to).map_err(|e| write_err("link", e.into()));
		});
        Ok(join_handle)
	}
    fn listen_loop(&self, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort) -> Result<(), Error> {
        loop {
            //println!("Link {}: waiting to recv", self.id);
            let packet = link_from.recv()?;
            link_to.send(LinkToPortPacket::Packet(packet)).context(LinkError::Chain { func_name: "listen_loop", comment: "" })?;
        }
    }
}
impl fmt::Display for Link { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nLink {}", self.id.get_name().to_string());
		if self.is_connected { s = s + " is connected"; }
		else                 { s = s + " is not connected"; }
		write!(f, "{}", s) 
	}
}
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum LinkError {
	#[fail(display = "LinkError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: &'static str },
}
