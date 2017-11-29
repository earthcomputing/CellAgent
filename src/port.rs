use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc};
use std::sync::atomic::Ordering::SeqCst;

use failure::Error;

use config::{PortNo, TableIndex};
use message_types::{PortToLink, PortFromLink, PortToPe, PortFromPe, LinkToPortPacket, PortToPePacket,
			  PortToNoc, PortFromNoc};
use name::{PortID, CellID};
use utility::{PortNumber, write_err};

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
impl Port {
	pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
			   port_to_pe: PortToPe) -> Result<Port, Error> {
		let port_id = PortID::new(cell_id, port_number)?;
		Ok(Port{ id: port_id, port_number: port_number, is_border: is_border, 
			is_connected: Arc::new(AtomicBool::new(is_connected)), 
			is_broken: Arc::new(AtomicBool::new(false)),
			port_to_pe: port_to_pe})
	}
	pub fn get_id(&self) -> &PortID { &self.id }
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//	pub fn get_port_number(&self) -> PortNumber { self.port_number }
	pub fn get_is_connected(&self) -> Arc<AtomicBool> { self.is_connected.clone() }
	pub fn is_connected(&self) -> bool { self.is_connected.load(SeqCst) }
	pub fn set_connected(&mut self) { self.is_connected.store(true, SeqCst); }
	pub fn set_disconnected(&mut self) { self.is_connected.store(false, SeqCst); }
	pub fn is_broken(&self) -> bool { self.is_broken.load(SeqCst) }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn outside_channel(&self, port_to_outside: PortToNoc, 
			port_from_outside: PortFromNoc, port_from_pe: PortFromPe) -> Result<(), Error> {
		self.port_to_pe.send(PortToPePacket::Status((self.get_port_no(), self.is_border, PortStatus::Connected)))?;
		let port_to_pe = self.port_to_pe.clone();
		let port_number = self.port_number;
		let id = self.id.clone();
		::std::thread::spawn( move || {
			let _ = Port::listen_outside_for_pe(port_number, port_to_pe, port_from_outside).map_err(|e| write_err("port", e));
		});
		let id = self.id.clone();
		::std::thread::spawn( move || {
			let _ = Port::listen_pe_for_outside(port_to_outside, port_from_pe).map_err(|e| write_err("port", e));
		});
		Ok(())
	}
	fn listen_outside_for_pe(port_number: PortNumber, port_to_pe: PortToPe, port_from_outside: PortFromNoc) -> Result<(), Error> {
		let other_index = TableIndex(0);
		loop {
			let packet = port_from_outside.recv()?;
			port_to_pe.send(PortToPePacket::Packet((port_number.get_port_no(), other_index, packet)))?;
		}
	}
	fn listen_pe_for_outside(port_to_noc: PortToNoc, port_from_pe: PortFromPe) -> Result<(), Error> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let (_, packet) = port_from_pe.recv()?;
			port_to_noc.send(packet)?;
		}		
	}
	pub fn link_channel(&self, port_to_link: PortToLink, port_from_link: PortFromLink, port_from_pe: PortFromPe) {
		let mut port = self.clone();
		::std::thread::spawn( move || {
			let _ = port.listen_link(port_from_link).map_err(|e| write_err("port", e));
		});
		let port = self.clone();
		::std::thread::spawn( move || {
			let _ = port.listen_pe(port_to_link, port_from_pe).map_err(|e| write_err("port", e));
		});
	}
	fn listen_link(&mut self, port_from_link: PortFromLink) -> Result<(), Error> {
		//println!("PortID {}: port_no {}", self.id, port_no);
		loop {
			//println!("Port {}: waiting for status or packet from link", port.id);
			match port_from_link.recv()? {
				LinkToPortPacket::Status(status) => {
					match status {
						PortStatus::Connected => self.set_connected(),
						PortStatus::Disconnected => self.set_disconnected()
					};
					self.port_to_pe.send(PortToPePacket::Status((self.port_number.get_port_no(), self.is_border, status)))?;
				}
				LinkToPortPacket::Packet((my_index, packet)) => {
					//println!("Port {}: got from link {}", self.id, packet);
					self.port_to_pe.send(PortToPePacket::Packet((self.port_number.get_port_no(), my_index, packet)))?;
					//println!("Port {}: sent from link to pe {}", self.id, packet);
				}
			}
		}
	}
	fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe) -> Result<(), Error> {
		loop {
			//println!("Port {}: waiting for packet from pe", id);
			let packet = port_from_pe.recv()?;
			//println!("Port {}: got from pe {}", id, packet);
			port_to_link.send(packet)?;
			//println!("Port {}: sent from pe to link {}", id, packet);
		}		
	}
}
impl fmt::Display for Port { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let is_connected = self.is_connected();
		let is_broken = self.is_broken();
		let mut s = format!("Port {} {}", self.port_number, self.id);
		if self.is_border { s = s + " is boundary  port,"; }
		else              { s = s + " is ECLP port,"; }
		if is_connected   { s = s + " is connected"; }
		else              { s = s + " is not connected"; }
		if is_broken      { s = s + " and is broken"; }
		else              { s = s + " and is not broken"; }
		write!(f, "{}", s) 
	}
}
// Errors
