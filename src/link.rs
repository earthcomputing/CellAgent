use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use crossbeam::Scope;
use packet::Packet;
use packet_engine::{SendPacket,ReceivePacket};
use name::{Name, LinkID};
use port::Port;

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
			Err(err) => panic!("{}", err)
		};
		let id = try!(LinkID::new(&temp_id.get_name()));
		let (left_send, left_port_recv) = channel();
		let (rite_send, rite_port_recv) = channel();
		let (left_port_send, left_recv)  = channel();
		let (rite_port_send, rite_recv) = channel();
		left.set_connected(left_port_send, left_port_recv);
		rite.set_connected(rite_port_send, rite_port_recv);
		let link = Link { id: id, is_broken: false, is_connected: true };
		link.listen(scope, left_recv, rite_send, rite_recv, left_send);
		Ok(link)
	}
	fn listen(&self, scope: &Scope, left_recv: ReceivePacket, rite_send: SendPacket,
					 rite_recv: ReceivePacket, left_send: SendPacket) -> Result<(), LinkError> {
		let link_id = self.id.clone();
		scope.spawn( move || -> Result<(), LinkError> {
				loop {
					let packet = try!(left_recv.recv());
					try!(rite_send.clone().send(packet));
					println!("Link {} received packet {}", link_id, packet);
				}
				Ok(())
			}
		);
		let link_id = self.id.clone();
		scope.spawn( move || -> Result<(), LinkError> {
				loop {
					let packet = try!(rite_recv.recv());
					try!(left_send.clone().send(packet));
					println!("Link {} received packet {}", link_id, packet);
				}
				Ok(())
			}
		);
		Ok(())
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("\nLink {}", self.id.get_name().to_string());
		if self.is_connected { s = s + " is connected"; }
		else                 { s = s + " is not connected"; }
		s
	}
}
impl fmt::Display for Link { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum LinkError {
	Name(NameError),
	Send(mpsc::SendError<Packet>),
	Recv(mpsc::RecvError)
}
impl Error for LinkError {
	fn description(&self) -> &str {
		match *self {
			LinkError::Name(ref err) => err.description(),
			LinkError::Send(ref err) => err.description(),
			LinkError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			LinkError::Name(ref err) => Some(err),
			LinkError::Send(ref err) => Some(err),
			LinkError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for LinkError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			LinkError::Name(ref err) => write!(f, "Link Name Error caused by {}", err),
			LinkError::Send(ref err) => write!(f, "Link Send Error caused by {}", err),
			LinkError::Recv(ref err) => write!(f, "Link Receive Error caused by {}", err),
		}
	}
}
impl From<NameError> for LinkError {
	fn from(err: NameError) -> LinkError { LinkError::Name(err) }
}
impl From<mpsc::SendError<Packet>> for LinkError {
	fn from(err: mpsc::SendError<Packet>) -> LinkError { LinkError::Send(err) }
}
impl From<mpsc::RecvError> for LinkError {
	fn from(err: mpsc::RecvError) -> LinkError { LinkError::Recv(err) }
}
