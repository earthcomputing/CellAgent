use std::{fmt, fmt::Write,
          cmp::max,
          collections::{HashMap, HashSet},
          iter::FromIterator,
          sync::mpsc::channel,
          thread::{JoinHandle}};

use crate::blueprint::{Blueprint, Cell};
use crate::config::{TRACE_OPTIONS, CellNo, CellQty, CellType, PortNo, PortQty, Edge, LinkNo, get_geometry};
use crate::dal;
use crate::message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
    PortToNoc, PortFromNoc};
use crate::link::{Link};
use crate::message_types::{OutsideFromNoc, OutsideToNoc, NocFromOutside, NocToOutside};
use crate::nalcell::{CellConfig, NalCell};
use crate::name::{CellID, LinkID};
use crate::noc::Noc;
use crate::utility::{S, TraceHeaderParams, TraceType};

#[derive(Debug)]
pub struct Datacenter {
    cells: Vec<NalCell>,
    links: Vec<Link>,
}
impl Datacenter {
    pub fn new() -> Datacenter { Datacenter { cells: Vec::new(), links: Vec::new() } }
    pub fn construct(num_cells: CellQty, edges: &Vec<Edge>, default_num_ports_per_cell: PortQty, cell_port_exceptions: &HashMap<CellNo, PortQty>, border_cell_ports: &HashMap<CellNo, Vec<PortNo>>) -> Result<(Datacenter, OutsideToNoc), Error> {
        /* Doesn't work when debugging in Eclipse
        let args: Vec<String> = env::args().collect();
        println!("Main: args {:?}",args);
         */
        println!("\nMain: {} ports for each of {} cells", *default_num_ports_per_cell, *num_cells);
        let blueprint = Blueprint::new(num_cells, &edges, default_num_ports_per_cell, &cell_port_exceptions, border_cell_ports)?;
        println!("{}", blueprint);
        let (outside_to_noc, noc_from_outside): (OutsideToNoc, NocFromOutside) = channel();
        let (noc_to_outside, _outside_from_noc): (NocToOutside, OutsideFromNoc) = channel();
        let mut noc = Noc::new(noc_to_outside)?;
        let (dc, _) = noc.initialize(&blueprint, noc_from_outside)?;
        return Ok((dc, outside_to_noc));
    }
    pub fn initialize(&mut self, blueprint: &Blueprint)  -> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "initialize";
        let num_cells = blueprint.get_ncells();
        let geometry = get_geometry(num_cells);  // A cheat used for visualization
        let edge_list = blueprint.get_edge_list();
        if *num_cells < 1  { return Err(DatacenterError::Cells{ num_cells, func_name: _f }.into()); }
        if edge_list.len() < *num_cells - 1 { return Err(DatacenterError::Edges { nlinks: LinkNo(CellNo(edge_list.len())), func_name: _f }.into() ); }
        self.cells.append(&mut blueprint.get_border_cells()
                          .iter()
                          .map(|border_cell| -> Result<NalCell, Error> {
                              let nalcell = NalCell::new(border_cell.get_cell_no(), border_cell.get_nports(),
                                                         &HashSet::from_iter(border_cell.get_border_ports().clone()),
                                                         CellType::Border, CellConfig::Large)?;
                              {
                                  if TRACE_OPTIONS.all || TRACE_OPTIONS.dc {
                                      let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell_start" };
                                      let cell_no = border_cell.get_cell_no();
                                      let cell_id = nalcell.get_id();
                                      let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location":  geometry.2.get(*cell_no)});
                                      let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                  }
                              }
                              Ok(nalcell)
                          })
                          .filter(|cell| cell.is_ok())
                          .map(|cell| cell.unwrap())
                          .collect::<Vec<_>>());
        self.cells.append(&mut blueprint.get_interior_cells()
                          .iter()
                          .map(|interior_cell| -> Result<NalCell, Error> {
                              let nalcell = NalCell::new(interior_cell.get_cell_no(), interior_cell.get_nports(),
                                                         &HashSet::new(), CellType::Interior, CellConfig::Large)?;
                              {
                                  if TRACE_OPTIONS.all || TRACE_OPTIONS.dc {
                                      let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "interior_cell_start" };
                                      let cell_no = interior_cell.get_cell_no();
                                      let cell_id = nalcell.get_id();
                                      let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location": geometry.2.get(*cell_no as usize) });
                                      let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                                  }
                              }
                              Ok(nalcell)
                          })
                          .filter(|cell| cell.is_ok())
                          .map(|cell| cell.unwrap())
                          .collect::<Vec<_>>());
        self.cells.sort_by(|a, b| (*a.get_no()).cmp(&*b.get_no())); // Sort to conform to edge list
        let mut link_handles = Vec::new();
        for edge in edge_list {
            if (*(edge.0) > num_cells.0) | (*(edge.1) >= num_cells.0) { return Err(DatacenterError::Wire { edge: *edge, func_name: _f, comment: "greater than num_cells test" }.into()); }
            let (e0, e1) = if *(edge.0) >= *(edge.1) {
                (*(edge.1), *(edge.0))
            } else {
                (*(edge.0), *(edge.1))
            };
            let split = self.cells.split_at_mut(max(e0,e1));
            let left_cell = split.0.get_mut(e0)
                .ok_or::<Error>(DatacenterError::Wire { edge: *edge, func_name: _f, comment: "split left" }.into())?;
            let left_cell_id = left_cell.get_id(); // For Trace
            let (left_port,left_from_pe) = left_cell.get_free_ec_port_mut()?;
            let rite_cell = split.1.first_mut()
                .ok_or::<Error>(DatacenterError::Wire { edge: *edge, func_name: _f, comment: "split rite" }.into())?;
            let rite_cell_id = rite_cell.get_id(); // For Trace
            let (rite_port, rite_from_pe) = rite_cell.get_free_ec_port_mut()?;
            //println!("Datacenter: edge {:?} {} {}", edge, *left_port.get_id(), *rite_port.get_id());
            let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
            let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
            let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
            let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
            left_port.link_channel(left_to_link, left_from_link, left_from_pe);
            rite_port.link_channel(rite_to_link, rite_from_link, rite_from_pe);
            let mut link = Link::new(left_port.get_id(), rite_port.get_id())?;
            {
                if TRACE_OPTIONS.all || TRACE_OPTIONS.dc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "connect_link" };
                    let trace = json!({ "left_cell": left_cell_id, "rite_cell": rite_cell_id, "left_port": left_port.get_port_no(), "rite_port": rite_port.get_port_no(), "link_id": link.get_id() });
                    let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
            link_handles.append(&mut handle_pair);
            self.links.push(link); 
        } 
        Ok(link_handles)
    }
    //pub fn get_links(&self) -> &Vec<Link> { &self.links }
    pub fn get_cells(&self) -> &Vec<NalCell> { &self.cells }
    pub fn get_links_mut(&mut self) -> &mut Vec<Link> { &mut self.links }
    pub fn get_cell_ids(&self) -> Vec<CellID> {
        self.cells.iter().map(|cell| cell.get_id()).collect::<Vec<_>>()
    }
    pub fn get_link_ids(&self) -> Vec<LinkID> {
        self.links.iter().map(|link| link.get_id()).collect::<Vec<_>>()
    }
    pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)
            -> Result<(), Error> {
        let _f = "connect_to_noc";
        let (port, port_from_pe) = self.cells
            .iter_mut()
            .filter(|cell| cell.is_border())
            .nth(0)
            .ok_or::<Error>(DatacenterError::Boundary { func_name: _f }.into())?
            .get_free_boundary_port_mut()?;
        port.noc_channel(port_to_noc, port_from_noc, port_from_pe)?;
        Ok(())
    }
}
impl fmt::Display for Datacenter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Links");
        for l in &self.links {
            write!(s, "{}",l)?;
        }
        s = s + "\nCells";
        for i in 0..self.cells.len() {
            if i < 30 { write!(s, "\n{}", self.cells[i])?; }
        }
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "DatacenterError::Cells {}: The number of cells {:?} must be at least 1", func_name, num_cells)]
    Cells { num_cells: CellQty, func_name: &'static str },
    #[fail(display = "DatacenterError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Wire {}: {:?} is not a valid edge at {}", func_name, edge, comment)]
    Wire { edge: Edge, func_name: &'static str, comment: &'static str }
}
