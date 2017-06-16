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
			   port_to_pe: PortToPe) -> Result<Port>{
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
			port_from_link: PortFromLink, port_from_pe: PortFromPe) -> Result<()> {
		let port = self.clone();
		scope.spawn( move || -> Result<()> {
			match port.listen_link(port_from_link) {
				Ok(_) => Ok(()),
				Err(err) => {
					println!("--- Port {}: listen_link {}", port.id, err);
					Err(err)
				}
			}
		});
		let port = self.clone();
		scope.spawn( move || -> Result<()> {
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
	fn listen_link(&self, port_from_link: PortFromLink) -> Result<()> {
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
	fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe) -> Result<()> {
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
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		PortToLink(::nalcell::PortLinkError);
		PortToPe(::nalcell::PortPeError);
	}
	links {
		Name(::name::Error, ::name::ErrorKind);
	}
}
