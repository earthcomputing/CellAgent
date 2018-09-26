use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;
use std::thread::{JoinHandle};

use blueprint::{Blueprint};
use config::{MIN_BOUNDARY_CELLS, CellNo, CellType, Edge, LinkNo};
use dal;
use message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
	PortToNoc, PortFromNoc};
use link::{Link};
use nalcell::{CellConfig, NalCell};
use name::{CellID, LinkID};
use utility::{TraceHeader, TraceHeaderParams, TraceType};

const MODULE: &'static str = "datacenter.rs";

#[derive(Debug)]
pub struct Datacenter {
	cells: Vec<NalCell>,
	links: Vec<Link>,
}
impl Datacenter {
	pub fn new() -> Datacenter { Datacenter { cells: Vec::new(), links: Vec::new() } }
	pub fn initialize(&mut self, blueprint: &Blueprint, trace_header: &mut TraceHeader)
            -> Result<Vec<JoinHandle<()>>, Error> {
		let f = "initialize";
		let ncells = blueprint.get_ncells();
		let edge_list = blueprint.get_edge_list();
		if *ncells < 1  { return Err(DatacenterError::Cells{ ncells, func_name: f }.into()); }
		if edge_list.len() < *ncells - 1 { return Err(DatacenterError::Edges { nlinks: LinkNo(CellNo(edge_list.len())), func_name: f }.into() ); }
		let border_cells = blueprint.get_border_cells();
		for border_cell in border_cells {
			let cell = NalCell::new(border_cell.get_cell_no(), border_cell.get_nports(),
                                    CellType::Border,CellConfig::Large,
                                    trace_header.fork_trace())?;
            {
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "border_cell_start" };
                let trace = json!({ "cell_number": border_cell.get_cell_no() });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params,&trace, f);
            }
			self.cells.push(cell);
		}
		let interior_cells = blueprint.get_interior_cells();
		for interior_cell in interior_cells {
			let cell = NalCell::new(interior_cell.get_cell_no(), interior_cell.get_nports(),
                                    CellType::Interior,CellConfig::Large,
                                    trace_header.fork_trace())?;
            {
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "interior_cell_start" };
                let trace = json!({"cell_number": interior_cell.get_cell_no() });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, f);
            }
			self.cells.push(cell);
		}
		self.cells.sort_by(|a, b| (*a.get_no()).cmp(&*b.get_no())); // Sort to conform to edge list
		let mut link_handles = Vec::new();
		for edge in edge_list {
            if (*(edge.0) > ncells.0) | (*(edge.1) >= ncells.0) { return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f, comment: "greater than ncells test" }.into()); }
			let (e0, e1) = if *(edge.0) >= *(edge.1) {
                (*(edge.1), *(edge.0))
            } else {
                (*(edge.0), *(edge.1))
            };
			let split = self.cells.split_at_mut(max(e0,e1));
			let left_cell = match split.0.get_mut(e0) {
				Some(c) => c,
				None => return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f, comment: "split left" }.into())

			};
            let left_cell_id = left_cell.get_id().clone(); // For Trace
			let (left_port,left_from_pe) = left_cell.get_free_ec_port_mut()?;
			let rite_cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f, comment: "split rite" }.into())
			};
            let rite_cell_id = rite_cell.get_id().clone(); // For Trace
			let (rite_port, rite_from_pe) = rite_cell.get_free_ec_port_mut()?;
			//println!("Datacenter: edge {:?} {} {}", edge, *left_port.get_id(), *rite_port.get_id());
			let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
			let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
			let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
			let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
			left_port.link_channel(left_to_link, left_from_link, left_from_pe);
			rite_port.link_channel(rite_to_link, rite_from_link, rite_from_pe);
			let mut link = Link::new(&left_port.get_id(), &rite_port.get_id())?;
            {
                let ref trace_params = TraceHeaderParams { module: MODULE, function: f, format: "connect_link" };
                let trace = json!({ "left_cell": left_cell_id, "rite_cell": rite_cell_id, "left_port": left_port.get_port_no(), "rite_port": rite_port.get_port_no(), "link_id": link.get_id() });
                let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params, &trace, f);
            }
			let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
			link_handles.append(&mut handle_pair);
			self.links.push(link); 
		} 
		Ok(link_handles)
	}
    pub fn get_links(&self) -> &Vec<Link> { &self.links }
    pub fn get_links_mut(&mut self) -> &mut Vec<Link> { &mut self.links }
	pub fn get_cell_ids(&self) -> Vec<&CellID> {
        self.cells.iter().map(|cell| cell.get_id()).collect::<Vec<_>>()
    }
	pub fn get_link_ids(&self) -> Vec<&LinkID> {
        self.links.iter().map(|link| link.get_id()).collect::<Vec<_>>()
    }
	pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)
			-> Result<(), Error> {
        let _f = "connect_to_noc";
        let (port, port_from_pe) = self.cells
            .iter_mut()
            .filter(|cell| cell.is_border())
            .nth(0)
            .ok_or_else(|| -> Error { DatacenterError::Boundary { func_name: _f }.into() })?
            .get_free_boundary_port_mut()?;
		port.noc_channel(port_to_noc, port_from_noc, port_from_pe)?;
		Ok(())
	}
}
impl fmt::Display for Datacenter {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Links");
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		s = s + "\nCells";
		for i in 0..self.cells.len() {
			if i < 30 { s = s + &format!("\n{}", self.cells.get(i).unwrap()); }
		}
		write!(f, "{}", s)
	}
}
// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "DatacenterError::Cells {}: The number of cells {:?} must be at least 1", func_name, ncells)]
    Cells { ncells: CellNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Wire {}: {:?} is not a valid edge at {}", func_name, edge, comment)]
    Wire { edge: Edge, func_name: &'static str, comment: &'static str }
}
