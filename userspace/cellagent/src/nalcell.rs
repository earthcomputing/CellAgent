use std::{
    fmt, fmt::Write,
    collections::{HashMap, HashSet},
    marker::{PhantomData},
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
use crate::name::{CellID, PortID};
use crate::port::{InteriorPortLike, BorderPortLike, BasePort, InteriorPortFactoryLike, BorderPortFactoryLike};
use crate::replay::{TraceFormat, process_trace_record};
use crate::utility::{CellConfig, CellType, PortNo, S,
                     TraceHeaderParams, TraceType};

#[cfg(not(feature = "cell"))]
#[allow(non_camel_case_types)]
type ECNL_Session = usize;
#[derive(Debug, Clone)]
pub struct NalCell<InteriorPortFactoryType: InteriorPortFactoryLike<InteriorPortType>, InteriorPortType: 'static + Clone + InteriorPortLike, BorderPortFactoryType: BorderPortFactoryLike<BorderPortType>, BorderPortType: 'static + Clone + BorderPortLike> {
    id: CellID,
    cell_type: CellType,
    config: CellConfig,
    ports: Box<[Either<InteriorPortType, BorderPortType>]>,
    cell_agent: CellAgent,
    ports_from_pe_or_ca: HashMap<PortNo, Either<PortFromPe, PortFromCa>>,
    interior_phantom: PhantomData<InteriorPortType>,
    border_phantom: PhantomData<BorderPortType>,
    interior_factory_phantom: PhantomData<InteriorPortFactoryType>,
    border_factory_phantom: PhantomData<BorderPortFactoryType>,
}

impl<InteriorPortFactoryType: InteriorPortFactoryLike<InteriorPortType>, InteriorPortType: 'static + Clone + InteriorPortLike, BorderPortFactoryType: BorderPortFactoryLike<BorderPortType>, BorderPortType: 'static + Clone + BorderPortLike> NalCell::<InteriorPortFactoryType, InteriorPortType, BorderPortFactoryType, BorderPortType> {
    pub fn new(name: &str, num_phys_ports: PortQty, border_port_nos: &HashSet<PortNo>, config: CellConfig, port_factories: HashMap<PortNo, Either<InteriorPortFactoryType, BorderPortFactoryType>>)
            -> Result<(NalCell<InteriorPortFactoryType, InteriorPortType, BorderPortFactoryType, BorderPortType>, JoinHandle<()>), Error> {
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
        let mut ports_from_pe_or_ca = HashMap::new(); // So I can remove the item
        let mut ca_to_ports = HashMap::new();
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_port_setup" };
                let trace = json!({ "cell_name": name });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let cell_type = if border_port_nos.is_empty() { CellType::Interior } else { CellType::Border };
        println!("Hello cell {}", cell_id);
        for port_num in 0..*num_phys_ports {
            let port_no = PortNo(port_num);
            let is_border_port = border_port_nos.contains(&port_no);
            println!("connecting port num {} for cell {}; is_border_port: {}", port_num, name, is_border_port);
            let port_to_pe_or_ca = if is_border_port {
                let (ca_to_port, port_from_ca): (CaToPort, PortFromCa) = channel();
                ca_to_ports.insert(PortNo(port_num), ca_to_port);
                ports_from_pe_or_ca.insert(PortNo(port_num), Either::Right(port_from_ca));
                Either::Right(port_to_ca.clone())
            } else {
                let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
                pe_to_ports.insert(PortNo(port_num), pe_to_port);
                ports_from_pe_or_ca.insert(PortNo(port_num), Either::Left(port_from_pe));
                Either::Left(port_to_pe.clone())
            };
            let port_number = PortNo(port_num).make_port_number(num_phys_ports).context(NalcellError::Chain { func_name: "new", comment: S("port number") })?;
            let ref port_factory = match &port_factories[&port_no] {
                Either::Left(interior_port_factory) => Either::Left(interior_port_factory.clone()),
                Either::Right(border_port_factory) => Either::Right(border_port_factory.clone()),
            };
            // THIS IS ALSO GENERATED IN BasePort::new !!
            let port_id = PortID::new(cell_id, port_number).context(NalcellError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
            let base_port = BasePort::new(
                cell_id,
                port_number,
                is_border_port,
                port_to_pe_or_ca.clone(),
            ).context(NalcellError::Chain { func_name: "new", comment: S("port") })?;
            println!("Hello port {}", port_num);
            let port_factory_clone = port_factory.clone();
            match port_to_pe_or_ca {
                Either::Left(port_to_pe) => {
                    let interior_port_factory = port_factory_clone.left().expect("Nalcell: interior port_to_pe_or_ca doesn't match border port_factory");
                    let sub_port = interior_port_factory.new_port(cell_id, port_id, port_number, port_to_pe)?;
                    ports.push(Either::Left(sub_port));
                },
                Either::Right(port_to_ca) => {
                    let border_port_factory = port_factory_clone.right().expect("Nalcell: border port_to_pe_or_ca doesn't match interior port_factory");
                    let sub_port = border_port_factory.new_port(cell_id, port_id, port_number, port_to_ca)?;
                    ports.push(Either::Right(sub_port));
                },
            }
            println!("Created port {} for cell {}", port_num, name);
        }
        println!("Linked pe channels");
        let boxed_ports: Box<[Either<InteriorPortType, BorderPortType>]> = ports.into_boxed_slice();
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
        Ok((NalCell::<InteriorPortFactoryType, InteriorPortType, BorderPortFactoryType, BorderPortType> {
            id: cell_id,
            cell_type,
            config,
            ports: boxed_ports,
            cell_agent,
            ports_from_pe_or_ca,
            interior_phantom: PhantomData,
            border_phantom: PhantomData,
            interior_factory_phantom: PhantomData,
            border_factory_phantom: PhantomData,
            },
            ca_join_handle))
    }

    pub fn get_id(&self) -> CellID { self.id }
    fn _get_name(&self) -> String { self.id.get_name() }                     // Used only in tests
    fn _get_num_ports(&self) -> PortQty { PortQty(self.ports.len() as u8) }  // Used only in tests
    pub fn get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
    pub fn get_port(&self, port_no: &PortNo) -> Either<InteriorPortType, BorderPortType> {
        self.ports[**port_no as usize].clone()
    }
    pub fn get_port_from_pe_or_ca(&self, port_no: &PortNo) -> Either<PortFromPe, PortFromCa> {
        return self.ports_from_pe_or_ca[port_no].clone();
    }
    pub fn is_border(&self) -> bool {
        match self.cell_type {
            CellType::Border => true,
            CellType::Interior => false,
        }
    }
}

impl<InteriorPortFactoryType: InteriorPortFactoryLike<InteriorPortType>, InteriorPortType: Clone + InteriorPortLike, BorderPortFactoryType: BorderPortFactoryLike<BorderPortType>, BorderPortType: Clone + BorderPortLike> fmt::Display for NalCell::<InteriorPortFactoryType, InteriorPortType, BorderPortFactoryType, BorderPortType> {
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
