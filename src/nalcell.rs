use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::mpsc::channel;

use cellagent::{CellAgent};
use config::{MAX_PORTS, CellNo, Json, PortNo, TableIndex};
use message_types::{CaToPe, PeFromCa, PeToCa, CaFromPe, PortToPe, PeFromPort, PeToPort,PortFromPe};
use name::{CellID};
use packet::Packet;
use packet_engine::{PacketEngine};
use port::{Port, PortStatus};
use routing_table_entry::RoutingTableEntry;
use utility::{Mask, PortNumber};
use vm::VirtualMachine;

#[derive(Debug)]
pub struct NalCell {
	id: CellID,
	cell_no: usize,
	is_border: bool,
	ports: Box<[Port]>,
	cell_agent: CellAgent,
	packet_engine: PacketEngine,
	vms: Vec<VirtualMachine>,
	ports_from_pe: HashMap<PortNo, PortFromPe>,
}

impl NalCell {
	pub fn new(cell_no: CellNo, nports: PortNo, is_border: bool) -> Result<NalCell> {
		if nports > MAX_PORTS { return Err(ErrorKind::NumberPorts(nports).into()) }
		let cell_id = CellID::new(cell_no)?;
		let (ca_to_pe, pe_from_ca): (CaToPe, PeFromCa) = channel();
		let (pe_to_ca, ca_from_pe): (PeToCa, CaFromPe) = channel();
		let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut ports_from_pe = HashMap::new(); // So I can remove the item
		let mut tcp_port_nos = HashSet::new();
		for i in 0..nports + 1 {
			let is_border_port = is_border & (i == 2);
			if is_border_port { tcp_port_nos.insert(i); }
			let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
			pe_to_ports.push(pe_to_port);
			ports_from_pe.insert(i, port_from_pe);
			let is_connected = if i == 0 { true } else { false };
			let port_number = PortNumber::new(i, nports).chain_err(|| ErrorKind::NalCellError)?;
			let port = Port::new(&cell_id, port_number, is_border_port, is_connected, 
				port_to_pe.clone()).chain_err(|| ErrorKind::NalCellError)?;
			ports.push(port);
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		let packet_engine = PacketEngine::new(&cell_id, pe_to_ca, pe_to_ports, tcp_port_nos).chain_err(|| ErrorKind::NalCellError)?;
		packet_engine.start_threads(pe_from_ca, pe_from_ports)?;
		let mut cell_agent = CellAgent::new(&cell_id, boxed_ports.len() as u8, ca_to_pe).chain_err(|| ErrorKind::NalCellError)?;
		cell_agent.initialize(ca_from_pe)?;
		Ok(NalCell { id: cell_id, cell_no: cell_no, is_border: is_border, 
				ports: boxed_ports, cell_agent: cell_agent, vms: Vec::new(),
				packet_engine: packet_engine, ports_from_pe: ports_from_pe, })
	}
	pub fn get_id(&self) -> &CellID { &self.id }
//	pub fn get_no(&self) -> usize { self.cell_no }
//	pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn get_free_ec_port_mut(&mut self) -> Result<(&mut Port, PortFromPe)> {
		self.get_free_port_mut(false)
	}
	pub fn get_free_tcp_port_mut(&mut self) -> Result<(&mut Port, PortFromPe)> {
		self.get_free_port_mut(true)
	}
	pub fn get_free_port_mut(&mut self, want_tcp_port: bool) 
			-> Result<(&mut Port, PortFromPe)> {
		for port in &mut self.ports.iter_mut() {
			//println!("NalCell {}: port {} is connected {}", self.id, p.get_port_no(), p.is_connected());
			if !port.is_connected() && !(want_tcp_port ^ port.is_border()) && (port.get_port_no() != 0 as u8) {
				let port_no = port.get_port_no();
				match self.ports_from_pe.remove(&port_no) { // Remove avoids a borrowed context error
					Some(recvr) => {
						port.set_connected();
						return Ok((port, recvr))
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
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { NalCellError
		Border(cell_id: CellID) {
			description("Not a border cell")
			display("{} is not a border cell", cell_id)
		}
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
		TcpPort(cell_id: CellID, port_no: PortNo) {
			description("No outside receiver for TCP port")
			display("Cell {} has no outside receiver for TCP port {}", cell_id, port_no)
		}
	}
}
