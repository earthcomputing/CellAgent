use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use std::{thread, time};
use crossbeam::Scope;
use cellagent::{CellAgent, CellAgentError};
use config::MAX_PORTS;
use name::{CellID, PortID};
use packet::Packet;
use packet_engine::{PacketEngine};
use port::{Port, PortStatus, PortError};
use routing_table_entry::RoutingTableEntry;
use vm::VirtualMachine;

// Packet from CellAgent to PacketEngine, (table index, mask, packet)
pub type PacketCaToPe = mpsc::Sender<(u32, u16, Packet)>;
pub type PacketPeFromCa = mpsc::Receiver<(u32, u16, Packet)>;
pub type PacketCaPeSendError = mpsc::SendError<(u32, u16, Packet)>;
// Packet from PacketEngine to CellAgent, (port_no, packet)
pub type PacketPeToCa = mpsc::Sender<(u8, Packet)>;
pub type PacketCaFromPe = mpsc::Receiver<(u8, Packet)>;
pub type PacketPeCaSendError = mpsc::SendError<(u8, Packet)>;
// Table entry from CellAgent to PacketEngine, table entry
pub type EntryCaToPe = mpsc::Sender<RoutingTableEntry>;
pub type EntryPeFromCa = mpsc::Receiver<RoutingTableEntry>;
pub type EntrySendError = mpsc::SendError<RoutingTableEntry>;
// Port status from Port to CellAgent, (port_no, status)
pub type StatusPortToCa = mpsc::Sender<(u8, PortStatus)>;
pub type StatusCaFromPort = mpsc::Receiver<(u8, PortStatus)>;
pub type PortStatusSendError = mpsc::SendError<(u8, PortStatus)>;
// Packet from PacketEngine to Port, Port to Link, Link to Port, Port to PacketEngine, packet
pub type SendPacket = mpsc::Sender<Packet>;
pub type RecvPacket = mpsc::Receiver<Packet>;
pub type SendPacketError = mpsc::SendError<Packet>;

#[derive(Debug)]
pub struct NalCell {
	id: CellID,
	cell_no: usize,
	is_border: bool,
	cell_agent: CellAgent,
	packet_engine: PacketEngine,
	vms: Vec<VirtualMachine>,
}
impl NalCell {
	pub fn new(scope: &Scope, cell_no: usize, nports: u8, is_border: bool) -> Result<NalCell,NalCellError> {
		if nports > MAX_PORTS { return Err(NalCellError::NumberPorts(NumberPortsError::new(nports))) }
		let cell_id = try!(CellID::new(cell_no));
		let (entry_ca_to_pe, entry_pe_from_ca): (EntryCaToPe, EntryPeFromCa) = channel();
		let (packet_ca_to_pe, packet_pe_from_ca): (PacketCaToPe, PacketPeFromCa) = channel();
		let (packet_pe_to_ca, packet_ca_from_pe): (PacketPeToCa, PacketCaFromPe) = channel();
		let (port_to_pe, pe_from_port): (SendPacket, RecvPacket) = channel();
		let (status_port_to_ca, status_ca_from_port): (StatusPortToCa, StatusCaFromPort) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut is_connected = true;
		for i in 0..nports + 1 {
			let is_border_port;
			if is_border & (i == 2) { is_border_port = true; }
			else                    { is_border_port = false; }
			let (pe_to_port, pe_from_port): (SendPacket, RecvPacket) = channel();
			pe_to_ports.push(pe_to_port);
			let port = try!(Port::new(&scope, &cell_id, PortNumber { port_no: i as u8 }, is_border_port, 
					is_connected, port_to_pe.clone(), status_port_to_ca.clone()));
			ports.push(port);
			is_connected = false;
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		let cell_agent = try!(CellAgent::new(scope, &cell_id, boxed_ports, port_to_pe, 
				packet_ca_to_pe, packet_ca_from_pe, entry_ca_to_pe, status_ca_from_port));
		let packet_engine = try!(PacketEngine::new(scope, &cell_id, packet_pe_to_ca, packet_pe_from_ca, pe_from_port,
								pe_to_ports, entry_pe_from_ca));
		let nalcell = NalCell { id: cell_id, cell_no: cell_no, is_border: is_border,
				cell_agent: cell_agent, packet_engine: packet_engine, vms: Vec::new()};
		Ok(nalcell)
	}
	pub fn get_id(&self) -> CellID { self.id.clone() }
	pub fn get_no(&self) -> usize { self.cell_no }
	pub fn get_free_port_mut (&mut self) -> Result<&mut Port,NalCellError> {
		Ok(try!(self.cell_agent.get_free_port_mut()))
	}
	pub fn stringify(&self) -> String {
		let mut s = String::new();
		if self.is_border { s = s + &format!("Border Cell {}", self.id); }
		else              { s = s + &format!("Cell {}", self.id); }

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
	NumberPorts(NumberPortsError)
}
impl Error for NalCellError {
	fn description(&self) -> &str {
		match *self {
			NalCellError::Name(ref err) => err.description(),
			NalCellError::Port(ref err) => err.description(),
			NalCellError::CellAgent(ref err) => err.description(),
			NalCellError::NumberPorts(ref err) => err.description(),
			NalCellError::PacketEngine(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
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
impl From<PortError> for NalCellError {
	fn from(err: PortError) -> NalCellError { NalCellError::Port(err) }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct PortNumber { pub port_no: u8 }
impl PortNumber {
	pub fn new(no: u8, no_ports: u8) -> Result<PortNumber, PortNumberError> {
		if no > no_ports {
			Err(PortNumberError::new(no, no_ports))
		} else {
			Ok(PortNumber { port_no: (no as u8) })
		}
	}
	pub fn get_port_no(&self) -> u8 { self.port_no }
}
impl fmt::Display for PortNumber {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_no) }
}
#[derive(Debug)]
pub struct PortNumberError { msg: String }
impl PortNumberError {
	pub fn new(port_no: u8, no_ports: u8) -> PortNumberError {
		let msg = format!("You asked for port number {}, but this cell only has {} ports",
			port_no, no_ports);
		PortNumberError { msg: msg }
	}
}
impl Error for PortNumberError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortNumberError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.msg) }
}
impl From<NumberPortsError> for NalCellError {
	fn from(err: NumberPortsError) -> NalCellError { NalCellError::NumberPorts(err) }
}
