use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;
use std::thread::{JoinHandle};

use failure::{Error, Fail, ResultExt};

use blueprint::{Blueprint};
use config::{MIN_BOUNDARY_CELLS, CellNo, CellType, Edge, LinkNo, PortNo};
use message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
	PortToNoc, PortFromNoc};
use link::{Link};
use nalcell::{CellConfig, NalCell};
use utility::S;

#[derive(Debug)]
pub struct Datacenter {
	cells: Vec<NalCell>,
	links: Vec<Link>,
}
impl Datacenter {
	pub fn new() -> Datacenter {
		Datacenter { cells: Vec::new(), links: Vec::new() }
	}
	pub fn initialize(&mut self, blueprint: &Blueprint) -> Result<Vec<JoinHandle<()>>, Error> {
		let f = "initialize";
		let ncells = blueprint.get_ncells();
		let edge_list = blueprint.get_edge_list();
		if *ncells < 1  { return Err(DatacenterError::Cells{ ncells: ncells, func_name: f }.into()); }
		if edge_list.len() < *ncells - 1 { return Err(DatacenterError::Edges { nlinks: LinkNo(CellNo(edge_list.len())), func_name: f }.into() ); }
		let border_cells = blueprint.get_border_cells();
		for cell in border_cells {
			let cell = NalCell::new(cell.get_cell_no(), cell.get_nports(), CellType::Border, CellConfig::Large)?;
			self.cells.push(cell);
		}
		let interior_cells = blueprint.get_interior_cells();
		for cell in interior_cells {
			let cell = NalCell::new(cell.get_cell_no(), cell.get_nports(), CellType::Interior, CellConfig::Large)?;
			self.cells.push(cell);
		}
		self.cells.sort_by(|a, b| (*a.get_no()).cmp(&*b.get_no())); // Sort to conform to edge list
		let mut link_handles = Vec::new();
		for edge in edge_list {
			if *(edge.v.0) == *(edge.v.1) { return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f }.into()); }
			if (*(edge.v.0) > ncells.0) | (*(edge.v.1) >= ncells.0) { return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f }.into()); }
			let split = self.cells.split_at_mut(max(*(edge.v.0),*(edge.v.1)));
			let cell = match split.0.get_mut(*(edge.v.0)) {
				Some(c) => c,
				None => return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f }.into())

			};
			let (left,left_from_pe) = cell.get_free_ec_port_mut()?;
			let cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(DatacenterError::Wire { edge: edge.clone(), func_name: f }.into())
			};
			let (rite, rite_from_pe) = cell.get_free_ec_port_mut()?;
			//println!("Datacenter: edge {:?} {} {}", edge, *left.get_id(), *rite.get_id());
			let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
			let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
			let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
			let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
			left.link_channel(left_to_link, left_from_link, left_from_pe);
			rite.link_channel(rite_to_link, rite_from_link, rite_from_pe);
			let link = Link::new(&left.get_id(), &rite.get_id())?;
			let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
			link_handles.append(&mut handle_pair);
			self.links.push(link); 
		} 
		Ok(link_handles)
	}
//	pub fn get_cells(&self) -> &Vec<NalCell> { &self.cells }
	fn get_boundary_cells(&mut self) -> Vec<&mut NalCell> {
		let mut boundary_cells = Vec::new();
		for cell in &mut self.cells {
			if cell.is_border() { boundary_cells.push(cell); }
		}
		boundary_cells
	}
	pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)  
			-> Result<(), Error> {
		let mut boundary_cells = self.get_boundary_cells();
		if boundary_cells.len() < *MIN_BOUNDARY_CELLS {
			return Err(DatacenterError::Boundary { func_name: "connect_to_noc" }.into());
		} else {
			let (boundary_cell, _) = boundary_cells.split_at_mut(1);
			let (port, port_from_pe) = boundary_cell[0].get_free_boundary_port_mut()?;
			port.noc_channel(port_to_noc, port_from_noc, port_from_pe)?;
			Ok(())
		}
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
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "DatacenterError::Cells {}: The number of cells {:?} must be at least 1", func_name, ncells)]
    Cells { ncells: CellNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkNo, func_name: &'static str },
    #[fail(display = "DatacenterError::Wire {}: {:?} is not a valid edge", func_name, edge)]
    Wire { edge: Edge, func_name: &'static str }
}
