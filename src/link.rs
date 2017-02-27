use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use name::{Name, LinkID};
use packet::Packet;
use port::Port;

pub type Sender = mpsc::Sender<Packet>;
pub type Receiver = mpsc::Receiver<Packet>;
#[derive(Debug)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		//     Left Port        Link        Right Port
	left_send: Sender,		//        R1              S1
	rite_send: Sender,      //                        S2            R2
	left_recv: Receiver,    //        S3              R3
	rite_recv: Receiver     //                        R4            S4
}
impl Link {
	pub fn new(left: &mut Port, rite: &mut Port) -> Result<Link,LinkError> {
		let temp_id = try!(left.get_id().add_component(&rite.get_id().get_name()));
		let id = try!(LinkID::new(&temp_id.get_name()));
		let (left_send, left_port_recv) = channel();
		let (rite_send, rite_port_recv) = channel();
		let (left_port_send, left_recv)  = channel();
		let (rite_port_send, rite_recv) = channel();
		left.set_connected(Some(left_port_send), Some(left_port_recv));
		rite.set_connected(Some(rite_port_send), Some(rite_port_recv));
		Ok(Link { id: id, is_broken: false, is_connected: true,
				left_send: left_send, rite_send: rite_send,
				left_recv: left_recv, rite_recv: rite_recv })
	}
	pub fn to_string(&self) -> String {
		let mut s = format!("\nLink {}", self.id.get_name().to_string());
		if self.is_connected { s = s + " is connected"; }
		else                 { s = s + " is not connected"; }
		s
	}
}
impl fmt::Display for Link { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum LinkError {
	Name(NameError),
}
impl Error for LinkError {
	fn description(&self) -> &str {
		match *self {
			LinkError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			LinkError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for LinkError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			LinkError::Name(_) => write!(f, "Link Name Error caused by")
		}
	}
}
impl From<NameError> for LinkError {
	fn from(err: NameError) -> LinkError { LinkError::Name(err) }
}
