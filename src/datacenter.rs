use std::fmt;
use std::cmp::max;
use crossbeam::Scope;

use config::{PortNo, CellNo};
use nalcell::{NalCell};
use link::{Link};
use noc::NOC;

#[derive(Debug)]
pub struct Datacenter<'a> {
	cells: Vec<NalCell>,
	links: Vec<Link>,
	noc: Option<NOC<'a>>
}
#[deny(unused_must_use)]
impl<'a> Datacenter<'a> {
	pub fn new(scope: &Scope, ncells: CellNo, nports: PortNo, edge_list: Vec<(CellNo,CellNo)>) -> 
				Result<Datacenter<'a>,DatacenterError> {
		if ncells < 2  {
			println!("ncells {}", ncells);
			return Err(DatacenterError::Size(SizeError::new(ncells)));
		}
		if edge_list.len() < ncells - 1 {
			println!("nlinks {}", edge_list.len());
			return Err(DatacenterError::Size(SizeError::new(edge_list.len())));			
		}
		let mut cells = Vec::new();
		for i in 0..ncells {
			let mut is_border = false;
			if i % 3 == 1 { is_border = true; }
			let cell = try!(NalCell::new(scope, i, nports, is_border));
			cells.push(cell);
		}	
		let mut links: Vec<Link> = Vec::new();
		for edge in edge_list {
			if edge.0 == edge.1 { return Err(DatacenterError::Wire(WireError::new(edge))); }
			if (edge.0 > ncells) | (edge.1 >= ncells) { return Err(DatacenterError::Wire(WireError::new(edge))); }
			let split = cells.split_at_mut(max(edge.0,edge.1));
			let mut cell = match split.0.get_mut(edge.0) {
				Some(c) => c,
				None => return Err(DatacenterError::Wire(WireError::new(edge)))

			};
			let mut p1 = cell.get_free_port_mut()?;
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(DatacenterError::Wire(WireError::new(edge)))
			};
			let mut p2 = cell.get_free_port_mut()?;
			//println!("Datacenter: edge {:?}", edge);
			links.push(try!(Link::new(scope, p1, p2)));
		} 
		Ok(Datacenter { cells: cells, links: links, noc: None })
	}
//	pub fn add_noc(&mut self, control: &'a NalCell, backup: &'a NalCell) {
//		self.noc = Some(NOC::new(control, backup));
//	}
}
impl<'b> fmt::Display for Datacenter<'b> { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Cells");
		for i in 0..self.cells.len() {			
			if i < 3 { s = s + &format!("\n {}", self.cells[i]); }
		}
		s = s + "\nLinks";
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		write!(f, "{}", s) 
	}
}
// Errors
use std::error::Error;
use link::LinkError;
use nalcell::NalCellError;
use name::NameError;
#[derive(Debug)]
pub enum DatacenterError {
	Name(NameError),
	Link(LinkError),
	Cell(NalCellError),
	Wire(WireError),
	Size(SizeError),
}
impl Error for DatacenterError {
	fn description(&self) -> &str {
		match *self {
			DatacenterError::Name(ref err) => err.description(),
			DatacenterError::Link(ref err) => err.description(),
			DatacenterError::Cell(ref err) => err.description(),
			DatacenterError::Wire(ref err) => err.description(),
			DatacenterError::Size(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			DatacenterError::Name(ref err) => Some(err),
			DatacenterError::Link(ref err) => Some(err),
			DatacenterError::Cell(ref err) => Some(err),
			DatacenterError::Wire(ref err) => Some(err),
			DatacenterError::Size(ref err) => Some(err),
		}
	}
}
impl fmt::Display for DatacenterError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			DatacenterError::Name(ref err) => write!(f, "Link Name Error caused by {}", err),
			DatacenterError::Link(ref err) => write!(f, "Link Error caused by {}", err),
			DatacenterError::Cell(ref err) => write!(f, "Cell Error caused by {}", err),
			DatacenterError::Wire(ref err) => write!(f, "Wire Error caused by {}", err),
			DatacenterError::Size(ref err) => write!(f, "Size Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct WireError { msg: String }
impl WireError { 
	pub fn new(wire: (usize,usize)) -> WireError {
		WireError { msg: format!("Wire {:?} is incorrect", wire) }
	}
}
impl Error for WireError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for WireError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<WireError> for DatacenterError {
	fn from(err: WireError) -> DatacenterError { DatacenterError::Wire(err) }
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError { 
	pub fn new(n: usize) -> SizeError {
		let msg = format!("Problem splitting cells array at {}", n);
		SizeError { msg: msg }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<SizeError> for DatacenterError {
	fn from(err: SizeError) -> DatacenterError { DatacenterError::Size(err) }
}
impl From<NameError> for DatacenterError {
	fn from(err: NameError) -> DatacenterError { DatacenterError::Name(err) }
}
impl From<LinkError> for DatacenterError {
	fn from(err: LinkError) -> DatacenterError { DatacenterError::Link(err) }
}
impl From<NalCellError> for DatacenterError {
	fn from(err: NalCellError) -> DatacenterError { DatacenterError::Cell(err) }
}
