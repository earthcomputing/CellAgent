use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use std::{thread, time};
use crossbeam::Scope;
use cellagent::{CellAgent, CellAgentError};
use config::MAX_PORTS;
use message::{Sender, Receiver};
use name::{CellID, PortID};
use packet_engine::PacketEngine;
use port::{Port, PortStatus, PortError};
use routing_table_entry::RoutingTableEntry;
use vm::VirtualMachine;

pub type EntrySender = mpsc::Sender<RoutingTableEntry>;
pub type EntryReceiver = mpsc::Receiver<RoutingTableEntry>;
pub type PortStatusSender = mpsc::Sender<(u8,PortStatus)>;
pub type PortStatusReceiver = mpsc::Receiver<(u8,PortStatus)>;
pub type PortStatusSenderError = mpsc::SendError<(u8,PortStatus)>;

#[derive(Debug)]
pub struct NalCell {
	id: CellID,
	cell_no: usize,
	is_border: bool,
	ports: Box<[Port]>,
	cell_agent: CellAgent,
	packet_engine: PacketEngine,
	vms: Vec<VirtualMachine>,
}
impl NalCell {
	pub fn new(scope: &Scope, cell_no: usize, nports: u8, is_border: bool) -> Result<NalCell,NalCellError> {
		if nports > MAX_PORTS { return Err(NalCellError::NumberPorts(NumberPortsError::new(nports))) }
		let cell_id = try!(CellID::new(cell_no));
		let (ca_entry_to_pe, pe_entry_from_ca): (EntrySender, EntryReceiver) = channel();
		let (ca_to_pe, pe_from_ca): (Sender, Receiver) = channel();
		let (pe_to_ca, ca_from_pe): (Sender, Receiver) = channel();
		let (port_to_pe, pe_from_port): (Sender, Receiver) = channel();
		let (port_to_ca, ca_from_port): (PortStatusSender, PortStatusReceiver) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut is_connected = true;
		for i in 0..nports + 1 {
			let mut is_border_port;
			if is_border & (i == 2) { is_border_port = true; }
			else                    { is_border_port = false; }
			let (pe_to_port, port_from_pe): (Sender, Receiver) = channel();
			pe_to_ports.push(pe_to_port);
			let port = try!(Port::new(&cell_id, i as u8, is_border_port, is_connected,
						port_to_pe.clone(), port_from_pe, port_to_ca.clone()));
			ports.push(port);
			is_connected = false;
		}
		let boxed: Box<[Port]> = ports.into_boxed_slice(); 
		let cell_agent = try!(CellAgent::new(scope, &cell_id, ca_to_pe, ca_from_pe,
								ca_entry_to_pe, ca_from_port));
		let packet_engine = try!(PacketEngine::new(scope, &cell_id, pe_to_ca, pe_from_ca, pe_from_port,
								pe_to_ports, pe_entry_from_ca));
		let nalcell = NalCell { id: cell_id, cell_no: cell_no, ports: boxed, is_border: is_border,
				cell_agent: cell_agent, packet_engine: packet_engine, vms: Vec::new()};
		Ok(nalcell)
	}
	pub fn get_id(&self) -> CellID { self.id.clone() }
	pub fn get_no(&self) -> usize { self.cell_no }
	pub fn get_port(&mut self, index: u8) -> &mut Port { &mut self.ports[index as usize] }
	pub fn get_free_port_mut (&mut self) -> Result<&mut Port,NalCellError> {
		for p in &mut self.ports.iter_mut() {
			if !p.is_connected() & !p.is_border() { return Ok(p); }
		}
		Err(NalCellError::NoFreePort(NoFreePortError::new(self.id.clone())))
	}
	pub fn stringify(&self) -> String {
		let mut s = String::new();
		if self.is_border { s = s + &format!("Border Cell {}", self.id); }
		else              { s = s + &format!("Cell {}", self.id); }
		for p in &mut self.ports.iter() {
			if p.get_no() < 4 { s = s + "\n" + &format!("{}", p); }
		}
		s = s + &format!("{}",self.cell_agent.stringify());
		s = s + &self.packet_engine.stringify();
		s
	}
}
impl fmt::Display for NalCell { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = self.stringify();
		write!(f, "{}", s) }
}
// Errors
use std::error::Error;
use name::NameError;
use packet_engine::PacketEngineError;
#[derive(Debug)]
pub enum NalCellError {
	Name(NameError),
	Port(PortError),
	CellAgent(CellAgentError),
	PacketEngine(PacketEngineError),
	NoFreePort(NoFreePortError),
	NumberPorts(NumberPortsError)
}
impl Error for NalCellError {
	fn description(&self) -> &str {
		match *self {
			NalCellError::NoFreePort(ref err) => err.description(),
			NalCellError::Name(ref err) => err.description(),
			NalCellError::Port(ref err) => err.description(),
			NalCellError::CellAgent(ref err) => err.description(),
			NalCellError::NumberPorts(ref err) => err.description(),
			NalCellError::PacketEngine(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			NalCellError::NoFreePort(_) => None,
			NalCellError::Name(ref err) => Some(err),
			NalCellError::Port(ref err) => Some(err),
			NalCellError::CellAgent(ref err) => Some(err),
			NalCellError::NumberPorts(ref err) => Some(err),
			NalCellError::PacketEngine(ref err) => Some(err),
		}
	}
}
impl fmt::Display for NalCellError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NalCellError::NoFreePort(ref err) => write!(f, "NalCell NoFreePort Error caused by {}", err),
			NalCellError::Name(ref err) => write!(f, "NalCell Name Error caused by {}", err),
			NalCellError::Port(ref err) => write!(f, "NalCell Port Error caused by {}", err),
			NalCellError::CellAgent(ref err) => write!(f, "NalCell Cell Agent Error caused by {}", err),
			NalCellError::NumberPorts(ref err) => write!(f, "NalCell Number Ports Error caused by {}", err),
			NalCellError::PacketEngine(ref err) => write!(f, "NalCell Number Ports Error caused by {}", err),
		}
	}
}
impl From<NameError> for NalCellError {
	fn from(err: NameError) -> NalCellError { NalCellError::Name(err) }
}
impl From<CellAgentError> for NalCellError {
	fn from(err: CellAgentError) -> NalCellError { NalCellError::CellAgent(err) }
}
impl From<PacketEngineError> for NalCellError {
	fn from(err: PacketEngineError) -> NalCellError { NalCellError::PacketEngine(err) }
}
#[derive(Debug)]
pub struct NoFreePortError { msg: String }
impl NoFreePortError { 
	pub fn new(cell_id: CellID) -> NoFreePortError {
		NoFreePortError { msg: format!("All ports have been assigned for cell {}", cell_id) }
	}
}
impl Error for NoFreePortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NoFreePortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NoFreePortError> for NalCellError {
	fn from(err: NoFreePortError) -> NalCellError { NalCellError::NoFreePort(err) }
}
#[derive(Debug)]
pub struct NumberPortsError { msg: String }
impl NumberPortsError { 
	pub fn new(nports: u8) -> NumberPortsError {
		NumberPortsError { msg: format!("You asked for {} ports, but only {} are allowed", nports, MAX_PORTS) }
	}
}
impl Error for NumberPortsError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NumberPortsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NumberPortsError> for NalCellError {
	fn from(err: NumberPortsError) -> NalCellError { NalCellError::NumberPorts(err) }
}
impl From<PortError> for NalCellError {
	fn from(err: PortError) -> NalCellError { NalCellError::Port(err) }
}
