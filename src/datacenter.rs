use std::fmt;
use crossbeam::Scope;

use nalcell::{NalCell,NalCellError};
use link::{Link,LinkError};
use noc::NOC;

#[derive(Debug)]
pub struct Datacenter<'b> {
	cells: Vec<NalCell>,
	links: Vec<Link>,
	noc: Option<NOC<'b>>
}
impl<'b> Datacenter<'b> {
	pub fn new(scope: &Scope, ncells: usize, nports: u8, edge_list: Vec<(usize,usize)>) -> Result<Datacenter<'b>,DatacenterError> {
		if ncells < 2  {
			println!("ncells {}", ncells);
			return Err(DatacenterError::Size(SizeError::new(ncells)));
		}
		if edge_list.len() < ncells {
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
			let split;
			if edge.0 == 0 { 
				split = cells.split_at_mut(edge.1)
			} else  {
				split = cells.split_at_mut(edge.0)
			};
			let mut cell = match split.0.last_mut() {
				Some(c) => c,
				None => return Err(DatacenterError::Wire(WireError::new(edge)))

			};
			let mut p1 = try!(cell.get_free_port_mut());
			let mut cell = match split.1.first_mut() {
				Some(c) => c,
				None => return Err(DatacenterError::Wire(WireError::new(edge)))
			};
			let mut p2 = try!(cell.get_free_port_mut());
			links.push(try!(Link::new(p1,p2)));
		} 
		Ok(Datacenter { cells: cells, links: links, noc: None })
	}
	pub fn add_noc(&mut self, control: &'b NalCell, backup: &'b NalCell) {
		self.noc = Some(NOC::new(control, backup));
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("Cells");
		for i in 0..self.cells.len() {			
			if i < 3 { s = s + &format!("\n {}", self.cells[i]); }
		}
		s = s + "\nLinks";
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		s
	}
}
impl<'b> fmt::Display for Datacenter<'b> { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
// Errors
use std::error::Error;
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
			DatacenterError::Name(_) => write!(f, "Link Name Error caused by"),
			DatacenterError::Link(_) => write!(f, "Link Error caused by"),
			DatacenterError::Cell(_) => write!(f, "Cell Error caused by"),
			DatacenterError::Wire(_) => write!(f, "Wire Error caused by"),
			DatacenterError::Size(_) => write!(f, "Size Error caused by"),
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
