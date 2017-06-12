use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::sync::atomic::Ordering::SeqCst;
use crossbeam::{Scope};
use config::PortNo;
use nalcell::{PortToLink, PortFromLink, PortToPe, PortFromPe};
use name::{Name, PortID, CellID};
use utility::PortNumber;

#[derive(Debug, Copy, Clone)]
pub enum PortStatus {
	Connected,
	Disconnected,
}

#[derive(Debug, Clone)]
pub struct Port {
	id: PortID,
	port_number: PortNumber,
	is_border: bool,
	is_connected: Arc<AtomicBool>,
	is_broken: Arc<AtomicBool>,
	port_to_pe: PortToPe,
}
#[deny(unused_must_use)]
impl Port {
	pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
			   port_to_pe: PortToPe) -> Result<Port,PortError>{
		let port_id = try!(PortID::new(port_number.get_port_no()));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let port = Port{ id: temp_id, port_number: port_number, is_border: is_border, 
			is_connected: Arc::new(AtomicBool::new(is_connected)), 
			is_broken: Arc::new(AtomicBool::new(false)), port_to_pe: port_to_pe};
		Ok(port)
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//	pub fn get_port_number(&self) -> PortNumber { self.port_number }
	pub fn is_connected(&self) -> bool { self.is_connected.load(SeqCst) }
	pub fn set_connected(&self) { self.is_connected.store(true, SeqCst); }
	pub fn set_disconnected(&self) { self.is_connected.store(false, SeqCst); }
	pub fn is_broken(&self) -> bool { self.is_broken.load(SeqCst) }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn link_channel(&self, scope: &Scope, port_to_link: PortToLink, 
			port_from_link: PortFromLink, port_from_pe: PortFromPe) 
				-> Result<(),PortError> {
		let port = self.clone();
		scope.spawn( move || -> Result<(), PortError> {
			match port.listen_link(port_from_link) {
				Ok(_) => Ok(()),
				Err(err) => {
					println!("--- Port {}: listen_link {}", port.id, err);
					Err(err)
				}
			}
		});
		let port = self.clone();
		scope.spawn( move || -> Result<(), PortError> {
			match port.listen_pe(port_to_link, port_from_pe) {
				Ok(_) => Ok(()),
				Err(err) => {
					println!("--- Port {}: listen_pe {}", port.id, err);
					Err(err)
				}
			}
		});
		Ok(())
	}
	fn listen_link(&self, port_from_link: PortFromLink) -> Result<(),PortError> {
		let port_no = self.get_port_no();
		//println!("PortID {}: port_no {}", self.id, port_no);
		loop {
			//println!("Port {}: waiting for status or packet from link", port.id);
			let (opt_status, opt_packet) = port_from_link.recv()?;
			match opt_status {
				Some(status) => {
					match status {
						PortStatus::Connected => self.set_connected(),
						PortStatus::Disconnected => self.set_disconnected()
					};
					self.port_to_pe.send((Some((port_no,status)),None))?;
				},
				None => match opt_packet {
					Some(packet) => self.port_to_pe.send((None,Some((port_no,packet))))?,
					None => ()
				}
			};
		}
	}
	fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe) -> Result<(),PortError> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let packet = port_from_pe.recv()?;
			port_to_link.send(packet)?;
		}		
	}
}
impl fmt::Display for Port { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let is_connected = self.is_connected();
		let is_broken = self.is_broken();
		let mut s = format!("Port {} {}", self.port_number, self.id);
		if self.is_border { s = s + " is TCP  port,"; }
		else              { s = s + " is ECLP port,"; }
		if is_connected   { s = s + " is connected"; }
		else              { s = s + " is not connected"; }
		if is_broken      { s = s + " and is broken"; }
		else              { s = s + " and is not broken"; }
		write!(f, "{}", s) 
	}
}
// Errors
use std::error::Error;
use nalcell::{PortPeError, PortLinkError};
use name::NameError;
#[derive(Debug)]
pub enum PortError {
	Name(NameError),
	Channel(ChannelError),
	SendToPe(PortPeError),
	Send(PortLinkError),
	Recv(mpsc::RecvError)
}
impl Error for PortError {
	fn description(&self) -> &str {
		match *self {
			PortError::Name(ref err) => err.description(),
			PortError::Channel(ref err) => err.description(),
			PortError::SendToPe(ref err) => err.description(),
			PortError::Send(ref err) => err.description(),
			PortError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PortError::Name(ref err) => Some(err),
			PortError::Channel(ref err) => Some(err),
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
			PortError::SendToPe(ref err) => write!(f, "Port Send Packet Error caused by {}", err),
			PortError::Send(ref err) => write!(f, "Port Send Error caused by {}", err),
			PortError::Recv(ref err) => write!(f, "Port Receive Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct ChannelError { msg: String }
impl ChannelError { 
//	pub fn new() -> ChannelError {
//		ChannelError { msg: format!("No channel to link") }
//	}
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
impl From<PortLinkError> for PortError {
	fn from(err: PortLinkError) -> PortError { PortError::Send(err) }
}
impl From<PortPeError> for PortError {
	fn from(err: PortPeError) -> PortError { PortError::SendToPe(err) }
}
impl From<mpsc::RecvError> for PortError {
	fn from(err: mpsc::RecvError) -> PortError { PortError::Recv(err) }
}
