use std::fmt;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use crossbeam::Scope;
use cellagent::{CellAgent};
use config::{MAX_PORTS, CellNo, PortNo, TableIndex};
use name::{CellID};
use packet::Packet;
use packet_engine::{PacketEngine};
use port::{Port, PortStatus};
use routing_table_entry::RoutingTableEntry;
use utility::{Mask, PortNumber};
use vm::VirtualMachine;

// CellAgent to PacketEngine
pub type CaToPeMsg = (Option<RoutingTableEntry>,Option<(TableIndex,Mask,Packet)>);
pub type CaToPe = mpsc::Sender<CaToPeMsg>;
pub type PeFromCa = mpsc::Receiver<CaToPeMsg>;
//pub type CaPeError = mpsc::SendError<CaToPeMsg>;
// PacketEngine to Port
pub type PeToPort = mpsc::Sender<Packet>;
pub type PortFromPe = mpsc::Receiver<Packet>;
pub type PePortError = mpsc::SendError<Packet>;
// PacketEngine to Port, Port to Link
pub type PortToLink = mpsc::Sender<Packet>;
pub type LinkFromPort = mpsc::Receiver<Packet>;
pub type PortLinkError = mpsc::SendError<Packet>;
// Link to Port
pub type LinkToPortMsg = (Option<PortStatus>,Option<Packet>);
pub type LinkToPort = mpsc::Sender<LinkToPortMsg>;
pub type PortFromLink = mpsc::Receiver<LinkToPortMsg>;
pub type LinkPortError = mpsc::SendError<LinkToPortMsg>;
// Port to PacketEngine
pub type PortToPeMsg = (Option<(PortNo, PortStatus)>,Option<(PortNo, Packet)>);
pub type PortToPe = mpsc::Sender<PortToPeMsg>;
pub type PeFromPort = mpsc::Receiver<PortToPeMsg>;
pub type PortPeError = mpsc::SendError<PortToPeMsg>;
// PacketEngine to CellAgent
pub type PeToCaMsg = (Option<(PortNo, PortStatus)>,Option<(PortNo, TableIndex, Packet)>);
pub type PeToCa = mpsc::Sender<PeToCaMsg>;
pub type CaFromPe = mpsc::Receiver<PeToCaMsg>;
pub type PeCaError = mpsc::SendError<PeToCaMsg>;

#[derive(Debug)]
pub struct NalCell { // Does not include PacketEngine so CellAgent can own it
	id: CellID,
	cell_no: usize,
	is_border: bool,
	ports: Box<[Port]>,
	cell_agent: CellAgent,
	vms: Vec<VirtualMachine>,
	ports_from_pe: HashMap<PortNo,PortFromPe>
}
#[deny(unused_must_use)]
impl NalCell {
	pub fn new(scope: &Scope, cell_no: CellNo, nports: PortNo, is_border: bool) -> Result<NalCell,NalCellError> {
		if nports > MAX_PORTS { return Err(NalCellError::NumberPorts(NumberPortsError::new(nports))) }
		let cell_id = try!(CellID::new(cell_no));
		let (ca_to_pe, pe_from_ca): (CaToPe, PeFromCa) = channel();
		let (pe_to_ca, ca_from_pe): (PeToCa, CaFromPe) = channel();
		let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut ports_from_pe = HashMap::new(); // So I can remove the item
		for i in 0..nports + 1 {
			let is_border_port;
			if is_border & (i == 2) { is_border_port = true; }
			else                    { is_border_port = false; }
			let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
			pe_to_ports.push(pe_to_port);
			ports_from_pe.insert(i, port_from_pe);
			let is_connected = if i == 0 { true } else { false };
			let port = Port::new(&cell_id, PortNumber { port_no: i as u8 }, is_border_port, 
				is_connected, port_to_pe.clone())?;
			ports.push(port);
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		PacketEngine::new(scope, &cell_id, pe_to_ca,
				pe_from_ca, pe_from_ports, pe_to_ports)?;
		let cell_agent = CellAgent::new(scope, &cell_id, boxed_ports.len() as u8, 
			ca_from_pe, ca_to_pe)?;
		let nalcell = NalCell { id: cell_id, cell_no: cell_no, is_border: is_border,
				ports: boxed_ports, cell_agent: cell_agent, vms: Vec::new(),
				ports_from_pe: ports_from_pe};
		Ok(nalcell)
	}
//	pub fn get_id(&self) -> CellID { self.id.clone() }
//	pub fn get_no(&self) -> usize { self.cell_no }
	pub fn get_free_port_mut (&mut self) -> Result<(&mut Port, PortFromPe), NalCellError> {
		for p in &mut self.ports.iter_mut() {
			//println!("NalCell {}: port {} is connected {}", self.id, p.get_port_no(), p.is_connected());
			if !p.is_connected() & !p.is_border() & (p.get_port_no() != 0 as u8) {
				let port_no = p.get_port_no();
				match self.ports_from_pe.remove(&port_no) { // Remove avoids a borrowed context error
					Some(recvr) => {
						p.set_connected();
						return Ok((p,recvr))
					},
					None => return Err(NalCellError::Channel(ChannelError::new(port_no)))
				} 
			}
		}
		Err(NalCellError::NoFreePort(NoFreePortError::new(self.id.clone())))
	}
}
impl fmt::Display for NalCell { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = String::new();
		if self.is_border { s = s + &format!("Border Cell {}", self.id); }
		else              { s = s + &format!("Cell {}", self.id); }

		s = s + &format!("{}",self.cell_agent);
		write!(f, "{}", s) }
}
// Errors
use std::error::Error;
use cellagent::CellAgentError;
use name::NameError;
use packet_engine::PacketEngineError;
use port::PortError;
#[derive(Debug)]
pub enum NalCellError {
	Name(NameError),
	Port(PortError),
	NoFreePort(NoFreePortError),
	CellAgent(CellAgentError),
	PacketEngine(PacketEngineError),
	NumberPorts(NumberPortsError),
	Channel(ChannelError)
}
impl Error for NalCellError {
	fn description(&self) -> &str {
		match *self {
			NalCellError::Name(ref err) => err.description(),
			NalCellError::Port(ref err) => err.description(),
			NalCellError::NoFreePort(ref err) => err.description(),
			NalCellError::CellAgent(ref err) => err.description(),
			NalCellError::NumberPorts(ref err) => err.description(),
			NalCellError::PacketEngine(ref err) => err.description(),
			NalCellError::Channel(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			NalCellError::Name(ref err) => Some(err),
			NalCellError::Port(ref err) => Some(err),
			NalCellError::NoFreePort(ref err) => Some(err),
			NalCellError::CellAgent(ref err) => Some(err),
			NalCellError::NumberPorts(ref err) => Some(err),
			NalCellError::PacketEngine(ref err) => Some(err),
			NalCellError::Channel(ref err) => Some(err),
		}
	}
}
impl fmt::Display for NalCellError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NalCellError::Name(ref err) => write!(f, "NalCell Name Error caused by {}", err),
			NalCellError::Port(ref err) => write!(f, "NalCell Port Error caused by {}", err),
			NalCellError::NoFreePort(ref err) => write!(f, "NalCell No Free Port Error caused by {}", err),
			NalCellError::CellAgent(ref err) => write!(f, "NalCell Cell Agent Error caused by {}", err),
			NalCellError::NumberPorts(ref err) => write!(f, "NalCell Number Ports Error caused by {}", err),
			NalCellError::PacketEngine(ref err) => write!(f, "NalCell Number Ports Error caused by {}", err),
			NalCellError::Channel(ref err) => write!(f, "NalCell Channel Error caused by {}", err),
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
	pub fn new(nports: PortNo) -> NumberPortsError {
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
#[derive(Debug)]
pub struct ChannelError { msg: String }
impl ChannelError { 
	pub fn new(port_no: PortNo) -> ChannelError {
		ChannelError { msg: format!("No receiver for port {}", port_no) }
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
impl From<PortError> for NalCellError {
	fn from(err: PortError) -> NalCellError { NalCellError::Port(err) }
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
impl From<NumberPortsError> for NalCellError {
	fn from(err: NumberPortsError) -> NalCellError { NalCellError::NumberPorts(err) }
}
