use std::fmt;
use std::cmp::max;
use std::sync::mpsc::channel;
use crossbeam::Scope;

use config::{PortNo, CellNo};
use nalcell::{NalCell, LinkToPort, PortFromLink, PortToLink, LinkFromPort};
use link::{Link};
use noc::NOC;

type Edge = (usize, usize);

#[derive(Debug)]
pub struct Datacenter<'a> {
	cells: Vec<NalCell>,
	links: Vec<Link>,
	noc: Option<NOC<'a>>
}
impl<'a> Datacenter<'a> {
	pub fn new(scope: &Scope, ncells: CellNo, nports: PortNo, edge_list: Vec<(CellNo,CellNo)>) -> 
				Result<Datacenter<'a>> {
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
			let mut is_border = false;
			if i % 3 == 1 { is_border = true; }
			let cell = NalCell::new(scope, i, nports, is_border).chain_err(|| ErrorKind::DatacenterError)?;
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
			let (left,left_from_pe) = cell.get_free_port_mut().chain_err(|| ErrorKind::DatacenterError)?;
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(ErrorKind::Wire(edge).into())
			};
			let (rite, rite_from_pe) = cell.get_free_port_mut().chain_err(|| ErrorKind::DatacenterError)?;
			//println!("Datacenter: edge {:?}", edge);
			let (link_to_left, left_from_link): (LinkToPort, PortFromLink) = channel();
			let (link_to_rite, rite_from_link): (LinkToPort, PortFromLink) = channel();
			let (left_to_link, link_from_left): (PortToLink, LinkFromPort) = channel();
			let (rite_to_link, link_from_rite): (PortToLink, LinkFromPort) = channel();
			left.link_channel(scope, left_to_link, left_from_link, left_from_pe).chain_err(|| ErrorKind::DatacenterError)?;
			rite.link_channel(scope, rite_to_link, rite_from_link, rite_from_pe).chain_err(|| ErrorKind::DatacenterError)?;
			links.push(Link::new(scope, &left.get_id(), &rite.get_id(), 
				link_to_left, link_from_left, link_to_rite, link_from_rite).chain_err(|| ErrorKind::DatacenterError)?);
		} 
		Ok(Datacenter { cells: cells, links: links, noc: None })
	}
	pub fn get_cells(&self) -> &Vec<NalCell> { &self.cells }
				//	pub fn add_noc(&mut self, control: &'a NalCell, backup: &'a NalCell) {
//		self.noc = Some(NOC::new(control, backup));
//	}
}
impl<'b> fmt::Display for Datacenter<'b> { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Links");
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		s = s + "\nCells";
		for i in 0..self.cells.len() {			
			if i < 50 { s = s + &format!("\n{}", self.cells[i]); }
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
