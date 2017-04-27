use std::fmt;
use std::sync::mpsc;
use crossbeam::Scope;
use config::PortNo;
use nalcell::{StatusPortToCa, PacketSend, PacketRecv, 
	RecvrPortFromCa, PacketPortToPe};
use name::{Name, PortID, CellID};
use packet::Packet;
use utility::PortNumber;

#[derive(Debug, Copy, Clone)]
pub enum PortStatus {
	Connected,
	Disconnected,
}

#[derive(Debug)]
pub struct Port {
	id: PortID,
	port_number: PortNumber,
	is_border: bool,
	is_connected: bool,
	is_broken: bool,
	status_port_to_ca: StatusPortToCa,
	packet_port_to_pe: PacketPortToPe,
	recv_port_from_ca: RecvrPortFromCa,
}
impl Port {
	pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
			   packet_port_to_pe: PacketPortToPe, status_port_to_ca: StatusPortToCa,
			   recv_port_from_ca: RecvrPortFromCa) -> Result<Port,PortError>{
		let port_id = try!(PortID::new(port_number.get_port_no()));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let port = Port{ id: temp_id, port_number: port_number, is_border: is_border, is_connected: is_connected, 
			is_broken: false, status_port_to_ca: status_port_to_ca, packet_port_to_pe: packet_port_to_pe,
			recv_port_from_ca: recv_port_from_ca };
		Ok(port)
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
	pub fn get_port_number(&self) -> PortNumber { self.port_number }
	pub fn is_connected(&self) -> bool { self.is_connected }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&mut self, scope: &Scope, packet_port_to_link: PacketSend, 
				packet_port_from_link: PacketRecv) -> Result<(),PortError> {
		self.is_connected = true; 
		let port_no = self.get_port_no();
		//println!("Port {}: sending status", self.id);
		try!(self.status_port_to_ca.send((port_no, PortStatus::Connected)));
		let packet_port_from_pe = try!(self.recv_port_from_ca.recv());
		//println!("Port {}: got recvr", self.id);
		let packet_port_to_pe = self.packet_port_to_pe.clone();
		let port_id = self.id.clone();
		// Listen for outgoing packets
		scope.spawn( move || -> Result<(), PortError> {
			loop {
				let (packet_count, packet) = try!(packet_port_from_pe.recv());
				//println!("Port {}: sent packet {} to link", port_id, packet_count);
				try!(packet_port_to_link.send((packet_count, packet)));
			}
		}); 
		// Listen for incoming packets
		let port_id = self.id.clone();
		scope.spawn( move || -> Result<(), PortError> {
			loop {
				let (packet_count,packet) = try!(packet_port_from_link.recv());
				try!(packet_port_to_pe.send((packet_count,port_no, packet)));
				//println!("Port {}: sent packet {} to packet engine", port_id, packet_count);
			}
		});
		Ok(())
	}
}
impl fmt::Display for Port { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let is_connected = self.is_connected;
		let mut s = format!("Port {} {}", self.port_number, self.id);
		if self.is_border { s = s + " is TCP  port,"; }
		else              { s = s + " is ECLP port,"; }
		if is_connected   { s = s + " is connected"; }
		else              { s = s + " is not connected"; }
		if self.is_broken { s = s + " and is broken"; }
		else              { s = s + " and is not broken"; }
		write!(f, "{}", s) 
	}
}
// Errors
use std::error::Error;
use nalcell::PortStatusSendError;
use name::NameError;
#[derive(Debug)]
pub enum PortError {
	Name(NameError),
	Channel(ChannelError),
	SendStatus(PortStatusSendError),
	SendToPe(mpsc::SendError<(usize,u8,Packet)>),
	Send(mpsc::SendError<(usize,Packet)>),
	Recv(mpsc::RecvError)
}
impl Error for PortError {
	fn description(&self) -> &str {
		match *self {
			PortError::Name(ref err) => err.description(),
			PortError::Channel(ref err) => err.description(),
			PortError::SendStatus(ref err) => err.description(),
			PortError::SendToPe(ref err) => err.description(),
			PortError::Send(ref err) => err.description(),
			PortError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PortError::Name(ref err) => Some(err),
			PortError::Channel(ref err) => Some(err),
			PortError::SendStatus(ref err) => Some(err),
			PortError::SendToPe(ref err) => Some(err),
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
			PortError::SendToPe(ref err) => write!(f, "Port Send Packet Error caused by {}", err),
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
impl From<mpsc::SendError<(usize,Packet)>> for PortError {
	fn from(err: mpsc::SendError<(usize,Packet)>) -> PortError { PortError::Send(err) }
}
impl From<mpsc::SendError<(usize,u8,Packet)>> for PortError {
	fn from(err: mpsc::SendError<(usize,u8,Packet)>) -> PortError { PortError::SendToPe(err) }
}
impl From<mpsc::RecvError> for PortError {
	fn from(err: mpsc::RecvError) -> PortError { PortError::Recv(err) }
}
