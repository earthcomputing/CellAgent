use std::fmt;
use std::marker::Sized;
use config::SEPARATOR;
use errors::*;
// Using String means names are not Copy
type NAME = String;
pub trait Name: Sized {
	fn get_name(&self) -> &str;
	fn create_from_string(&self, n: String) -> Self;
	// Default implementations
	fn stringify(&self) -> String { self.get_name().to_string() }
	fn from_str(&self, s: &str) -> Result<Self> {
		// Names may not contain blanks
		match s.find(' ') {
			Some(_) => Err(ErrorKind::FormatError(s.to_string()).into()),
			None => Ok(self.create_from_string(s.to_string()))
		}
	}
	fn add_component(&self, s: &str) -> Result<Self> {	
		match s.find(' ') {
			Some(_) => Err(ErrorKind::FormatError(s.to_string()).into()),
			None => self.from_str(&([self.get_name(),s].join(SEPARATOR)))
		}
	}		
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NAME, }
impl<'a> CellID {
	pub fn new(n: usize) -> Result<CellID> { 
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
	pub fn new(n: u8) -> Result<PortID> { 
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
	pub fn new(n: &str) -> Result<TreeID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(TreeID { name: str}),
			Some(_) => Err(ErrorKind::FormatError(str).into())
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
	pub fn new(n: &str) -> Result<TenantID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(TenantID { name: str}),
			Some(_) => Err(ErrorKind::FormatError(str).into())
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
	pub fn new(n: &str) -> Result<LinkID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(LinkID { name: str, }),
			Some(_) => Err(ErrorKind::FormatError(str).into())
		}
	}
}
impl Name for LinkID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> LinkID { LinkID { name: n } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
// Errors
error_chain! {
	errors {
		FormatError(name: String) {
			description("Name cannot contain blanks")
			display("'{}' contains blanks.", name)
		}
		SizeError(name: String) {
			description("Name is too long")
			display("'{}' is longer than {} characters", name, ::config::MAX_CHARS)
		}
	}
}
