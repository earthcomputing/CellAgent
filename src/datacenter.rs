use std::fmt;
use std::cell::{RefCell, BorrowError, BorrowMutError};
use nalcell::{NalCell,NalCellError};
use link::{Link,LinkError};
use noc::NOC;

#[derive(Debug)]
pub struct Datacenter<'b> {
	cells: Vec<RefCell<NalCell>>,
	links: Vec<Link>,
	noc: Option<NOC<'b>>
}
impl<'b> Datacenter<'b> {
	pub fn new(cells: &'b mut Vec<NalCell>, ncells: usize, nports: u8, edge_list: Vec<(usize,usize)>) -> Result<Datacenter<'b>,DatacenterError> {
		let mut cells = Vec::new();
		for i in 0..ncells {
			let mut is_border = false;
			if i % 3 == 1 { is_border = true; }
			cells.push(RefCell::new(try!(NalCell::new(i, nports, is_border)))); 
		}	
		let mut links: Vec<Link> = Vec::new();
		for edge in edge_list {
			if edge.0 == edge.1 { return Err(DatacenterError::Wire(WireError::new(edge))); }
			let mut cell = try!(cells[edge.0].try_borrow_mut());
			let p1 = try!(cell.get_free_port_mut());
			let mut cell = try!(cells[edge.1].try_borrow_mut());
			let p2 = try!(cell.get_free_port_mut());
			links.push(try!(Link::new(p1,p2)));
		} 
		Ok(Datacenter { cells: cells, links: links, noc: None })
	}
	pub fn add_noc(&mut self, control: &'b NalCell, backup: &'b NalCell) {
		self.noc = Some(NOC::new(control, backup));
	}
	pub fn to_string(&self) -> String {
		let mut s = format!("Cells");
		for i in 0..self.cells.len() {
			// let cell = try!(self.cells[i].try_borrow()); Doesn't compile
			let cell = match self.cells[i].try_borrow() {
				Ok(cell) => cell,
				Err(err) => panic!("error")
			};
			
			if i < 3 { s = s + &format!("\n {}", *cell); }
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
	Wire(WireError),
	Borrow(BorrowError),
	BorrowMut(BorrowMutError)
}
impl Error for DatacenterError {
	fn description(&self) -> &str {
		match *self {
			DatacenterError::Name(ref err) => err.description(),
			DatacenterError::Link(ref err) => err.description(),
			DatacenterError::Cell(ref err) => err.description(),
			DatacenterError::Wire(ref err) => err.description(),
			DatacenterError::Borrow(ref err) => err.description(),
			DatacenterError::BorrowMut(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			DatacenterError::Name(ref err) => Some(err),
			DatacenterError::Link(ref err) => Some(err),
			DatacenterError::Cell(ref err) => Some(err),
			DatacenterError::Wire(ref err) => Some(err),
			DatacenterError::Borrow(ref err) => Some(err),
			DatacenterError::BorrowMut(ref err) => Some(err),
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
			DatacenterError::Borrow(_) => write!(f, "RefCell Error caused by"),
			DatacenterError::BorrowMut(_) => write!(f, "RefCell Error caused by"),
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
impl From<BorrowError> for DatacenterError {
	fn from(err: BorrowError) -> DatacenterError { DatacenterError::Borrow(err) }
}
impl From<BorrowMutError> for DatacenterError {
	fn from(err: BorrowMutError) -> DatacenterError { DatacenterError::BorrowMut(err) }
}
