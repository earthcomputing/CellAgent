use std::fmt;
use port::Port;
use name::LinkID;

#[derive(Debug, Clone)]
pub struct Link<'a> {
	id: LinkID,
	is_connected: bool,
	ports: (&'a Port,&'a Port),
}
impl<'a> Link<'a> {
	pub fn new(id: &str, left: &'a Port, right: &'a Port) -> Result<Link<'a>,LinkError> {
		let id = try!(LinkID::new(id));
		left.set_connected();
		right.set_connected();
		Ok(Link { id: id, ports: (left,right), is_connected: true })
	}
	pub fn to_string(&self) -> String {
		let mut s = format!("\nLink {}", self.id.get_name().to_string());
		if self.is_connected { s = s + " is connected"; }
		else                 { s = s + " is not connected"; }
		s
	}
}
impl<'a> fmt::Display for Link<'a> { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum LinkError {
	Name(NameError),
}
impl Error for LinkError {
	fn description(&self) -> &str {
		match *self {
			LinkError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			LinkError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for LinkError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			LinkError::Name(_) => write!(f, "Link Name Error caused by")
		}
	}
}
impl From<NameError> for LinkError {
	fn from(err: NameError) -> LinkError { LinkError::Name(err) }
}
