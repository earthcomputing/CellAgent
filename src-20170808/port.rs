use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc};
use std::sync::atomic::Ordering::SeqCst;

use config::{PortNo, TableIndex};
use message_types::{PortToLink, PortFromLink, PortToPe, PortFromPe, LinkToPortPacket, PortToPePacket,
			  PortToNoc, PortFromNoc};
use name::{PortID, CellID};
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
impl Port {
	pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
			   port_to_pe: PortToPe) -> Result<Port> {
		let port_id = PortID::new(cell_id, port_number)?;
		Ok(Port{ id: port_id, port_number: port_number, is_border: is_border, 
			is_connected: Arc::new(AtomicBool::new(is_connected)), 
			is_broken: Arc::new(AtomicBool::new(false)),
			port_to_pe: port_to_pe})
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
	pub fn get_port_number(&self) -> PortNumber { self.port_number }
	pub fn is_connected(&self) -> bool { self.is_connected.load(SeqCst) }
	pub fn set_connected(&self) { self.is_connected.store(true, SeqCst); }
	pub fn set_disconnected(&self) { self.is_connected.store(false, SeqCst); }
	pub fn is_broken(&self) -> bool { self.is_broken.load(SeqCst) }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn outside_channel(&self, port_to_outside: PortToNoc, 
			port_from_outside: PortFromNoc, port_from_pe: PortFromPe) -> Result<()> {
		let port = self.clone();
		self.port_to_pe.send(PortToPePacket::Status((self.get_port_no(), self.is_border, PortStatus::Connected))).chain_err(|| ErrorKind::PortError)?;
		let outside_handle = ::std::thread::spawn( move || {
			let _ = port.listen_outside_for_pe(port_from_outside).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		let mut port = self.clone();
		let pe_handle = ::std::thread::spawn( move || {
			let _ = port.listen_pe_for_outside(port_to_outside, port_from_pe).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		Ok(())
	}
	fn listen_outside_for_pe(&self, port_from_outside: PortFromNoc) -> Result<()> {
		let port = self.clone();
		let other_index = 0 as TableIndex;
		loop {
			let packet = port_from_outside.recv().chain_err(|| "Receive from outside")?;
			self.port_to_pe.send(PortToPePacket::Packet((port.port_number.get_port_no(), other_index, packet))).chain_err(|| ErrorKind::PortError)?;
		}
	}
	fn listen_pe_for_outside(&mut self, port_to_noc: PortToNoc, port_from_pe: PortFromPe) -> Result<()> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let (_, packet) = port_from_pe.recv().chain_err(|| "Receive from pe for outside")?;
			port_to_noc.send(packet).chain_err(|| ErrorKind::PortError)?;
		}		
	}
	pub fn link_channel(&self, port_to_link: PortToLink, 
			port_from_link: PortFromLink, port_from_pe: PortFromPe) 
				-> Result<()> {
		let port = self.clone();
		let link_handle = ::std::thread::spawn( move || {
			let _ = port.listen_link(port_from_link).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		let port = self.clone();
		let pe_handle = ::std::thread::spawn( move || {
			let _ = port.listen_pe(port_to_link, port_from_pe).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		Ok(())
	}
	fn listen_link(&self, port_from_link: PortFromLink) -> Result<()> {
		let port_no = self.get_port_no();
		//println!("PortID {}: port_no {}", self.id, port_no);
		loop {
			//println!("Port {}: waiting for status or packet from link", port.id);
			match port_from_link.recv().chain_err(|| ErrorKind::PortError)? {
				LinkToPortPacket::Status(status) => {
					match status {
						PortStatus::Connected => self.set_connected(),
						PortStatus::Disconnected => self.set_disconnected()
					};
					self.port_to_pe.send(PortToPePacket::Status((port_no, self.is_border, status))).chain_err(|| ErrorKind::PortError)?;
				}
				LinkToPortPacket::Packet((my_index, packet)) => {
					//println!("Port {}: got from link {}", self.id, packet);
					self.port_to_pe.send(PortToPePacket::Packet((port_no, my_index, packet))).chain_err(|| ErrorKind::PortError)?;
					//println!("Port {}: sent from link to pe {}", self.id, packet);
				}
			}
		}
	}
	fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe) -> Result<()> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let packet = port_from_pe.recv()?;
			//println!("Port {}: got from pe {}", self.id, packet);
			port_to_link.send(packet).chain_err(|| ErrorKind::PortError)?;
			//println!("Port {}: sent from pe to link {}", self.id, packet);
		}		
	}
	fn write_err(&self, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Port {}: {}", self.id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
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
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		PortToLink(::message_types::PortLinkError);
		PortToPe(::message_types::PortPeError);
	}
	links {
		Name(::name::Error, ::name::ErrorKind);
		Packet(::packet::Error, ::packet::ErrorKind);
	}
	errors { PortError
	}
}