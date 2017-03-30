use std::fmt;
use std::sync::mpsc;
use crossbeam::Scope;
use nalcell::{PortNumber, StatusPortToCa, PortStatusSendError, SendPacket, RecvPacket};
use packet::Packet;
use name::{Name, NameError,PortID,CellID};

#[derive(Debug, Copy, Clone)]
pub enum PortStatus {
	Connected,
	Disconnected,
}

#[derive(Debug, Clone)]
pub struct Port {
	id: PortID,
	port_no: PortNumber,
	is_border: bool,
	is_connected: bool,
	is_broken: bool,
	status_port_to_ca: StatusPortToCa,
	packet_port_to_pe: SendPacket
}
impl Port {
	pub fn new(scope: &Scope, cell_id: &CellID, port_no: PortNumber, is_border: bool, is_connected: bool,
			   packet_port_to_pe: SendPacket, status_port_to_ca: StatusPortToCa) -> Result<Port,PortError>{
		let port_id = try!(PortID::new(port_no.get_port_no()));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let port = Port{ id: temp_id, port_no: port_no, is_border: is_border, is_connected: is_connected, 
			is_broken: false, status_port_to_ca: status_port_to_ca, packet_port_to_pe: packet_port_to_pe };
		Ok(port)
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_no(&self) -> u8 { self.port_no.get_port_no() }
	pub fn get_port_number(&self) -> PortNumber { self.port_no }
	pub fn is_connected(&self) -> bool { self.is_connected }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&mut self, send_to_link: SendPacket, recv_from_link: RecvPacket) -> Result<(),PortStatusSendError> {
		println!("Port {} connected", self.id); 
		self.is_connected = true; 
		try!(self.status_port_to_ca.send((self.port_no.get_port_no(), PortStatus::Connected)));
		Ok(())
	}
	pub fn listen_for_outgoing(&self, scope: &Scope, 
			send_to_link: SendPacket, recv_from_pe: RecvPacket) -> Result<(), PortError> {
		let port_id = self.id.clone();
		scope.spawn( move || -> Result<(), PortError> {
			println!("spawn Port {} listening for packets", port_id);
			loop {
				let packet = try!(recv_from_pe.recv());
				try!(send_to_link.clone().send(packet));
				println!("Port {} received packet {}", port_id, packet);
			}
				Ok(())
		});
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
	Name(NameError),
	Channel(ChannelError),
	SendStatus(PortStatusSendError),
	Send(mpsc::SendError<Packet>),
	Recv(mpsc::RecvError)
}
impl Error for PortError {
	fn description(&self) -> &str {
		match *self {
			PortError::Name(ref err) => err.description(),
			PortError::Channel(ref err) => err.description(),
			PortError::SendStatus(ref err) => err.description(),
			PortError::Send(ref err) => err.description(),
			PortError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PortError::Name(ref err) => Some(err),
			PortError::Channel(ref err) => Some(err),
			PortError::SendStatus(ref err) => Some(err),
			PortError::Send(ref err) => Some(err),
			PortError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PortError::Name(ref err) => write!(f, "Port Name Error caused by {}", err),
			PortError::Channel(ref err) => write!(f, "Port Channel Error caused by {}", err),
			PortError::SendStatus(ref err) => write!(f, "Port Send Status Error caused by {}", err),
			PortError::Send(ref err) => write!(f, "Port Send Error caused by {}", err),
			PortError::Recv(ref err) => write!(f, "Port Receive Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct ChannelError { msg: String }
impl ChannelError { 
	pub fn new() -> ChannelError {
		ChannelError { msg: format!("No channel to link") }
	}
}
impl Error for ChannelError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for ChannelError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NameError> for PortError {
	fn from(err: NameError) -> PortError { PortError::Name(err) }
}
impl From<ChannelError> for PortError {
	fn from(err: ChannelError) -> PortError { PortError::Channel(err) }
}
impl From<PortStatusSendError> for PortError {
	fn from(err: PortStatusSendError) -> PortError { PortError::SendStatus(err) }
}
impl From<mpsc::SendError<Packet>> for PortError {
	fn from(err: mpsc::SendError<Packet>) -> PortError { PortError::Send(err) }
}
impl From<mpsc::RecvError> for PortError {
	fn from(err: mpsc::RecvError) -> PortError { PortError::Recv(err) }
}
