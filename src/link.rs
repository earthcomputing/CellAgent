use std::fmt;
use std::sync::mpsc::channel;
use cellagent::{SendPacketSmall,ReceivePacketSmall};
use name::{Name, LinkID};
use port::Port;

#[derive(Debug)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		       //     Left Port        Link        Right Port
	left_send: SendPacketSmall,	   //        R1              S1
	rite_send: SendPacketSmall,       //                        S2            R2
	left_recv: ReceivePacketSmall,    //        S3              R3
	rite_recv: ReceivePacketSmall     //                        R4            S4
}
impl Link {
	pub fn new(left: &mut Port, rite: &mut Port) -> Result<Link,LinkError> {
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
		Ok(Link { id: id, is_broken: false, is_connected: true,
				left_send: left_send, rite_send: rite_send,
				left_recv: left_recv, rite_recv: rite_recv })
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
