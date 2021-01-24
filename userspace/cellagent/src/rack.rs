use either::Either;
use multi_mut::HashMapMultiMut;
use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          iter::FromIterator,
          //sync::mpsc::channel,
          sync::{Arc},
          thread, thread::{JoinHandle}};
use crossbeam::crossbeam_channel::unbounded as channel;

use crate::app_message_formats::{PortFromCa};
use crate::blueprint::{Blueprint, Cell, };
use crate::config::{CONFIG, CellQty, LinkQty};
use crate::dal::{add_to_trace, fork_trace_header, get_cell_replay_lines, update_trace_header};
use crate::ec_message_formats::{PortFromPe};
use crate::link::{Link};
use crate::nalcell::{NalCell};
use crate::name::{CellID, LinkID};
use crate::port::{PortData};
use crate::replay::{process_trace_record, TraceFormat};
use crate::simulated_border_port::{NocFromPort, NocToPort, PortFromNoc, PortToNoc, SimulatedBorderPort};
use crate::simulated_internal_port::{LinkFromPort, LinkToPort, PortFromLink, PortToLink, SimulatedInteriorPort};
use crate::utility::{CellNo, CellConfig, Edge, S, TraceHeaderParams, TraceType};

#[derive(Clone, Debug, Default)]
pub struct Rack {
    cells: HashMap<CellNo, NalCell<SimulatedInteriorPort, SimulatedBorderPort>>,
    links: HashMap<Edge, Link>,
}
impl Rack {
    pub fn new() -> Rack { Default::default() }
    pub fn initialize(&mut self, blueprint: &Blueprint)  -> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "initialize";
        let num_cells = blueprint.get_ncells();
        let edge_list = blueprint.get_edge_list();
        if *num_cells < 1  { return Err(RackError::Cells{ num_cells, func_name: _f }.into()); }
        if edge_list.len() < *num_cells - 1 { return Err(RackError::Edges { nlinks: LinkQty(edge_list.len()), func_name: _f }.into() ); }
        for border_cell in blueprint.get_border_cells() {
            let cell_no = border_cell.get_cell_no();
            let border_ports = border_cell.get_border_ports();
            let (nal_cell, _join_handle) = match NalCell::<SimulatedInteriorPort, SimulatedBorderPort>::new(&border_cell.get_name(),
                                                              border_cell.get_num_phys_ports(),
                                                              &HashSet::from_iter(border_ports.clone()),
                                                              CellConfig::Large,
                                                              None,
            ) {
                Ok(t) => t,
                Err(e) => {
                    println!("Rack: {} error from nalcell {}", _f, e);
                    return Err(RackError::Chain { func_name: _f, comment: S("Border cell") }.into() );
                }
            };
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc || CONFIG.trace_options.visualize { // Needed for visualization
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell_start" };
                    let cell_id = nal_cell.get_id();
                    let trace = json!({ "cell_id": cell_id, "cell_number": cell_no,
                            "border_ports": border_ports, "location":  CONFIG.geometry.get(*cell_no)});
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.cells.insert(cell_no, nal_cell);
        }
        for interior_cell in blueprint.get_interior_cells() {
            let cell_no = interior_cell.get_cell_no();
            let (nal_cell, _join_handle) = match NalCell::<SimulatedInteriorPort, SimulatedBorderPort>::new(&interior_cell.get_name(),
                                                        interior_cell.get_num_phys_ports(),
                                                        &HashSet::new(),
                                                        CellConfig::Large,
                                                        None,
                                                        )
            {
                Ok(t) => t,
                Err(e) => {
                    println!("Rack: {} error from nalcell {}", _f, e);
                    return Err(RackError::Chain { func_name: _f, comment: S("Interior cell") }.into());
                }
            };
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc || CONFIG.trace_options.visualize { // Needed for visualization
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "interior_cell_start" };
                    let cell_id = nal_cell.get_id();
                    let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location": CONFIG.geometry.get(*cell_no as usize) });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.cells.insert(cell_no, nal_cell);
        }
        let mut link_handles = Vec::new();
        for edge in edge_list {
            if (*(edge.0) > num_cells.0) | (*(edge.1) >= num_cells.0) { return Err(RackError::Wire { edge: *edge, func_name: _f, comment: "greater than num_cells test" }.into()); }
            let (left_cell, rite_cell) = self.cells
                .get_pair_mut(&edge.0, &edge.1)
                .unwrap();
            let left_cell_id: CellID = left_cell.get_id(); // For Trace
            let (left_port, left_from_pe): (&mut PortData<SimulatedInteriorPort, SimulatedBorderPort>, PortFromPe) = left_cell.get_free_ec_port_mut()?;
            let rite_cell_id: CellID = rite_cell.get_id(); // For Trace
            let (rite_port, rite_from_pe): (&mut PortData<SimulatedInteriorPort, SimulatedBorderPort>, PortFromPe) = rite_cell.get_free_ec_port_mut()?;
            let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
            let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
            let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
            let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
            left_port.listen_link_and_pe(SimulatedInteriorPort::new(left_port.clone(), left_to_link, left_from_link)?, left_from_pe);
            rite_port.listen_link_and_pe(SimulatedInteriorPort::new(rite_port.clone(), rite_to_link, rite_from_link)?, rite_from_pe);
            let link = Link::new(left_port.get_id(), rite_port.get_id(),
                                           link_to_left, link_to_rite)?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "connect_link" };
                    let trace = json!({ "left_cell": left_cell_id, "rite_cell": rite_cell_id, "left_port": left_port.get_port_no(), "rite_port": rite_port.get_port_no(), "link_id": link.get_id() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let mut link_clone = link.clone();
            let child_trace_header = fork_trace_header();
            let thread_name = format!("Link {} thread", link.get_id());
            let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
                update_trace_header(child_trace_header);
                let _ = link_clone.listen(link_from_left, link_from_rite);
            })?;
            //let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
            link_handles.append(&mut vec![join_handle]);
            self.links.insert(*edge, link);
        }
        println!("Rack {}: Links started", _f);
        Ok(link_handles)
    }
    pub fn construct(blueprint: &Blueprint) -> Result<(Rack, Vec<JoinHandle<()>>), Error> {
        let _f = "construct";
        let mut rack = Rack::new();
        let join_handles = rack.initialize(blueprint).context(RackError::Chain { func_name: _f, comment: S("initialize")})?;
        Ok((rack, join_handles))
    }
    pub fn get_cells(&self) -> &HashMap<CellNo, NalCell<SimulatedInteriorPort, SimulatedBorderPort>> { &self.cells }
    pub fn get_links_mut(&mut self) -> &mut HashMap<Edge, Link> { &mut self.links }
    pub fn get_links(&self) -> &HashMap<Edge, Link> { &self.links }
    pub fn get_cell_ids(&self) -> HashMap<CellNo, CellID> {
        self.cells.iter().map(|cell_no_and_cell| (*cell_no_and_cell.0, cell_no_and_cell.1.get_id())).collect::<HashMap<CellNo, _>>()
    }
    pub fn get_link_ids(&self) -> HashMap<Edge, LinkID> {
        self.links.iter().map(|edge_and_link| (*edge_and_link.0, edge_and_link.1.get_id())).collect::<HashMap<Edge,  _>>()
    }
    pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)
            -> Result<(), Error> {
        let _f = "connect_to_noc";
        let (cell_no, cell) = if CONFIG.replay {
            let mut trace_lines = get_cell_replay_lines("Rack").context(RackError::Chain { func_name: _f, comment: S("Rack") })?;
            let record = trace_lines.next().transpose()?.expect(&format!("First record for rack must be there"));
            let trace_format = process_trace_record(record)?;
            match trace_format {
                TraceFormat::BorderCell(cell_no,) => {
                    let cell = self.cells.get_mut(&cell_no)
                        .ok_or::<Error>(RackError::Boundary { func_name: _f }.into())?;
                    (cell_no, cell)
                },
                _ => {
                    unimplemented!()
                }
            }
        } else {
            self.cells
                .iter_mut()
                .find(|(_, nalcell)| nalcell.is_border())
                .map(|(cell_no, cell)| (*cell_no, cell))
                .ok_or::<Error>(RackError::Boundary { func_name: _f }.into())?
        };
        {
            { 
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell" };
                let trace = json!({ "cell_id": {"name": "Rack"}, "cell_no": cell_no});
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
       let (port, port_from_ca) = cell.get_free_boundary_port_mut()?;
        port.listen_noc_and_ca(SimulatedBorderPort::new(port.clone(), port_to_noc, port_from_noc), port_from_ca)?;
        if CONFIG.replay {
            println!("Connecting NOC to border cell {} for replay", cell.get_id());
        } else {
            println!("Connecting NOC to border cell {}", cell.get_id());
        }
        Ok(())
    }
}
impl fmt::Display for Rack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nLinks\n");
        for (_edge, link) in &self.links {
            write!(s, "  {}\n", link)?;
        }
        s = s + "\nCells";
        for i in 0..self.cells.len() {
            if i < 30 { write!(s, "\n{}\n", self.cells[&CellNo(i)])?; }
        }
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum RackError {
    #[fail(display = "RackError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "RackError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "RackError::Cells {}: The number of cells {:?} must be at least 1", func_name, num_cells)]
    Cells { num_cells: CellQty, func_name: &'static str },
    #[fail(display = "RackError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkQty, func_name: &'static str },
    #[fail(display = "RackError::Wire {}: {:?} is not a valid edge at {}", func_name, edge, comment)]
    Wire { edge: Edge, func_name: &'static str, comment: &'static str }
}
