use std::fmt;
use nalcell::{NalCell,NalCellError};
use link::{Link,LinkError};
use noc::NOC;

#[derive(Debug,Clone)]
pub struct Datacenter<'a> {
	cells: Vec<NalCell>,
	links: Vec<Link<'a>>,
	noc: Option<NOC>
}
impl<'a> Datacenter<'a> {
	pub fn new() -> Datacenter<'a> {
		Datacenter { cells: Vec::new(), links: Vec::new(), noc: None, }
	}
	pub fn build(&'a mut self, ncells: usize, nports: u8, edge_list: Vec<(usize,usize)>) -> Result<(),DatacenterError> {
		try!(self.build_cells(ncells, nports));
		try!(self.connect_edges(edge_list));
		Ok(()) 
	}
	fn build_cells(&mut self, ncells: usize, nports: u8) -> Result<&Vec<NalCell>,DatacenterError>{
		for i in 0..ncells {
			let mut is_border = false;
			if i % 3 == 1 { is_border = true; }
			let id = format!("C:{}",i);
			let cell = try!(NalCell::new(&id, i, nports, is_border));
			self.cells.push(cell);
		}	
		Ok((&self.cells))	
	}
	fn connect_edges(&'a mut self, edge_list: Vec<(usize,usize)>) -> Result<&Vec<Link>,DatacenterError> {
		for edge in edge_list {
			if edge.0 == edge.1 { return Err(DatacenterError::Wire(WireError::new(edge))); }
			let p1 = match self.cells[edge.0].get_free_port() {
				Ok(p) => p,
				Err(err) => return Err(DatacenterError::Cell(err))
			};
			let p2 = try!(self.cells[edge.1].get_free_port());
			let id = try!(p1.get_id().add_component(&p2.get_id().get_name().to_string()));
			let link = try!(Link::new(&id.get_name().to_string(),&p1,&p2));
			self.links.push(link);
		} 
		Ok(&self.links)		
	}
	pub fn add_noc(&mut self, control: NalCell, backup: NalCell) {
		self.noc = Some(NOC::new(control, backup));
	}
	pub fn to_string(&self) -> String {
		let mut s = format!("Cells");
		for i in 0..self.cells.len() {
			if i < 3 { s = s + &format!("\n {}",self.cells[i]); }
		}
		s = s + "\nLinks";
		for l in &self.links {
			s = s + &format!("{}",l);
		}
		s
	}
}
impl<'a> fmt::Display for Datacenter<'a> { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum DatacenterError {
	Name(NameError),
	Link(LinkError),
	Cell(NalCellError),
	Wire(WireError)
}
impl Error for DatacenterError {
	fn description(&self) -> &str {
		match *self {
			DatacenterError::Name(ref err) => err.description(),
			DatacenterError::Link(ref err) => err.description(),
			DatacenterError::Cell(ref err) => err.description(),
			DatacenterError::Wire(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			DatacenterError::Name(ref err) => Some(err),
			DatacenterError::Link(ref err) => Some(err),
			DatacenterError::Cell(ref err) => Some(err),
			DatacenterError::Wire(ref err) => Some(err),
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
		}
	}
}
#[derive(Debug)]
pub struct WireError { msg: String }
impl WireError { 
	pub fn new(wire: (usize,usize)) -> WireError {
		WireError { msg: format!("Wire {:?} connects a cell to itself", wire) }
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
impl From<NameError> for DatacenterError {
	fn from(err: NameError) -> DatacenterError { DatacenterError::Name(err) }
}
impl From<LinkError> for DatacenterError {
	fn from(err: LinkError) -> DatacenterError { DatacenterError::Link(err) }
}
impl From<NalCellError> for DatacenterError {
	fn from(err: NalCellError) -> DatacenterError { DatacenterError::Cell(err) }
}
impl From<WireError> for DatacenterError {
	fn from(err: WireError) -> DatacenterError { DatacenterError::Wire(err) }
}
