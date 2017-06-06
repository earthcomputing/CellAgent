use std::fmt;
use std::sync::mpsc;
use crossbeam::Scope;
use nalcell::{LinkToPort, LinkFromPort, LinkPortError};
use name::{Name, LinkID, PortID};
use port::{PortStatus};

#[derive(Debug, Clone)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		      //     Left Port        Link        Right Port
}
#[deny(unused_must_use)]
impl Link {
	pub fn new(scope: &Scope, left_id: &PortID, rite_id: &PortID,
			link_to_left: LinkToPort, link_from_left: LinkFromPort,
			link_to_rite: LinkToPort, link_from_rite: LinkFromPort )
				-> Result<Link,LinkError> {
		let rite_name = rite_id.get_name();
		let temp_id = match left_id.add_component(&rite_name) {
			Ok(x) => x,
			Err(err) => return Err(LinkError::Name(err))
		};
		let id = LinkID::new(&temp_id.get_name())?;
		let link = Link { id: id, is_broken: false, is_connected: true };
		link.listen(scope, link_to_left.clone(), link_from_left, link_to_rite.clone())?;
		link.listen(scope, link_to_rite, link_from_rite, link_to_left)?;
		Ok(link)
	}
	fn listen(&self, scope: &Scope, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort) 
				-> Result<(), LinkError> {
		let link = self.clone();
		scope.spawn( move || -> Result<(), LinkError> {
			status.send((Some(PortStatus::Connected),None))?;
			match link.listen_loop(link_from, link_to) {
				Ok(val) => Ok(val),
				Err(err) => {
					println!("--- Link {}: from left {}", link.id, err);
					Err(err)
				}
			}
		});
		Ok(())
	}			
	fn listen_loop(&self, link_from: LinkFromPort, link_to: LinkToPort) -> Result<(),LinkError> {
		loop {
			//println!("Link {}: waiting to recv left", link_id);
			let packet = link_from.recv()?;
			link_to.send((None,Some(packet)))?;
			//println!("Link {}: sent packet {} right", link_id, packet.get_count());
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
use std::error::Error;
use name::{NameError};
use port::{PortError};
#[derive(Debug)]
pub enum LinkError {
	Name(NameError),
	Port(PortError),
	Send(LinkPortError),
	Recv(mpsc::RecvError),
}
impl Error for LinkError {
	fn description(&self) -> &str {
		match *self {
			LinkError::Name(ref err) => err.description(),
			LinkError::Port(ref err) => err.description(),
			LinkError::Send(ref err) => err.description(),
			LinkError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			LinkError::Name(ref err) => Some(err),
			LinkError::Port(ref err) => Some(err),
			LinkError::Send(ref err) => Some(err),
			LinkError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for LinkError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			LinkError::Name(ref err) => write!(f, "Link Name Error caused by {}", err),
			LinkError::Port(ref err) => write!(f, "Link Port Error caused by {}", err),
			LinkError::Send(ref err) => write!(f, "Link Send Error caused by {}", err),
			LinkError::Recv(ref err) => write!(f, "Link Receive Error caused by {}", err),
		}
	}
}
impl From<NameError> for LinkError {
	fn from(err: NameError) -> LinkError { LinkError::Name(err) }
}
impl From<PortError> for LinkError {
	fn from(err: PortError) -> LinkError { LinkError::Port(err) }
}
impl From<LinkPortError> for LinkError {
	fn from(err: LinkPortError) -> LinkError { LinkError::Send(err) }
}
impl From<mpsc::RecvError> for LinkError {
	fn from(err: mpsc::RecvError) -> LinkError { LinkError::Recv(err) }
}