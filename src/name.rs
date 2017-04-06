use std::fmt;
use std::marker::Sized;
use config::SEPARATOR;
// Using String means names are not Copy
type NAME = String;
pub trait Name: Sized {
	fn get_name(&self) -> &str;
	fn create_from_string(&self, n: String) -> Self;
	// Default implementations
	fn stringify(&self) -> String { self.get_name().to_string() }
	fn from_str(&self, s: &str) -> Result<Self,NameError> {
		// Names may not contain blanks
		match s.find(' ') {
			Some(_) => Err(NameError::Format(FormatError::new(s))),
			None => Ok(self.create_from_string(s.to_string()) )
		}
	}
	fn add_component(&self, s: &str) -> Result<Self,NameError> {	
		match s.find(' ') {
			Some(_) => Err(NameError::Format( FormatError::new(s) )),
			None => self.from_str(&([self.get_name(),s].join(SEPARATOR)))
		}
	}		
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NAME, }
impl<'a> CellID {
	pub fn new(n: usize) -> Result<CellID,NameError> { 
		let n = format!("C:{}",n);
		Ok(CellID { name: n})
	}
}
impl Name for CellID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> CellID { CellID { name: n } }	
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortID { name: NAME, }
impl PortID {
	pub fn new(n: u8) -> Result<PortID,NameError> { 
		let n = format!("P:{}",n);
		Ok(PortID { name: n })
	}
}
impl Name for PortID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> PortID { PortID { name: n } }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TreeID { name: NAME, }
impl<'a> TreeID {
	pub fn new(n: &str) -> Result<TreeID,NameError> { 
		match n.find(' ') {
			None => Ok(TreeID { name: n.to_string()}),
			Some(_) => Err(NameError::Format(FormatError::new(n)))
		}
	}
}
impl Name for TreeID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> TreeID { TreeID { name: n } }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: NAME, }
impl TenantID {
	pub fn new(n: &str) -> Result<TenantID,NameError> { 
		match n.find(' ') {
			None => Ok(TenantID { name: n.to_string()}),
			Some(i) => Err(NameError::Format(FormatError::new(n)) )
		}
	}
}
impl Name for TenantID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> TenantID { TenantID { name: n } }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LinkID { name: NAME, }
impl LinkID {
	pub fn new(n: &str) -> Result<LinkID,NameError> { 
		match n.find(' ') {
			None => Ok(LinkID { name: n.to_string(), }),
			Some(_) => Err(NameError::Format(FormatError::new(n)) )
		}
	}
}
impl Name for LinkID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> LinkID { LinkID { name: n } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }

// Errors
use std::error::Error;
#[derive(Debug)]
pub enum NameError {
	Format(FormatError),
}
impl Error for NameError {
	fn description(&self) -> &str { 
		match *self {
			NameError::Format(ref err) => err.description(),
		}	 
	}
	fn cause(&self) -> Option<&Error> {  
		match *self {
			NameError::Format(_) => None,
		}
	}
}
impl fmt::Display for NameError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NameError::Format(ref err) => write!(f, "Name Format Error: {}", err),
		}
	}
}
#[derive(Debug)]
pub struct FormatError { msg: String }
impl FormatError {
	pub fn new(s: &str) -> FormatError { 
		FormatError { msg: format!("'{}' contains blanks.", s) }
	}
}
impl Error for FormatError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for FormatError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "{}", self.msg) 
	}
}
impl From<FormatError> for NameError {
	fn from(err: FormatError) -> NameError { NameError::Format(err) }
}