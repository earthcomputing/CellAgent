// Want names to be stack allocated, but String goes on the heap, so using &'static str

use std::fmt;
use config::SEPARATOR;

type NAME = &'static str;
// Names may not contain blanks
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Name { name: NAME }
impl Name {
	pub fn new(n: &'static str) -> Result<Name,NameError> {
		match n.find(' ') {
			Some(_) => { Err(NameError::Format(FormatError::new(n)) ) },
			None => Ok(Name { name: n, })
		}		
	}
	fn get_name(&self) -> &'static str { self.name }
	fn add_component(&self, s: &'static str) -> Result<Self,NameError> {	
		match s.find(' ') {
			Some(_) => Err(NameError::Format( FormatError::new(s) )),
			None => Ok(Name { name: Name::string_to_static_str([self.name,s].join(SEPARATOR)) })
		}
	}	
	pub fn to_string(&self) -> String { self.name.to_string() }
}
// From http://stackoverflow.com/questions/23975391/how-to-convert-string-into-static-str
use std::mem;
impl Name {
	fn string_to_static_str(s: String) -> &'static str {
	    unsafe {
	        let ret = mem::transmute(&s as &str);
	        mem::forget(s);
	        ret
	    }
	}
}
impl fmt::Display for Name {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CellID { name: Name, }
impl<'a> CellID {
	pub fn new(n: usize) -> Result<CellID,NameError> { 
		let n = Name::string_to_static_str(format!("C:{}",n));
		match Name::new(n) {
			Ok(name) => Ok(CellID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &'static str) -> Result<CellID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(CellID { name: n}),
			Err(e) => Err(e)
		}
	}
	pub fn get_name(&self) -> &'static str { self.name.name }
	pub fn to_string(&self) -> String { format!("CellID: {}", self.name.to_string()) }
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TreeID { name: Name, }
impl<'a> TreeID {
	pub fn new(n: &'static str) -> Result<TreeID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(TreeID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &'static str) -> Result<TreeID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(TreeID { name: n}),
			Err(e) => Err(e)
		}
	}
	pub fn get_name(&self) -> &'static str { self.name.name }
	pub fn to_string(&self) -> String { format!("TreeID: {}", self.name.to_string()) }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: Name, }
impl TenantID {
	pub fn new(n: &'static str) -> Result<TenantID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(TenantID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &'static str) -> Result<TenantID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(TenantID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> &'static str { self.name.name }
	pub fn to_string(&self) -> String { format!("TenantID: {}", self.name.to_string()) }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PortID { name: Name, }
impl PortID {
	pub fn new(n: u8) -> Result<PortID,NameError> { 
		let n = Name::string_to_static_str(format!("C:{}",n));
		match Name::new(n) {
			Ok(name) => Ok(PortID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &'static str) -> Result<PortID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(PortID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> &'static str { self.name.name }
	pub fn to_string(&self) -> String { format!("PortID: {}", self.name.to_string()) }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct LinkID { name: Name, }
impl LinkID {
	pub fn new(n: &'static str) -> Result<LinkID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(LinkID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &'static str) -> Result<LinkID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(LinkID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> &'static str { self.name.name }
	pub fn to_string(&self) -> String { format!("LinkID: {}", self.name.to_string()) }
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