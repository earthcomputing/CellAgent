use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;

use config::{PortNo, CellNo};
use message_types::{LinkToPort, PortFromLink, PortToLink, LinkFromPort,
	OutsideToPort, OutsideFromPort, PortToOutside, PortFromOutside};
use link::{Link};
use nalcell::{NalCell};
use noc::Noc;

type Edge = (usize, usize);

#[derive(Debug)]
pub struct Datacenter {
	cells: Vec<NalCell>,
	links: Vec<Link>,
	noc:   Noc
}
impl Datacenter {
	pub fn new(ncells: CellNo, nports: PortNo, edge_list: Vec<(CellNo,CellNo)>) -> 
				Result<Datacenter> {
		if ncells < 2  {
			println!("ncells {}", ncells);
			return Err(ErrorKind::CellsSize(ncells).into());
		}
		if edge_list.len() < ncells - 1 {
			println!("nlinks {}", edge_list.len());
			return Err(ErrorKind::LinksSize(edge_list.len()).into());			
		}
		let mut cells = Vec::new();
		for i in 0..ncells {
			let is_border = (i % 3) == 1;
			let cell = NalCell::new(i, nports, is_border).chain_err(|| ErrorKind::DatacenterError)?;
			cells.push(cell);
		}
		let mut links: Vec<Link> = Vec::new();
		for edge in edge_list {
			if edge.0 == edge.1 { return Err(ErrorKind::Wire(edge).into()); }
			if (edge.0 > ncells) | (edge.1 >= ncells) { return Err(ErrorKind::Wire(edge).into()); }
			let split = cells.split_at_mut(max(edge.0,edge.1));
			let mut cell = match split.0.get_mut(edge.0) {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge).into())

			};
			let (left,left_from_pe) = cell.get_free_ec_port_mut().chain_err(|| ErrorKind::DatacenterError)?;
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge).into())
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
			let link_handles = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
			links.push(link); 
		} 
		let (outside_to_primary, primary_from_outside): (OutsideToPort, OutsideFromPort) = channel();
		let (primary_to_outside, outside_from_primary): (PortToOutside, PortFromOutside) = channel();
		let noc = Noc::new();
		noc.initialize(outside_to_primary, outside_from_primary)?;
		{
			let mut boundary_cells = Datacenter::get_boundary_cells(&mut cells)?;
			if boundary_cells.len() < 2 {
				return Err(ErrorKind::Boundary.into());
			} else {
				let (mut primary, _) = boundary_cells.split_at_mut(1);
				let (port, port_from_pe) = primary[0].get_free_tcp_port_mut()?;
				port.outside_channel(primary_to_outside, primary_from_outside, port_from_pe)?;
			}
		}
		Ok(Datacenter { cells: cells, links: links, noc: noc })
	}
	pub fn get_cells(&self) -> &Vec<NalCell> { &self.cells }
	fn get_boundary_cells(cells: &mut Vec<NalCell>) -> Result<Vec<&mut NalCell>> {
		let mut boundary_cells = Vec::new();
		for cell in cells {
			if cell.is_border() { boundary_cells.push(cell); }
		}
		if boundary_cells.len() == 0 {
			Err(ErrorKind::Boundary.into())
		} else {
			Ok(boundary_cells)
		}
	}
				//	pub fn add_noc(&mut self, control: &'a NalCell, backup: &'a NalCell) {
//		self.noc = Some(NOC::new(control, backup));
//	}
}
impl fmt::Display for Datacenter { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Links");
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		s = s + "\nCells";
		for i in 0..self.cells.len() {			
			if i < 3 { s = s + &format!("\n{}", self.cells[i]); }
		}
		write!(f, "{}", s) 
	}
}
// Errors
error_chain! {
	links {
		Link(::link::Error, ::link::ErrorKind);
		NalCell(::nalcell::Error, ::nalcell::ErrorKind);
		Noc(::noc::Error, ::noc::ErrorKind);
		Port(::port::Error, ::port::ErrorKind);
	}
	errors { DatacenterError
		Boundary {
			description("No boundary cells")
		}
		CellsSize(n: usize) {
			description("Not enough cells")
			display("The number of cells {} must be at least 2", n)
		}
		LinksSize(nlinks: usize) {
			description("Not enough cells")
			display("{} is not enough links", nlinks)
		}
		Wire(edge: Edge) {
			description("Invalid edge")
			display("{:?} is not a valid edge", edge)
		}
	}
}
