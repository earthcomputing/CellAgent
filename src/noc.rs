use std::fmt;
use nalcell::NalCell;

#[derive(Debug,Clone)]
pub struct NOC {
//	control: &'a NalCell<'a>,
//	backup: &'a NalCell<'a>
}
impl NOC {
//	pub fn new(control: &'a NalCell, backup: &'a NalCell) -> NOC<'a> { 
//		NOC { control: control, backup: backup }
//	}
	//pub fn to_string(&self) -> String {
	//	let mut s = format!("Control Cell = {}, Backup Cell = {}", 
	//		self.control.get_id(), self.backup.get_id());
	//	s
	//}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum NocError {
	Name(NameError),
}
impl Error for NocError {
	fn description(&self) -> &str {
		match *self {
			NocError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			NocError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for NocError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NocError::Name(_) => write!(f, "Link Name Error caused by")
		}
	}
}
impl From<NameError> for NocError {
	fn from(err: NameError) -> NocError { NocError::Name(err) }
}
