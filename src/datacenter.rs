use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;
use std::thread::{JoinHandle, spawn};

use config::{MIN_BOUNDARY_CELLS, CellNo, Edge, LinkNo, PortNo};
use message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
	NocToPort, NocFromPort, PortToNoc, PortFromNoc};
use link::{Link};
use nalcell::{CellType, NalCell};
use name::UpTraphID;
use noc::Noc;

#[derive(Debug)]
pub struct Datacenter {
	id: UpTraphID,
	cell_type: CellType,
	cells: Vec<NalCell>,
	links: Vec<Link>,
}
impl Datacenter {
	pub fn new(id: &UpTraphID, cell_type: CellType) -> Datacenter {
		Datacenter { id: id.clone(), cell_type: cell_type, cells: Vec::new(), links: Vec::new() }
	}
	pub fn initialize(&mut self, ncells: CellNo, nports: PortNo, edge_list: Vec<Edge>,
			cell_type: CellType) -> Result<Vec<JoinHandle<()>>> {
		if ncells.0 < 1  { return Err(ErrorKind::Cells(ncells, "initialize".to_string()).into()); }
		if edge_list.len() < ncells.0 - 1 { return Err(ErrorKind::Edges(LinkNo(CellNo(edge_list.len())), "initialize".to_string()).into()); }
		for i in 0..ncells.0 {
			let is_border = (i % 3) == 1;
			let cell = NalCell::new(CellNo(i), nports, is_border, cell_type).chain_err(|| ErrorKind::DatacenterError)?;
			self.cells.push(cell);
		}
		let mut link_handles = Vec::new();
		for edge in edge_list {
			if *(edge.v.0) == *(edge.v.1) { return Err(ErrorKind::Wire(edge, "initialize".to_string()).into()); }
			if (*(edge.v.0) > ncells.0) | (*(edge.v.1) >= ncells.0) { return Err(ErrorKind::Wire(edge, "initialize".to_string()).into()); }
			let split = self.cells.split_at_mut(max(*(edge.v.0),*(edge.v.1)));
			let mut cell = match split.0.get_mut(*(edge.v.0)) {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge, "initialize".to_string()).into())

			};
			let (left,left_from_pe) = cell.get_free_ec_port_mut().chain_err(|| ErrorKind::DatacenterError)?;
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge, "initialize".to_string()).into())
			};
			let (rite, rite_from_pe) = cell.get_free_ec_port_mut().chain_err(|| ErrorKind::DatacenterError)?;
			//println!("Datacenter: edge {:?}", edge);
			let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
			let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
			let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
			let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
			left.link_channel(left_to_link, left_from_link, left_from_pe).chain_err(|| ErrorKind::DatacenterError)?;
			rite.link_channel(rite_to_link, rite_from_link, rite_from_pe).chain_err(|| ErrorKind::DatacenterError)?;
			let link = Link::new(&left.get_id(), &rite.get_id())?;
			let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
			link_handles.append(&mut handle_pair);
			self.links.push(link); 
		} 
		Ok(link_handles)
	}
	pub fn get_cells(&self) -> &Vec<NalCell> { &self.cells }
	fn get_boundary_cells(&mut self) -> Vec<&mut NalCell> {
		let mut boundary_cells = Vec::new();
		for cell in &mut self.cells {
			if cell.is_border() { boundary_cells.push(cell); }
		}
		boundary_cells
	}
	pub fn connect_to_noc(&mut self, port_to_noc: PortToNoc, port_from_noc: PortFromNoc)  
			-> Result<()> {
		let mut boundary_cells = self.get_boundary_cells();
		if boundary_cells.len() < *MIN_BOUNDARY_CELLS {
			return Err(ErrorKind::Boundary("connect_to_noc".to_string()).into());
		} else {
			let (mut boundary_cell, _) = boundary_cells.split_at_mut(1);
			let (port, port_from_pe) = boundary_cell[0].get_free_boundary_port_mut()?;
			port.outside_channel(port_to_noc, port_from_noc, port_from_pe)?;
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
			if i < 30 { s = s + &format!("\n{}", self.cells[i]); }
		}
		write!(f, "{}", s) 
	}
}
// Errors
error_chain! {
	links {
		Link(::link::Error, ::link::ErrorKind);
		NalCell(::nalcell::Error, ::nalcell::ErrorKind);
		Port(::port::Error, ::port::ErrorKind);
	}
	errors { DatacenterError
		Boundary(fn_name: String) {
			display("{}: Datacenter: No boundary cells found", fn_name)
		}
		Cells(n: CellNo, fn_name: String) {
			display("{}: Datacenter: The number of cells {} must be at least 1", fn_name, n.0)
		}
		Edges(nlinks: LinkNo, fn_name: String) {
			display("{}: Datacenter: {} is not enough links to connect all cells", fn_name, (nlinks.0).0)
		}
		Wire(edge: Edge, fn_name: String) {
			display("{}: Datacenter: {:?} is not a valid edge", fn_name, edge)
		}
	}
}
