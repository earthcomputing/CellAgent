use std::fmt;
use std::sync::mpsc;
use crossbeam::Scope;
use nalcell::{PortNumber, PortStatusSender, PortStatusSenderError};
use packet_engine::{SendPacket, ReceivePacket};
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
	send_to_ca: PortStatusSender,
}
impl Port {
	pub fn new(scope: &Scope, cell_id: &CellID, port_no: PortNumber, is_border: bool, is_connected: bool,
			   send_to_pe: SendPacket, recv_from_pe: ReceivePacket, send_to_ca: PortStatusSender) -> Result<Port,NameError>{
		let port_id = try!(PortID::new(port_no.get_port_no()));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let port = Port{ id: temp_id, port_no: port_no, is_border: is_border, is_connected: is_connected, 
			is_broken: false, send_to_ca: send_to_ca };
		port.listen_for_outgoing(scope, send_to_pe, recv_from_pe);
		Ok(port)
	}
	fn listen_for_outgoing(&self, scope: &Scope, send_to_pe: SendPacket, recv_from_pe: ReceivePacket) {
		let port_id = self.id.clone();
		scope.spawn( move || -> Result<(), PortError> {
				loop {
					let packet = try!(recv_from_pe.recv());
					println!("Port {} received packet {}", port_id, packet);
				}
				Ok(())
			}
		);
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_no(&self) -> u8 { self.port_no.get_port_no() }
	pub fn get_port_number(&self) -> PortNumber { self.port_no }
	pub fn is_connected(&self) -> bool { self.is_connected }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&mut self, send: SendPacket, recv: ReceivePacket) -> Result<(),PortStatusSenderError> {
		println!("Port {} connected", self.id); 
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
	Recv(mpsc::RecvError)
}
impl Error for PortError {
	fn description(&self) -> &str {
		match *self {
			PortError::Send(ref err) => err.description(),
			PortError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PortError::Send(ref err) => Some(err),
			PortError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PortError::Send(ref err) => write!(f, "Port Send Error caused by {}", err),
			PortError::Recv(ref err) => write!(f, "Port Receive Error caused by {}", err),
		}
	}
}
impl From<PortStatusSenderError> for PortError {
	fn from(err: PortStatusSenderError) -> PortError { PortError::Send(err) }
}
impl From<mpsc::RecvError> for PortError {
	fn from(err: mpsc::RecvError) -> PortError { PortError::Recv(err) }
}
