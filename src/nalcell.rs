use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::channel;
use std::thread;

use kafka::producer::Producer;

use cellagent::{CellAgent};
use cmodel::{Cmodel};
use dal;
use config::{MAX_PORTS, CellNo, CellType, PortNo};
use message_types::{PortToPe, PeFromPort, PeToPort,PortFromPe,
                    CaToCm, CmFromCa, CmToCa, CaFromCm,
                    CmToPe, PeFromCm, PeToCm, CmFromPe};
use name::{CellID};
use packet_engine::{PacketEngine};
use port::{Port};
use utility::{PortNumber, S, TraceHeader, TraceHeaderParams, TraceType};
use vm::VirtualMachine;

const MODULE: &'static str = "nalcell.rs";

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
	pub fn new(cell_no: CellNo, nports: PortNo, cell_type: CellType,
               config: CellConfig, producer: &mut Producer, mut trace_header: TraceHeader) -> Result<NalCell, Error> {
        let f = "new";
		if *nports > *MAX_PORTS { return Err(NalcellError::NumberPorts { nports, func_name: "new", max_ports: MAX_PORTS }.into()) }
		let cell_id = CellID::new(cell_no).context(NalcellError::Chain { func_name: "new", comment: S("cell_id")})?;
        let (ca_to_cm, cm_from_ca): (CaToCm, CmFromCa) = channel();
        let (cm_to_ca, ca_from_cm): (CmToCa, CaFromCm) = channel();
        let (cm_to_pe, pe_from_cm): (CmToPe, PeFromCm) = channel();
        let (pe_to_cm, cm_from_pe): (PeToCm, CmFromPe) = channel();
		let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
		let mut ports = Vec::new();
		let mut pe_to_ports = Vec::new();
		let mut ports_from_pe = HashMap::new(); // So I can remove the item
		let mut boundary_port_nos = HashSet::new();
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "nalcell_port_setup" };
            let trace = json!({ "cell_number": cell_no });
            let _ = dal::add_to_trace(producer, &mut trace_header, TraceType::Trace, trace_params, &trace, f);
        }
		for i in 0..*nports + 1 {
			let is_border_port = match cell_type {
				CellType::Border => {
					let is_border_port = i == 2;
					if is_border_port { boundary_port_nos.insert(PortNo(i)); }
					is_border_port					
				}
				CellType::Interior => false
			};
			let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
			pe_to_ports.push(pe_to_port);
			ports_from_pe.insert(PortNo(i), port_from_pe);
			let is_connected = if i == 0 { true } else { false };
			let port_number = PortNumber::new(PortNo(i), nports).context(NalcellError::Chain { func_name: "new", comment: S("port number")})?;
			let port = Port::new(&cell_id, port_number, is_border_port, is_connected,port_to_pe.clone()).context(NalcellError::Chain { func_name: "new", comment: S("port")})?;
			ports.push(port);
		}
		let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
		let cell_agent = CellAgent::new(&cell_id, cell_type, config, nports, ca_to_cm).context(NalcellError::Chain { func_name: "new", comment: S("cell agent create")})?;
        NalCell::start_cell(&cell_agent, ca_from_cm, producer, &mut trace_header);
        let cmodel = Cmodel::new(&cell_id);
        NalCell::start_cmodel(&cmodel, cm_from_ca, cm_to_pe, cm_from_pe, cm_to_ca, &mut trace_header);
		let packet_engine = PacketEngine::new(&cell_id, pe_to_cm, pe_to_ports, boundary_port_nos).context(NalcellError::Chain { func_name: "new", comment: S("packet engine create")})?;
		NalCell::start_packet_engine(&packet_engine, pe_from_cm, pe_from_ports, producer, &mut trace_header);
		Ok(NalCell { id: cell_id, cell_no, cell_type, config,
				ports: boxed_ports, cell_agent, vms: Vec::new(),
				packet_engine, ports_from_pe })
	}
    fn start_cell(cell_agent: &CellAgent, ca_from_cm: CaFromCm, producer: &mut Producer, outer_trace_header: &mut TraceHeader) {
        let f = "start_cell";
        let mut ca = cell_agent.clone();
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "nalcell_start_ca" };
            let trace = json!({  "cell_id": &ca.get_id() });
            let _ = dal::add_to_trace(producer, outer_trace_header, TraceType::Trace, trace_params, &trace, f);
        }
		let mut outer_trace_header_clone = outer_trace_header.clone();
        thread::spawn( move || {
            let ref mut producer = dal::make_kafka_producer(f).expect("Kafka error");
            let inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = ca.initialize(ca_from_cm, producer, inner_trace_header).map_err(|e| ::utility::write_err("nalcell", e));
            // Don't automatically restart cell agent if it crashes
        });
    }
    fn start_cmodel(cmodel: &Cmodel, cm_from_ca: CmFromCa, cm_to_pe: CmToPe, cm_from_pe: CmFromPe, cm_to_ca: CmToCa,
                    outer_trace_header: &mut TraceHeader) {
        let f = "start_cmodel";
        let cm = cmodel.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        thread::spawn( move || {
            let ref mut producer = dal::make_kafka_producer(f).expect("Kafka error");
            let inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = cm.initialize(cm_from_ca, cm_to_pe, cm_from_pe, cm_to_ca, producer, inner_trace_header);
            // Don't automatically restart cmodel if it crashes
        });
    }
    fn start_packet_engine(packet_engine: &PacketEngine, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort,
                           producer: &mut Producer, outer_trace_header: &mut TraceHeader) {
        let f = "start_packet_engine";
        let pe = packet_engine.clone();
        {
            let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "nalcell_start_pe" };
            let trace = json!({ "cell_id": &pe.get_id() });
            let _ = dal::add_to_trace(producer, outer_trace_header, TraceType::Trace, trace_params, &trace, f);
        }
        let mut outer_trace_header_clone = outer_trace_header.clone();
        thread::spawn( move || {
            let ref mut producer = dal::make_kafka_producer(f).expect("Kafka error");
            let inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = pe.initialize(pe_from_cm, pe_from_ports, producer, inner_trace_header).map_err(|e| ::utility::write_err("nalcell", e));
            // Don't automatically restart packet engine if it crashes
        });
    }
	pub fn get_id(&self) -> &CellID { &self.id }
	pub fn get_no(&self) -> CellNo { self.cell_no }
//	pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
	pub fn is_border(&self) -> bool {
		match self.cell_type {
			CellType::Border => true,
			CellType::Interior => false,
		}  
	}
	pub fn get_free_ec_port_mut(&mut self) -> Result<(&mut Port, PortFromPe), Error> {
		self.get_free_port_mut(false)
	}
	pub fn get_free_boundary_port_mut(&mut self) -> Result<(&mut Port, PortFromPe), Error> {
		self.get_free_port_mut(true)
	}
	pub fn get_free_port_mut(&mut self, want_boundary_port: bool) 
			-> Result<(&mut Port, PortFromPe), Error> {
        let f = "Nalcell::get_free_port_mut";
		for port in &mut self.ports.iter_mut() {
			if !port.is_connected() && !(want_boundary_port ^ port.is_border()) && (*(port.get_port_no()) != 0 as u8) {
				let port_no = port.get_port_no();
				match self.ports_from_pe.remove(&port_no) { // Remove avoids a borrowed context error
					Some(recvr) => {
						port.set_connected();
						return Ok((port, recvr))
					},
					None => return Err(NalcellError::Channel { port_no, func_name: f }.into())
				} 
			}
		}
		Err(NalcellError::NoFreePorts{ cell_id: self.id.clone(), func_name: f }.into())
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
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum NalcellError {
	#[fail(display = "NalcellError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
    #[fail(display = "NalcellError::Channel {}: No receiver for port {:?}", func_name, port_no)]
    Channel { func_name: &'static str, port_no: PortNo },
    #[fail(display = "NalcellError::NoFreePorts {}: All ports have been assigned for cell {}", func_name, cell_id)]
    NoFreePorts { func_name: &'static str, cell_id: CellID },
    #[fail(display = "NalcellError::NumberPorts {}: You asked for {:?} ports, but only {:?} are allowed", func_name, nports, max_ports)]
    NumberPorts { func_name: &'static str, nports: PortNo, max_ports: PortNo }
}
