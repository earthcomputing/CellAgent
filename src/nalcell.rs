#[cfg(feature = "cell")]
extern crate libc;

use std::{
    fmt, fmt::Write,
    collections::{HashMap, HashSet},
    os::raw::{c_void},
    thread,
    iter::FromIterator
};
use crossbeam::crossbeam_channel::unbounded as channel;

#[cfg(feature = "cell")]
use libc::{free};
#[cfg(feature = "cell")]
use std::{
    os::raw::{c_char, c_int, c_uchar, c_uint},
    ptr::{null, null_mut},	  
    ffi::CStr,
};

use either::Either;

use crate::cellagent::{CellAgent};
use crate::cmodel::{Cmodel};
use crate::config::{CONFIG, PortQty};
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message_formats::{PortToPe, PeFromPort, PeToPort,PortFromPe,
                                CaToCm, CmFromCa, CmToCa, CaFromCm,
                                CmToPe, PeFromCm, PeToCm, CmFromPe};
use crate::name::{CellID};
use crate::packet_engine::{PacketEngine};
use crate::port::{Port};
use crate::utility::{CellConfig, CellType, PortNo, S, TraceHeaderParams, TraceType, write_err};
use crate::vm::VirtualMachine;

#[cfg(feature = "cell")]
#[allow(improper_ctypes)]
#[link(name = ":ecnl_sdk.o")]
#[link(name = ":libnl-3.so")]
#[link(name = ":libnl-genl-3.so")]
extern {
    pub fn alloc_ecnl_session(ecnl_session_ptr: *const *mut c_void) -> c_int;
    pub fn get_module_info(ecnl_session: *mut c_void, mipp: *const *const ModuleInfo) -> c_int;
    pub fn free_ecnl_session(ecnl_session: *mut c_void) -> c_int;
}

#[derive(Debug)]
pub struct NalCell {
    id: CellID,
    cell_type: CellType,
    config: CellConfig,
    ports: Box<[Port]>,
    cell_agent: CellAgent,
    cmodel: Cmodel,
    packet_engine: PacketEngine,
    vms: Vec<VirtualMachine>,
    ports_from_pe: HashMap<PortNo, PortFromPe>,
    ports_from_ca: HashMap<PortNo, PortFromCa>,
    ecnl: Option<*mut c_void>,
}

impl NalCell {
    pub fn new(name: &str, simulated_options: Option<PortQty>, border_port_nos: &HashSet<PortNo>, config: CellConfig)
            -> Result<NalCell, Error> {
        let _f = "new";
        let ecnl =
            match simulated_options {
                Some(_) => None,
                None => {
                    #[cfg(feature = "cell")]
                        {
                            let ecnl_session: *mut c_void = null_mut();
                            let ecnl_session_ptr: *const *mut c_void = &ecnl_session;
                            unsafe {
                                alloc_ecnl_session(ecnl_session_ptr);
                                Some(*ecnl_session_ptr)
                            }
                        }
                    #[cfg(feature = "simulator")]
                        {
                            None
                        }
                },
            };
        let num_phys_ports =
            match simulated_options {
                Some(num_phys_ports) => num_phys_ports,
                None => {
                    #[cfg(feature = "cell")]
                        let mip: *const ModuleInfo = null();
                    #[cfg(feature = "cell")]
                        unsafe
                        {
                            get_module_info(ecnl.unwrap(), &mip);
                            let module_id = (*mip).module_id as u8;
                            println!("Module id: {:?} ", module_id);
                            let module_name = CStr::from_ptr((*mip).module_name).to_string_lossy().into_owned();
                            println!("Module name: {:?} ", module_name);
                            let num_phys_ports = (*mip).num_ports as u8;
                            println!("Num phys ports: {:?} ", num_phys_ports);
                            free(mip as *mut libc::c_void);
                            PortQty(num_phys_ports)
                        }
                    #[cfg(feature = "simulator")]
                        {
                            PortQty(0)
                        }
                }
            };
        if *num_phys_ports > *CONFIG.max_num_phys_ports_per_cell {
            return Err(NalcellError::NumberPorts { num_phys_ports, func_name: "new", max_num_phys_ports: CONFIG.max_num_phys_ports_per_cell }.into())
        }
        let cell_id = CellID::new(name).context(NalcellError::Chain { func_name: "new", comment: S("cell_id") })?;
        let (ca_to_cm, cm_from_ca): (CaToCm, CmFromCa) = channel();
        let (cm_to_ca, ca_from_cm): (CmToCa, CaFromCm) = channel();
        let (cm_to_pe, pe_from_cm): (CmToPe, PeFromCm) = channel();
        let (pe_to_cm, cm_from_pe): (PeToCm, CmFromPe) = channel();
        let (port_to_pe, pe_from_ports): (PortToPe, PeFromPort) = channel();
        let (port_to_ca, ca_from_ports): (PortToCa, CaFromPort) = channel();
        let port_list: Vec<PortNo> = (0..*num_phys_ports)
            .map(|i| PortNo(i as u8))
            .collect();
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
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let cell_type = if border_port_nos.is_empty() { CellType::Interior } else { CellType::Border };
        for i in 0..=*num_phys_ports {
            let is_border_port = border_port_nos.contains(&PortNo(i));
            let port_to_pe_or_ca = if is_border_port {
                let (ca_to_port, port_from_ca): (CaToPort, PortFromCa) = channel();
                ca_to_ports.insert(PortNo(i), ca_to_port);
                ports_from_ca.insert(PortNo(i), port_from_ca);
                Either::Right(port_to_ca.clone())
            } else {
                let (pe_to_port, port_from_pe): (PeToPort, PortFromPe) = channel();
                pe_to_ports.insert(PortNo(i), pe_to_port);
                ports_from_pe.insert(PortNo(i), port_from_pe);
                Either::Left(port_to_pe.clone())
            };
            let is_connected = i == 0;
            let port_number = PortNo(i).make_port_number(num_phys_ports).context(NalcellError::Chain { func_name: "new", comment: S("port number") })?;
            let port = Port::new(cell_id, port_number, is_border_port, is_connected, port_to_pe_or_ca).context(NalcellError::Chain { func_name: "new", comment: S("port") })?;
            ports.push(port);
        }
        let boxed_ports: Box<[Port]> = ports.into_boxed_slice();
        let cell_agent = CellAgent::new(cell_id, cell_type, config, num_phys_ports, ca_to_ports, ca_to_cm).context(NalcellError::Chain { func_name: "new", comment: S("cell agent create") })?;
        NalCell::start_cell(&cell_agent, ca_from_cm, ca_from_ports);
        let cmodel = Cmodel::new(cell_id);
        NalCell::start_cmodel(&cmodel, cm_from_ca, cm_to_pe, cm_from_pe, cm_to_ca);
        let packet_engine = PacketEngine::new(cell_id, cell_agent.get_connected_tree_id(),
                                              pe_to_cm, pe_to_ports, border_port_nos).context(NalcellError::Chain { func_name: "new", comment: S("packet engine create") })?;
        NalCell::start_packet_engine(&packet_engine, pe_from_cm, pe_from_ports);
        Ok(NalCell {
            id: cell_id,
            cell_type,
            config,
            cmodel,
            ports: boxed_ports,
            cell_agent,
            vms: Vec::new(),
            packet_engine,
            ports_from_pe,
            ports_from_ca,
            ecnl
        })
    }
    // SPAWN THREAD (ca.initialize)
    fn start_cell(cell_agent: &CellAgent, ca_from_cm: CaFromCm, ca_from_ports: CaFromPort) {
        let _f = "start_cell";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_start_ca" };
                let trace = json!({ "cell_id": &cell_agent.get_cell_id() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let mut ca = cell_agent.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("CellAgent {}", cell_agent.get_cell_id());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = ca.initialize(ca_from_cm, ca_from_ports).map_err(|e| write_err("nalcell", &e));
            if CONFIG.continue_on_error { } // Don't automatically restart cell agent if it crashes
        }).expect("thread failed");
    }

    // SPAWN THREAD (cm.initialize)
    fn start_cmodel(cmodel: &Cmodel, cm_from_ca: CmFromCa, cm_to_pe: CmToPe,
                    cm_from_pe: CmFromPe, cm_to_ca: CmToCa) {
        let _f = "start_cmodel";
        let cm = cmodel.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Cmodel {}", cmodel.get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = cm.initialize(cm_from_ca, cm_to_pe, cm_from_pe, cm_to_ca);
            if CONFIG.continue_on_error { } // Don't automatically restart cmodel if it crashes
        }).expect("thread failed");
    }

    // SPAWN THREAD (pe.initialize)
    fn start_packet_engine(packet_engine: &PacketEngine, pe_from_cm: PeFromCm, pe_from_ports: PeFromPort) {
        let _f = "start_packet_engine";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.nal {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "nalcell_start_pe" };
                let trace = json!({ "cell_id": packet_engine.get_cell_id() });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let pe = packet_engine.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("PacketEngine {}", packet_engine.get_cell_id());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = pe.initialize(pe_from_cm, pe_from_ports).map_err(|e| write_err("nalcell", &e));
            if CONFIG.continue_on_error { } // Don't automatically restart packet engine if it crashes
        }).expect("thread failed");
    }

    pub fn get_id(&self) -> CellID { self.id }
    pub fn get_name(&self) -> String { self.id.get_name() }                     // Used only in tests
    pub fn get_num_ports(&self) -> PortQty { PortQty(self.ports.len() as u8) }  // Used only in tests
    pub fn _get_cell_agent(&self) -> &CellAgent { &self.cell_agent }
    //pub fn get_cmodel(&self) -> &Cmodel { &self.cmodel }
    pub fn get_packet_engine(&self) -> &PacketEngine { &self.packet_engine }
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
}
impl fmt::Display for NalCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        match self.cell_type {
            CellType::Border => write!(s, "Border Cell {}", self.id)?,
            CellType::Interior => write!(s, "Cell {}", self.id)?
        }
        write!(s, " {}", self.config)?;
        write!(s, "\n{}", self.cell_agent)?;
        write!(s, "\n{}", self.packet_engine)?;
        write!(f, "{}", s) }
}
impl Drop for NalCell {
    fn drop(&mut self) {
        match self.ecnl {
            Some(ecnl_session) => {
                #[cfg(feature = "cell")]
                unsafe {
                    free_ecnl_session(ecnl_session);
                }
            },
            None => {
            },
        }
    }
}

#[cfg(feature = "cell")]
#[repr(C)]
pub struct ModuleInfo {
    module_id: c_uint,
    module_name: *const c_char,
    num_ports: c_uint,
}


// Errors
use failure::{Error, ResultExt};
use crate::app_message_formats::{PortToCa, CaFromPort, CaToPort, PortFromCa};

#[derive(Debug, Fail)]
pub enum NalcellError {
    #[fail(display = "NalcellError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "NalcellError::Channel {}: No receiver for port {:?}", func_name, port_no)]
    Channel { func_name: &'static str, port_no: PortNo },
    #[fail(display = "NalcellError::NoFreePorts {}: All ports have been assigned for cell {}", func_name, cell_id)]
    NoFreePorts { func_name: &'static str, cell_id: CellID },
    #[fail(display = "NalcellError::NumberPorts {}: You asked for {:?} ports, but only {:?} are allowed", func_name, num_phys_ports, max_num_phys_ports)]
    NumberPorts { func_name: &'static str, num_phys_ports: PortQty, max_num_phys_ports: PortQty }
}
