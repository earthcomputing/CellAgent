use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use crossbeam::Scope;
use nalcell::{PacketSend,PacketRecv};
use name::{Name, LinkID};
use packet::Packet;
use port::{Port, PortError};

#[derive(Debug)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		      //     Left Port        Link        Right Port
}
impl Link {
	pub fn new(scope: &Scope, left: &mut Port, rite: &mut Port) -> Result<Link,LinkError> {
		let left_id = left.get_id();
		let rite_id = rite.get_id();
		let rite_name = rite_id.get_name();
		let temp_id = match left_id.add_component(&rite_name) {
			Ok(x) => x,
			Err(err) => return Err(LinkError::Name(err))
		};
		let id = try!(LinkID::new(&temp_id.get_name()));
		let (packet_link_to_left, packet_left_from_link) = channel();
		let (packet_link_to_rite, packet_rite_from_link) = channel();
		let (packet_left_to_link, packet_link_from_left)  = channel();
		let (packet_rite_to_link, packet_link_from_rite) = channel();
		let link = Link { id: id, is_broken: false, is_connected: true };
		try!(link.listen(scope, packet_link_from_left, packet_link_to_rite, 
							    packet_link_from_rite, packet_link_to_left));
		try!(left.set_connected(scope, packet_left_to_link, packet_left_from_link));
		try!(rite.set_connected(scope, packet_rite_to_link, packet_rite_from_link));
		Ok(link)
	}
	fn listen(&self, scope: &Scope, packet_link_from_left: PacketRecv, packet_link_to_rite: PacketSend,
					 packet_link_from_rite: PacketRecv, packet_link_to_left: PacketSend) -> Result<(), LinkError> {
		let link_id = self.id.clone();
		scope.spawn( move || -> Result<(), LinkError> {
				loop {
					let packet = try!(packet_link_from_left.recv());
					try!(packet_link_to_rite.send(packet));
				}
			}
		);
		let link_id = self.id.clone();
		scope.spawn( move || -> Result<(), LinkError> {
				loop {
					let packet = try!(packet_link_from_rite.recv());
					try!(packet_link_to_left.send(packet));
				}
			}
		);
		Ok(())
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
#[derive(Debug)]
pub enum LinkError {
	Name(NameError),
	Port(PortError),
	Send(mpsc::SendError<Packet>),
	Recv(mpsc::RecvError)
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
impl From<mpsc::SendError<Packet>> for LinkError {
	fn from(err: mpsc::SendError<Packet>) -> LinkError { LinkError::Send(err) }
}
impl From<mpsc::RecvError> for LinkError {
	fn from(err: mpsc::RecvError) -> LinkError { LinkError::Recv(err) }
}
