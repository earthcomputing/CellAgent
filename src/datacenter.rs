use multi_mut::{HashMapMultiMut};

use std::{fmt, fmt::Write,
          cmp::max,
          collections::{HashMap, HashSet},
          iter::FromIterator,
          sync::mpsc::channel,
          thread::{JoinHandle}};

use crate::app_message_formats::{ApplicationFromNoc, ApplicationToNoc, NocFromApplication, NocToApplication,
                                 PortToNoc, PortFromNoc};
use crate::blueprint::{Blueprint, Cell};
use crate::config::{TRACE_OPTIONS, CellNo, CellQty, CellType, CellConfig, PortNo, PortQty, Edge, LinkNo, get_geometry};
use crate::dal;
use crate::ec_message_formats::{LinkToPort, PortFromLink, PortToLink, LinkFromPort, PortFromPe};
use crate::link::{Link};
use crate::nalcell::{NalCell};
use crate::name::{CellID, LinkID};
use crate::noc::{Noc};
use crate::port::{Port};
use crate::utility::{S, TraceHeaderParams, TraceType};

#[derive(Debug)]
pub struct Datacenter {
    cells: HashMap<CellNo, NalCell>,
    links: HashMap<Edge, Link>,
}
impl Datacenter {
    pub fn new() -> Datacenter { Datacenter { cells: HashMap::new(), links: HashMap::new() } }
    pub fn construct(num_cells: CellQty, edges: &Vec<Edge>, default_num_phys_ports_per_cell: PortQty, cell_port_exceptions: &HashMap<CellNo, PortQty>, border_cell_ports: &HashMap<CellNo, Vec<PortNo>>) -> Result<(Datacenter, ApplicationToNoc, ApplicationFromNoc), Error> {
        println!("\nMain: {} ports for each of {} cells", *default_num_phys_ports_per_cell, *num_cells);
        let blueprint = Blueprint::new(num_cells, &edges, default_num_phys_ports_per_cell, &cell_port_exceptions, border_cell_ports)?;
        println!("{}", blueprint);
        let (mut dc, join_handles) = build_datacenter(&blueprint).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        let (application_to_noc, noc_from_application): (ApplicationToNoc, NocFromApplication) = channel();
        let (noc_to_application, application_from_noc): (NocToApplication, ApplicationFromNoc) = channel();
        let mut noc = Noc::new(noc_to_application)?;
        let (port_to_noc, port_from_noc) = noc.initialize(&blueprint, noc_from_application)?;
        dc.connect_to_noc(port_to_noc, port_from_noc).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        return Ok((dc, application_to_noc, application_from_noc));
    }
    pub fn initialize(&mut self, blueprint: &Blueprint)  -> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "initialize";
        let num_cells = blueprint.get_ncells();
        let geometry = get_geometry(num_cells);  // A cheat used for visualization
        let edge_list = blueprint.get_edge_list();
        if *num_cells < 1  { return Err(DatacenterError::Cells{ num_cells, func_name: _f }.into()); }
        if edge_list.len() < *num_cells - 1 { return Err(DatacenterError::Edges { nlinks: LinkNo(CellNo(edge_list.len())), func_name: _f }.into() ); }
        let labeled_border_nal_cells  =
            blueprint
            .get_border_cells()
            .iter()
            .map(|border_cell| -> Result<(CellNo, NalCell), Error> {
                let cell_no = border_cell.get_cell_no();
                let nal_cell = NalCell::new(&border_cell.get_name(), border_cell.get_num_phys_ports(),
                                            &HashSet::from_iter(border_cell.get_border_ports().clone()),
                                            CellConfig::Large)?;
                {
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.dc {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell_start" };
                        let cell_id = nal_cell.get_id();
                        let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location":  geometry.2.get(*cell_no)});
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                Ok((cell_no, nal_cell))
            })
            .collect::<Result<Vec<(CellNo, NalCell)>, Error>>().context(DatacenterError::Chain { func_name: "initialize", comment: S("border")})?;
        for (cell_no, border_nal_cell) in labeled_border_nal_cells {
            self.cells.insert(cell_no, border_nal_cell);
        }
        let labeled_interior_nal_cells =
            blueprint
            .get_interior_cells()
            .iter()
            .map(|interior_cell| -> Result<(CellNo, NalCell), Error> {
                let cell_no = interior_cell.get_cell_no();
                let nal_cell = NalCell::new(&interior_cell.get_name(), interior_cell.get_num_phys_ports(),
                                            &HashSet::new(), CellConfig::Large)?;
                {
                    if TRACE_OPTIONS.all || TRACE_OPTIONS.dc {
                        let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "interior_cell_start" };
                        let cell_id = nal_cell.get_id();
                        let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location": geometry.2.get(*cell_no as usize) });
                        let _ = dal::add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                    }
                }
                Ok((cell_no, nal_cell))
            })
            .collect::<Result<Vec<(CellNo, NalCell)>, Error>>().context(DatacenterError::Chain { func_name: "initialize", comment: S("interior")})?;
        for (cell_no, interior_nal_cell) in labeled_interior_nal_cells {
            self.cells.insert(cell_no, interior_nal_cell);
        }
        let mut link_handles = Vec::new();
        for edge in edge_list {
            if (*(edge.0) > num_cells.0) | (*(edge.1) >= num_cells.0) { return Err(DatacenterError::Wire { edge: *edge, func_name: _f, comment: "greater than num_cells test" }.into()); }
            let (e0, e1) = if *(edge.0) >= *(edge.1) {
                (*(edge.1), *(edge.0))
            } else {
                (*(edge.0), *(edge.1))
            };
            let (left_cell, rite_cell) = self.cells
                .get_pair_mut(&edge.0, &edge.1)
                .unwrap();
            let left_cell_id: CellID = left_cell.get_id(); // For Trace
            let left_pair: (&mut Port, PortFromPe) = left_cell.get_free_ec_port_mut()?;
            let left_port: &mut Port = left_pair.0;
            let left_from_pe: PortFromPe = left_pair.1;
            let rite_cell_id: CellID = rite_cell.get_id(); // For Trace
            let rite_pair: (&mut Port, PortFromPe) = rite_cell.get_free_ec_port_mut()?;
            let rite_port: &mut Port = rite_pair.0;
            let rite_from_pe: PortFromPe = rite_pair.1;
                //println!("Datacenter: edge {:?} {} {}", edge, *left_port.get_id(), *rite_port.get_id());
            let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
            let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
            let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
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
            self.links.insert(*edge, link);
        } 
        Ok(link_handles)
    }
    //pub fn get_links(&self) -> &Vec<Link> { &self.links }
    pub fn get_cells(&self) -> &HashMap<CellNo, NalCell> { &self.cells }
    pub fn get_links_mut(&mut self) -> &mut HashMap<Edge, Link> { &mut self.links }
    pub fn get_cell_ids(&self) -> HashMap<CellNo, CellID> {
        self.cells.iter().map(|cell_no_and_cell| (*cell_no_and_cell.0, cell_no_and_cell.1.get_id())).collect::<HashMap<CellNo, _>>()
    }
    pub fn get_link_ids(&self) -> HashMap<Edge, LinkID> {
        self.links.iter().map(|edge_and_link| (*edge_and_link.0, edge_and_link.1.get_id())).collect::<HashMap<Edge,  _>>()
    }
    pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)
            -> Result<(), Error> {
        let _f = "connect_to_noc";
        let (port, port_from_pe) = self.cells
            .iter_mut()
            .filter(|binding| binding.1.is_border())
            .map(|binding| binding.1)
            .nth(0)
            .ok_or::<Error>(DatacenterError::Boundary { func_name: _f }.into())?
            .get_free_boundary_port_mut()?;
        port.noc_channel(port_to_noc, port_from_noc, port_from_pe)?;
        Ok(())
    }
}
fn build_datacenter(blueprint: &Blueprint) -> Result<(Datacenter, Vec<JoinHandle<()>>), Error> {
    let mut dc = Datacenter::new();
    let join_handles = dc.initialize(blueprint).context(DatacenterError::Chain { func_name: "build_datacenter", comment: S("")})?;
    Ok((dc, join_handles))
}
impl fmt::Display for Datacenter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Links");
        for (edge, link) in &self.links {
            write!(s, "{}", link)?;
        }
        s = s + "\nCells";
        for i in 0..self.cells.len() {
            if i < 30 { write!(s, "\n{}", self.cells[&CellNo(i)])?; }
        }
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "DatacenterError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "DatacenterError::Cells {}: The number of cells {:?} must be at least 1", func_name, num_cells)]
    Cells { num_cells: CellQty, func_name: &'static str },
    #[fail(display = "DatacenterError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Wire {}: {:?} is not a valid edge at {}", func_name, edge, comment)]
    Wire { edge: Edge, func_name: &'static str, comment: &'static str }
}
