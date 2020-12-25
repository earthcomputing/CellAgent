use std::{
    fmt, fmt::Write,
    collections::{HashMap, HashSet},
    thread, thread::JoinHandle,
    iter::FromIterator,
};
use crossbeam::crossbeam_channel::unbounded as channel;
use either::Either;

use crate::app_message_formats::{CaToPort, PortFromCa, PortToCa, CaFromPort};
use crate::cellagent::{CellAgent};
use crate::config::{CONFIG, PortQty};
use crate::dal::{add_to_trace, get_cell_replay_lines};
use crate::ec_message_formats::{PortToPe, PeFromPort, PeToPort, PortFromPe,
                                CmToCa, CaFromCm, CaToCm, CmFromCa, CaToCmBytes, CmToCaBytes,
                                PeToCm, CmFromPe, CmToPe, PeFromCm}; 
#[cfg(feature = "cell")]
use crate::ecnl::ECNL_Session;
#[cfg(feature = "cell")]
use crate::ecnl_port::{ECNL_Port};
use crate::name::CellID;
use crate::port::Port;
use crate::replay::{TraceFormat, process_trace_record};
use crate::utility::{CellConfig, CellType, PortNo, S,
                     TraceHeaderParams, TraceType};
#[cfg(feature = "cell")]
use crate::utility::TraceHeader;

#[cfg(not(feature = "cell"))]
#[allow(non_camel_case_types)]
type ECNL_Session = usize;
#[derive(Debug)]
pub struct NalCell {
    id: CellID,
    cell_type: CellType,
    config: CellConfig,
    ports: Box<[Port]>,
    cell_agent: CellAgent,
    ports_from_pe: HashMap<PortNo, PortFromPe>,
    ports_from_ca: HashMap<PortNo, PortFromCa>,
    ecnl: Option<ECNL_Session>,
}

impl NalCell {
    pub fn new(name: &str, num_phys_ports: PortQty, border_port_nos: &HashSet<PortNo>, config: CellConfig, ecnl: Option<ECNL_Session>)
            -> Result<(NalCell, JoinHandle<()>), Error> {
        let _f = "new";
        if *num_phys_ports > *CONFIG.max_num_phys_ports_per_cell {
            return Err(NalcellError::NumberPorts { num_phys_ports, func_name: "new", max_num_phys_ports: CONFIG.max_num_phys_ports_per_cell }.into())
        }
        let mut trace_lines = get_cell_replay_lines(name).context(NalcellError::Chain { func_name: _f, comment: S(name) })?;
        let (cell_id, tree_ids) = if CONFIG.replay {
            let record = trace_lines.next().transpose()?.expect(&format!("First record for cell {} must be there", name));
            let trace_format = process_trace_record(record)?;
            match trace_format {
                TraceFormat::CaNewFormat(cell_id, my_tree_id, control_tree_id, connected_tree_id) =>
                    (cell_id, Some((my_tree_id, control_tree_id, connected_tree_id))),
                _ => {
                    unimplemented!()
                }
            }
        } else {
            (CellID::new(name).context(NalcellError::Chain { func_name: "new", comment: S("cell_id") })?,
             None)
        };
        let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
        let (port_to_ca, ca_from_ports): (PortToCa, CaFromPort) = channel();
        let port_list: Vec<PortNo> = (0..*num_phys_ports).map(|i| PortNo(i as u8)).collect();
        let all: HashSet<PortNo> = HashSet::from_iter(port_list);
        let mut interior_port_list = all
            .difference(&border_port_nos)
            .cloned()
            .collect::<Vec<_>>();
        interior_port_list.sort();
        let mut ports = Vec::new();
        let mut pe_to_ports = HashMap::new();
        let mut ports_from_pe = HashMap::new(); // So I can remove the item
        let mut ca_to_ports = HashMap::new();
        let mut ports_from_ca = HashMap::new();
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_port_setup" };
                let trace = json!({ "cell_name": name });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let cell_type = if border_port_nos.is_empty() { CellType::Interior } else { CellType::Border };
        for i in 0..=*num_phys_ports {
            #[cfg(feature = "cell")]
            let ecnl_clone = ecnl.clone();
            let is_border_port = border_port_nos.contains(&PortNo(i));
            let is_connected;
            let port_to_pe_or_ca = if is_border_port {
                is_connected = false;
                let (ca_to_port, port_from_ca): (CaToPort, PortFromCa) = channel();
                ca_to_ports.insert(PortNo(i), ca_to_port);
                ports_from_ca.insert(PortNo(i), port_from_ca);
                Either::Right(port_to_ca.clone())
            } else {
                is_connected = if i == 0 {
                    true
                } else {
                    #[cfg(not(feature = "cell"))] {
                        false
                    }
                    #[cfg(feature = "cell")]
                    match ecnl_clone {
                        Some(ecnl_session) => {
                                ecnl_session.get_port(i-1).is_connected()
                        }
                        None => {
                            false
                        }
                    }
                };
                let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
                pe_to_ports.insert(PortNo(i), pe_to_port);
                ports_from_pe.insert(PortNo(i), port_from_pe);
                Either::Left(port_to_pe.clone())
            };
            let port_number = PortNo(i).make_port_number(num_phys_ports).context(NalcellError::Chain { func_name: "new", comment: S("port number") })?;
            let port = Port::new(cell_id, port_number, is_border_port, is_connected, port_to_pe_or_ca).context(NalcellError::Chain { func_name: "new", comment: S("port") })?;
            ports.push(port);
        }
        let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
        let (cm_to_ca, ca_from_cm): (CmToCa, CaFromCm) = channel();
        let (ca_to_cm, cm_from_ca): (CaToCm, CmFromCa) = channel();
        let (pe_to_cm, cm_from_pe): (PeToCm, CmFromPe) = channel();
        let (cm_to_pe, pe_from_cm): (CmToPe, PeFromCm) = channel();
        let (cell_agent, _cm_join_handle) = CellAgent::new(cell_id, tree_ids, cell_type, config,
                 num_phys_ports, ca_to_ports.clone(), cm_to_ca.clone(),
                  pe_from_ports, pe_to_ports,
                  border_port_nos,
                  ca_to_cm.clone(), cm_from_ca, pe_to_cm.clone(),
                            cm_from_pe, cm_to_pe.clone(), pe_from_cm).context(NalcellError::Chain { func_name: "new", comment: S("cell agent create") })?;
        let ca_join_handle = cell_agent.start(ca_from_cm, ca_from_ports);
        if CONFIG.replay {
            thread::spawn(move || -> Result<(), Error> {
                loop {
                    match trace_lines.next().transpose()? {
                        None => break,
                        Some(record) => {
                            let trace_format = process_trace_record(record)?;
                            match trace_format {
                                TraceFormat::EmptyFormat => (),
                                TraceFormat::BorderCell(_) => (),
                                TraceFormat::CaNewFormat(_, _, _, _) => println!("nalcell {}: {} ca_new out of order", cell_id, _f),
                                TraceFormat::CaToCmEntryFormat(entry) => {
                                    ca_to_cm.send(CaToCmBytes::Entry(entry))?;
                                }
                                TraceFormat::CaFromCmBytesMsg(port_no, is_ait, uuid, msg) => {
                                    cm_to_ca.send(CmToCaBytes::Bytes((port_no, is_ait, uuid, msg)))?;
                                }
                                TraceFormat::CaFromCmBytesStatus(port_no, is_border, number_of_packets, status) => {
                                    cm_to_ca.send(CmToCaBytes::Status((port_no, is_border, number_of_packets, status)))?;
                                }
                                TraceFormat::CaToNoc(noc_port, bytes) => {
                                    let ca_to_port = ca_to_ports.get(&noc_port).expect("cellagent.rs: border port sender must be set");
                                    ca_to_port.send(bytes)?;
    
                                }
                            };
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(100));
                println!("Noc {} thread exit", cell_id);
                Ok(())
            });
        }
        Ok((NalCell { id: cell_id, cell_type, config, ports: boxed_ports, cell_agent,
            ports_from_pe, ports_from_ca, ecnl },
            ca_join_handle))
    }

    pub fn get_id(&self) -> CellID { self.id }
    fn _get_name(&self) -> String { self.id.get_name() }                     // Used only in tests
    fn _get_num_ports(&self) -> PortQty { PortQty(self.ports.len() as u8) }  // Used only in tests
    pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
    pub fn take_port_from_ca(&mut self, port_no: PortNo) -> Option<PortFromCa> {
        self.ports_from_ca.remove(&port_no)
    }
    pub fn is_border(&self) -> bool {
        match self.cell_type {
            CellType::Border => true,
            CellType::Interior => false,
        }
    }
    pub fn get_free_ec_port_mut(&mut self) -> Result<(&mut Port, PortFromPe), Error> {
        let _f = "get_free_ec_port_mut";
        let cell_id = self.id;
        let port = self.ports
            .iter_mut()
            .filter(|port| !port.is_connected())
            .filter(|port| !port.is_border())
            .filter(|port| (*(port.get_port_no()) != 0 as u8))
            .nth(0)
            .ok_or::<Error>(NalcellError::NoFreePorts{ cell_id, func_name: _f }.into())?;
        port.set_connected();
        let recvr = self.ports_from_pe.remove(&port.get_port_no())
            .ok_or::<Error>(NalcellError::Channel { port_no: port.get_port_no(), func_name: _f }.into())?;
        Ok((port, recvr))
    }
    pub fn get_free_boundary_port_mut(&mut self) -> Result<(&mut Port, PortFromCa), Error> {
        let _f = "get_free_boundary_port_mut";
        let cell_id = self.id;
        let port = self.ports
            .iter_mut()
            .filter(|port| !port.is_connected())
            .filter(|port| port.is_border())
            .filter(|port| (*(port.get_port_no()) != 0 as u8))
            .nth(0)
            .ok_or::<Error>(NalcellError::NoFreePorts{ cell_id, func_name: _f }.into())?;
        port.set_connected();
        let recvr = self.ports_from_ca.remove(&port.get_port_no())
            .ok_or::<Error>(NalcellError::Channel { port_no: port.get_port_no(), func_name: _f }.into())?;
        Ok((port, recvr))
    }
    #[cfg(feature = "cell")]
    pub fn link_ecnl_channels(&mut self, mut ecnl: ECNL_Session) -> Result<&mut Self, Error> {
        let _f = "link_ecnl_channels";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        for port_id in 0..=*(ecnl.num_ecnl_ports())-1 {
            let port = self.ports[port_id as usize].clone();
            let ecnl_port: ECNL_Port = ECNL_Port::new(port_id as u8, port.clone());
            port.link_channel(Either::Right(ecnl_port.clone()), (self.ports_from_pe[&PortNo(port_id as u8)]).clone());
            ecnl.push_port(ecnl_port);
        }
        println!("Linked ecnl channels");
        Ok(self)
    }
}
impl fmt::Display for NalCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        match self.cell_type {
            CellType::Border => write!(s, "Border Cell {}", self.id)?,
            CellType::Interior => write!(s, "Cell {}", self.id)?,
        }
        write!(s, " {}", self.config)?;
        write!(s, "\n{}", self.cell_agent)?;
        write!(f, "{}", s)
    }
}

impl Drop for NalCell {
    fn drop(&mut self) {
        match &self.ecnl {
            Some(ecnl_session) => {
                drop(ecnl_session);
            },
            None => {
            },
        }
    }
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
    #[fail(display = "NalcellError::NumberPorts {}: You asked for {:?} ports, but only {:?} are allowed", func_name, num_phys_ports, max_num_phys_ports)]
    NumberPorts { func_name: &'static str, num_phys_ports: PortQty, max_num_phys_ports: PortQty },
    #[fail(display = "NalCellError::Replay {}: Error opening replay file {}", func_name, cell_name)]
    Replay { func_name: &'static str, cell_name: String }
}
