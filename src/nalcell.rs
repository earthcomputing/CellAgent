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
pub type CaPeError = mpsc::SendError<CaToPeMsg>;
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
	packet_engine: PacketEngine,
	vms: Vec<VirtualMachine>,
	ports_from_pe: HashMap<PortNo,PortFromPe>
}

impl NalCell {
	pub fn new(scope: &Scope, cell_no: CellNo, nports: PortNo, is_border: bool) -> Result<NalCell> {
		if nports > MAX_PORTS { return Err(ErrorKind::NumberPorts(nports).into()) }
		let cell_id = CellID::new(cell_no)?;
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
				is_connected, port_to_pe.clone()).chain_err(|| ErrorKind::NalCellError)?;
			ports.push(port);
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		let packet_engine = PacketEngine::new(&cell_id, pe_to_ca, pe_to_ports).chain_err(|| ErrorKind::NalCellError)?;
		packet_engine.start_threads(scope, pe_from_ca, pe_from_ports)?;
		let mut cell_agent = CellAgent::new(&cell_id, boxed_ports.len() as u8, ca_to_pe).chain_err(|| ErrorKind::NalCellError)?;
		cell_agent.initialize(scope, ca_from_pe)?;
		Ok(NalCell { id: cell_id, cell_no: cell_no, is_border: is_border,
				ports: boxed_ports, cell_agent: cell_agent, vms: Vec::new(),
				packet_engine: packet_engine, ports_from_pe: ports_from_pe})
	}
//	pub fn get_id(&self) -> CellID { self.id.clone() }
//	pub fn get_no(&self) -> usize { self.cell_no }
//	pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
	pub fn get_free_port_mut (&mut self) -> Result<(&mut Port, PortFromPe)> {
		for p in &mut self.ports.iter_mut() {
			//println!("NalCell {}: port {} is connected {}", self.id, p.get_port_no(), p.is_connected());
			if !p.is_connected() & !p.is_border() & (p.get_port_no() != 0 as u8) {
				let port_no = p.get_port_no();
				match self.ports_from_pe.remove(&port_no) { // Remove avoids a borrowed context error
					Some(recvr) => {
						p.set_connected();
						return Ok((p,recvr))
					},
					None => return Err(ErrorKind::Channel(port_no).into())
				} 
			}
		}
		Err(ErrorKind::NoFreePorts(self.id.clone()).into())
	}
}
impl fmt::Display for NalCell { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = String::new();
		if self.is_border { s = s + &format!("Border Cell {}", self.id); }
		else              { s = s + &format!("Cell {}", self.id); }

		s = s + &format!("{}", self.cell_agent);
		s = s + &format!("\n{}", self.packet_engine);
		write!(f, "{}", s) }
}
// Errors
error_chain! {
	links {
		CellAgent(::cellagent::Error, ::cellagent::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		PacketEngine(::packet_engine::Error, ::packet_engine::ErrorKind);
		Port(::port::Error, ::port::ErrorKind);
	}
	errors { NalCellError
		Channel(port_no: PortNo) {
			description("No receiver for port")
			display("No receiver for port {}", port_no)
		}
		NoFreePorts(cell_id: CellID) {
			description("All ports have been assigned")
			display("All ports have been assigned for cell {}", cell_id)
		}
		NumberPorts(nports: PortNo) {
			description("You are asking for too many ports.")
			display("You asked for {} ports, but only {} are allowed", nports, MAX_PORTS)
		}
	}
}
