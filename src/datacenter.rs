use std::fmt;
use nalcell::{NalCell,NalCellError};
use link::{Link,LinkError};
use noc::NOC;

#[derive(Debug,Clone)]
pub struct Datacenter<'b> {
	cells: Vec<NalCell>,
	links: Vec<Link<'b>>,
	noc: Option<NOC>
}
impl<'b> Datacenter<'b> {
	pub fn new(cells: &'b mut Vec<NalCell>, ncells: usize, nports: u8, edge_list: Vec<(usize,usize)>) -> Result<Datacenter<'b>,DatacenterError> {
		//let mut cells = Vec::new();
		for i in 0..ncells {
			let mut is_border = false;
			if i % 3 == 1 { is_border = true; }
			cells.push(try!(NalCell::new(i, nports, is_border))); 
		}	
		let mut links: Vec<Link> = Vec::new();
		for edge in edge_list {
			if edge.0 == edge.1 { return Err(DatacenterError::Wire(WireError::new(edge))); }
			let p1 = try!(cells[edge.0].get_free_port()); 
			let p2 = try!(cells[edge.1].get_free_port());
			links.push(try!(Link::new(p1,p2)));
		} 
		Ok(Datacenter { cells: cells.clone(), links: links, noc: None })
	}
	//pub fn add_noc(&mut self, control: &'b NalCell, backup: &'b NalCell) {
	//	self.noc = Some(NOC::new(control, backup));
	//}
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
impl<'b> fmt::Display for Datacenter<'b> { 
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
