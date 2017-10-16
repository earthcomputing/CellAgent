use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::channel;

use cellagent::{CellAgent};
use config::{MAX_PORTS, CellNo, CellType, PortNo};
use message_types::{CaToPe, PeFromCa, PeToCa, CaFromPe, PortToPe, PeFromPort, PeToPort,PortFromPe};
use name::{CellID};
use packet_engine::{PacketEngine};
use port::{Port};
use utility::{PortNumber};
use vm::VirtualMachine;

#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum CellConfig { Small, Medium, Large }
impl fmt::Display for CellConfig { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = match *self {
			CellConfig::Small  => "Small",
			CellConfig::Medium => "Medium",
			CellConfig::Large  => "Large"
		};
		write!(f, "{}", s) 
	}
}

#[derive(Debug)]
pub struct NalCell {
	id: CellID,
	cell_type: CellType,
	config: CellConfig,
	cell_no: CellNo,
	ports: Box<[Port]>,
	cell_agent: CellAgent,
	packet_engine: PacketEngine,
	vms: Vec<VirtualMachine>,
	ports_from_pe: HashMap<PortNo, PortFromPe>,
}

impl NalCell {
	pub fn new(cell_no: CellNo, nports: PortNo, cell_type: CellType, config: CellConfig) -> Result<NalCell> {
		if nports.v > MAX_PORTS.v { return Err(ErrorKind::NumberPorts(nports, "new".to_string()).into()) }
		let cell_id = CellID::new(cell_no)?;
		let (ca_to_pe, pe_from_ca): (CaToPe, PeFromCa) = channel();
		let (pe_to_ca, ca_from_pe): (PeToCa, CaFromPe) = channel();
		let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut ports_from_pe = HashMap::new(); // So I can remove the item
		let mut boundary_port_nos = HashSet::new();
		for i in 0..nports.v + 1 {
			let is_border_port = match cell_type {
				CellType::Border => {
					let is_border_port = i == 2;
					if is_border_port { boundary_port_nos.insert(PortNo{v:i}); }
					is_border_port					
				}
				CellType::Interior => false
			};
			let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
			pe_to_ports.push(pe_to_port);
			ports_from_pe.insert(PortNo{v:i}, port_from_pe);
			let is_connected = if i == 0 { true } else { false };
			let port_number = PortNumber::new(PortNo{v:i}, nports)?;
			let port = Port::new(&cell_id, port_number, is_border_port, is_connected, 
				port_to_pe.clone())?;
			ports.push(port);
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		let mut cell_agent = CellAgent::new(&cell_id, cell_type, config, nports, ca_to_pe)?;
		cell_agent.initialize(cell_type, ca_from_pe)?;
		let packet_engine = PacketEngine::new(&cell_id, pe_to_ca, pe_to_ports, boundary_port_nos)?;
		packet_engine.start_threads(pe_from_ca, pe_from_ports);
		Ok(NalCell { id: cell_id, cell_no: cell_no, cell_type: cell_type, config: config,
				ports: boxed_ports, cell_agent: cell_agent, vms: Vec::new(),
				packet_engine: packet_engine, ports_from_pe: ports_from_pe, })
	}
//	pub fn get_id(&self) -> &CellID { &self.id }
//	pub fn get_no(&self) -> usize { self.cell_no }
//	pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
	pub fn is_border(&self) -> bool {
		match self.cell_type {
			CellType::Border => true,
			CellType::Interior => false,
		}  
	}
	pub fn get_free_ec_port_mut(&mut self) -> Result<(&mut Port, PortFromPe)> {
		self.get_free_port_mut(false)
	}
	pub fn get_free_boundary_port_mut(&mut self) -> Result<(&mut Port, PortFromPe)> {
		self.get_free_port_mut(true)
	}
	pub fn get_free_port_mut(&mut self, want_boundary_port: bool) 
			-> Result<(&mut Port, PortFromPe)> {
		for port in &mut self.ports.iter_mut() {
			//println!("NalCell {}: port {} is connected {}", self.id, p.get_port_no(), p.is_connected());
			if !port.is_connected() && !(want_boundary_port ^ port.is_border()) && (port.get_port_no().v != 0 as u8) {
				let port_no = port.get_port_no();
				match self.ports_from_pe.remove(&port_no) { // Remove avoids a borrowed context error
					Some(recvr) => {
						Port::set_connected(port.get_is_connected());
						return Ok((port, recvr))
					},
					None => return Err(ErrorKind::Channel(port_no, "get_free_port_mut".to_string()).into())
				} 
			}
		}
		Err(ErrorKind::NoFreePorts(self.id.clone(), "get_free_port_mut".to_string()).into())
	}
}
impl fmt::Display for NalCell { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = String::new();
		match self.cell_type { 
			CellType::Border => s = s + &format!("Border Cell {}", self.id),
			CellType::Interior => s = s + &format!("Cell {}", self.id)
		}
		s = s + &format!(" {}", self.config);
		s = s + &format!("\n{}", self.cell_agent);
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
	errors { 
		Channel(port_no: PortNo, func_name: String) {
			display("NalCell {}: No receiver for port {}", func_name, port_no.v)
		}
		NoFreePorts(cell_id: CellID, func_name: String) {
			display("NalCell {}: All ports have been assigned for cell {}", func_name, cell_id)
		}
		NumberPorts(nports: PortNo, func_name: String) {
			display("NalCell {}: You asked for {} ports, but only {} are allowed", func_name, nports.v, MAX_PORTS.v)
		}
	}
}
