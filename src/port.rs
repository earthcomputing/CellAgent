use std::fmt;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::sync::atomic::Ordering::SeqCst;
use crossbeam::{Scope, ScopedJoinHandle};
use serde;
use serde_json;
use config::{PortNo, TableIndex, Uniquifier};
use message::OutsideMsg;
use message_types::{PortToLink, PortFromLink, PortToPe, PortFromPe, LinkToPortMsg, PortToPeMsg,
			  PortToOutside, PortFromOutside, PortToOutsideMsg};
use name::{Name, PortID, CellID};
use packet::{PacketAssembler, Packetizer};
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
	packet_assemblers: HashMap<Uniquifier, PacketAssembler>,
}
impl Port {
	pub fn new(cell_id: &CellID, port_number: PortNumber, is_border: bool, is_connected: bool,
			   port_to_pe: PortToPe) -> Result<Port> {
		let port_id = PortID::new(port_number.get_port_no())?;
		let temp_id = port_id.add_component(&cell_id.get_name())?;
		Ok(Port{ id: temp_id, port_number: port_number, is_border: is_border, 
			is_connected: Arc::new(AtomicBool::new(is_connected)), 
			is_broken: Arc::new(AtomicBool::new(false)), packet_assemblers: HashMap::new(),
			port_to_pe: port_to_pe})
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
//	pub fn get_port_number(&self) -> PortNumber { self.port_number }
	pub fn is_connected(&self) -> bool { self.is_connected.load(SeqCst) }
	pub fn set_connected(&self) { self.is_connected.store(true, SeqCst); }
	pub fn set_disconnected(&self) { self.is_connected.store(false, SeqCst); }
	pub fn is_broken(&self) -> bool { self.is_broken.load(SeqCst) }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn outside_channel(&self, scope: &Scope, port_to_outside: PortToOutside, 
			port_from_outside: PortFromOutside, port_from_pe: PortFromPe) 
			-> Result<(ScopedJoinHandle<()>)> {
		let port = self.clone();
		let outside_handle = scope.spawn( move || {
			let _ = port.listen_outside(port_from_outside).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		let mut port = self.clone();
		let pe_handle = scope.spawn( move || {
			let _ = port.listen_pe_for_outside(port_to_outside, port_from_pe).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		Ok((outside_handle))
	}
	fn listen_outside(&self, port_from_outside: PortFromOutside) -> Result<()> {
		let port = self.clone();
		let other_index = 0 as TableIndex;
		loop {
			let json_msg = port_from_outside.recv().chain_err(|| "Receive from outside")?;
			let msg = OutsideMsg::new(&json_msg);
			let packets = Packetizer::packetize(&msg, other_index)?;
			println!("Port {}: msg from outside {}", port.id, msg);
			for packet in packets {
				self.port_to_pe.send(PortToPeMsg::Msg((port.port_number.get_port_no(), *packet))).chain_err(|| ErrorKind::PortError)?;
			}
		}
	}
	fn listen_pe_for_outside(&mut self, port_to_outside: PortToOutside, port_from_pe: PortFromPe) -> Result<()> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let packet = port_from_pe.recv().chain_err(|| "Receive from pe for outside")?;
			if let Some(packets) = Packetizer::process_packet(&mut self.packet_assemblers, packet) {
				let msg = Packetizer::unpacketize(packets).chain_err(|| ErrorKind::PortError)?;
				let json_msg = serde_json::to_string(&msg).chain_err(|| ErrorKind::PortError)?;
				port_to_outside.send(json_msg).chain_err(|| ErrorKind::PortError)?;
			}
		}		
	}
	pub fn link_channel(&self, scope: &Scope, port_to_link: PortToLink, 
			port_from_link: PortFromLink, port_from_pe: PortFromPe) 
				-> Result<(ScopedJoinHandle<()>,ScopedJoinHandle<()>)> {
		let port = self.clone();
		let link_handle = scope.spawn( move || {
			let _ = port.listen_link(port_from_link).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		let port = self.clone();
		let pe_handle = scope.spawn( move || {
			let _ = port.listen_pe(port_to_link, port_from_pe).chain_err(|| ErrorKind::PortError).map_err(|e| port.write_err(e));
		});
		Ok((link_handle, pe_handle))
	}
	fn listen_link(&self, port_from_link: PortFromLink) -> Result<()> {
		let port_no = self.get_port_no();
		//println!("PortID {}: port_no {}", self.id, port_no);
		loop {
			//println!("Port {}: waiting for status or packet from link", port.id);
			match port_from_link.recv().chain_err(|| ErrorKind::PortError)? {
				LinkToPortMsg::Status(status) => {
					match status {
						PortStatus::Connected => self.set_connected(),
						PortStatus::Disconnected => self.set_disconnected()
					};
					self.port_to_pe.send(PortToPeMsg::Status((port_no, status))).chain_err(|| ErrorKind::PortError)?;
				}
				LinkToPortMsg::Msg(msg) => {
					self.port_to_pe.send(PortToPeMsg::Msg((port_no, msg))).chain_err(|| ErrorKind::PortError)?;
				}
			}
		}
	}
	fn listen_pe(&self, port_to_link: PortToLink, port_from_pe: PortFromPe) -> Result<()> {
		loop {
			//println!("Port {}: waiting for packet from pe", port.id);
			let packet = port_from_pe.recv()?;
			port_to_link.send(packet).chain_err(|| ErrorKind::PortError)?;
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
