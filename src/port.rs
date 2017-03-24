use std::fmt;
use std::thread;
use nalcell::{PortNumber, PortStatusSender, PortStatusSenderError};
use cellagent::{SendPacket, ReceivePacket, SendPacketError};
use name::{Name, NameError,PortID,CellID};

#[derive(Debug, Copy, Clone)]
pub enum PortStatus {
	Connected,
	Disconnected,
}

#[derive(Debug)]
pub struct Port {
	id: PortID,
	port_no: PortNumber,
	is_border: bool,
	is_connected: bool,
	is_broken: bool,
	send_to_ca: PortStatusSender,
}
impl Port {
	pub fn new(cell_id: &CellID, port_no: PortNumber, is_border: bool, is_connected: bool,
			   send_to_pe: SendPacket, recv_from_pe: ReceivePacket, send_to_ca: PortStatusSender) -> Result<Port,NameError>{
		let port_id = try!(PortID::new(port_no.get_port_no()));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let port = Port{ id: temp_id, port_no: port_no, is_border: is_border, is_connected: is_connected, 
			is_broken: false, send_to_ca: send_to_ca };
		port.work(&port_id);
		Ok(port)
	}
	fn work(&self, port_id: &PortID) {
		//println!("Port {} worker", port_id);
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_no(&self) -> u8 { self.port_no.get_port_no() }
	pub fn get_port_number(&self) -> PortNumber { self.port_no }
	pub fn is_connected(&self) -> bool { self.is_connected }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&mut self, send: SendPacket, recv: ReceivePacket) -> Result<(),PortStatusSenderError> { 
		self.is_connected = true; 
		let send_to_link = send;
		let recv_from_link = recv;
		try!(self.send_to_ca.send((self.port_no.get_port_no(), PortStatus::Connected)));
		Ok(())
	}
	pub fn stringify(&self) -> String {
		let is_connected = self.is_connected;
		let mut s = format!("Port {} {}", self.port_no, self.id);
		if self.is_border { s = s + " is TCP  port,"; }
		else              { s = s + " is ECLP port,"; }
		if is_connected   { s = s + " is connected"; }
		else              { s = s + " is not connected"; }
		if self.is_broken { s = s + " and is broken"; }
		else              { s = s + " and is not broken"; }
		s
	}
}
impl fmt::Display for Port { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PortError {
	Send(PortStatusSenderError),
}
impl Error for PortError {
	fn description(&self) -> &str {
		match *self {
			PortError::Send(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PortError::Send(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PortError::Send(ref err) => write!(f, "Port Send Error caused by {}", err),
		}
	}
}
impl From<PortStatusSenderError> for PortError {
	fn from(err: PortStatusSenderError) -> PortError { PortError::Send(err) }
}
