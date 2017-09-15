use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;
use std::thread::{JoinHandle, spawn};

use config::{MIN_BOUNDARY_CELLS, CellNo, Edge, LinkNo, PortNo};
use message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
	NocToPort, NocFromPort, PortToNoc, PortFromNoc};
use link::{Link};
use nalcell::{CellType, NalCell};
use name::{CellID, LinkID, PortID, UpTraphID};
use noc::Noc;
use utility::S;

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
		let f = "initialize";
		if ncells.0 < 1  { return Err(ErrorKind::Cells(ncells, S(f)).into()); }
		if edge_list.len() < ncells.0 - 1 { return Err(ErrorKind::Edges(LinkNo(CellNo(edge_list.len())), S(f)).into()); }
		for i in 0..ncells.0 {
			let is_border = (i % 3) == 1;
			let cell = NalCell::new(CellNo(i), nports, is_border, cell_type).chain_err(|| ErrorKind::NalCell(i, S(f)))?;
			self.cells.push(cell);
		}
		let mut link_handles = Vec::new();
		for edge in edge_list {
			if *(edge.v.0) == *(edge.v.1) { return Err(ErrorKind::Wire(edge, S(f)).into()); }
			if (*(edge.v.0) > ncells.0) | (*(edge.v.1) >= ncells.0) { return Err(ErrorKind::Wire(edge, S(f)).into()); }
			let split = self.cells.split_at_mut(max(*(edge.v.0),*(edge.v.1)));
			let mut cell = match split.0.get_mut(*(edge.v.0)) {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge, S(f)).into())

			};
			let cell_id = cell.get_id().clone();
			let (left,left_from_pe) = cell.get_free_ec_port_mut().chain_err(|| ErrorKind::FreeECPort(cell_id.clone(), S(f)))?;
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge, S(f)).into())
			};
			let (rite, rite_from_pe) = cell.get_free_ec_port_mut().chain_err(|| ErrorKind::FreeECPort(cell_id, S(f)))?;
			//println!("Datacenter: edge {:?}", edge);
			let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
			let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
			let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
			let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
			left.link_channel(left_to_link, left_from_link, left_from_pe);
			rite.link_channel(rite_to_link, rite_from_link, rite_from_pe);
			let link = Link::new(&left.get_id(), &rite.get_id()).chain_err(|| ErrorKind::LinkCreate(left.get_id().clone(), rite.get_id().clone(), S(f)))?;
			let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite).chain_err(|| ErrorKind::Link(link.get_id().clone(), S(f)))?;
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
		let f = "connect_to_noc";
		let mut boundary_cells = self.get_boundary_cells();
		if boundary_cells.len() < *MIN_BOUNDARY_CELLS {
			return Err(ErrorKind::Boundary("connect_to_noc".to_string()).into());
		} else {
			let (mut boundary_cell, _) = boundary_cells.split_at_mut(1);
			let cell_id = boundary_cell[0].get_id().clone();
			let (port, port_from_pe) = boundary_cell[0].get_free_boundary_port_mut().chain_err(|| ErrorKind::FreeBorderPort(cell_id, S(f)))?;
			port.outside_channel(port_to_noc, port_from_noc, port_from_pe).chain_err(|| ErrorKind::Port(port.get_id().clone(), S(f)))?;
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
	errors { 
		Boundary(func_name: String) {
			display("Datacenter {}: No boundary cells found", func_name)
		}
		Cells(n: CellNo, func_name: String) {
			display("Datacenter {}: The number of cells {} must be at least 1", func_name, n.0)
		}
		Edges(nlinks: LinkNo, func_name: String) {
			display("Datacenter {}: {} is not enough links to connect all cells", func_name, (nlinks.0).0)
		}
		FreeECPort(cell_id: CellID, func_name: String) {
			display("Datacenter {}: No free EC port on cell {}", func_name, cell_id)
		}
		FreeBorderPort(cell_id: CellID, func_name: String) {
			display("Datacenter {}: No free EC port on cell {}", func_name, cell_id)
		}
		LinkCreate(left_id: PortID, rite_id: PortID, func_name: String) {
			display("Datacenter {}: Problem connecting port {} to port {}", func_name, left_id, rite_id)
		}
		Link(link_id: LinkID, func_name: String) {
			display("Datacenter {}: Problem connecting link {}", func_name, link_id)
		}
		NalCell(i: usize, func_name: String) {
			display("Datacenter {}: Can't create cell {}", func_name, i)
		}
		Port(port_id: PortID, func_name: String) {
			display("Datacenter {}: Problem on port {}", func_name, port_id)
		}
		Wire(edge: Edge, func_name: String) {
			display("Datacenter {}: {:?} is not a valid edge", func_name, edge)
		}
	}
}
