// Want names to be stack allocated, but String goes on the heap

use std::fmt;
use std::hash::{Hash,Hasher};
use config::MAX_NAME_SIZE;
use utility::chars_to_string;

const SEPARATOR: char = '+';
type NAME = [char; MAX_NAME_SIZE];
// Names may not contain blanks
#[derive(Copy)]
pub struct Name { name: NAME }
impl Name {
	pub fn new(n: &str) -> Result<Name,NameError> {
		let mut name = [' '; MAX_NAME_SIZE];
		match n.find(' ') {
			Some(_) => { Err(NameError::Format(FormatError::new(n)) ) },
			None => {
				if n.len() <= MAX_NAME_SIZE { 
					for c in n.char_indices() { name[c.0] = c.1; } 
					Ok(Name { name: name, })
				} else { 
					Err( NameError::Size(SizeError::new(n.len())) )
				}
			}
		}		
	}
	fn get_name(&self) -> NAME { self.name }
	fn len(&self) -> usize {
		let mut l = 0;
		for c in self.name.iter() {
			if *c == ' ' { break; }
			l = l + 1;
		}
		l
	}
	fn add_component(&self, s: &str) -> Result<Self,NameError> {	
		let mut n = self.name;
		let mut s_plus = s.to_string();
		s_plus.insert(0,SEPARATOR);
		match s.find(' ') {
			Some(_) => Err(NameError::Format( FormatError::new(s) )),
			None => {
				if self.len() + s_plus.len() <= MAX_NAME_SIZE { 
					let mut c_iter = s_plus.chars();
					for i in 0..self.name.len() {
						if n[i] != ' ' { continue; }
						else { n[i] = match c_iter.next() {
								Some(c) => c,
								None    => break
							}	
						}
					}
					Ok(Name { name: n, })
				} else {
					Err(NameError::Size( SizeError::new(n.len()) ))
				}
			}
		}
	}
	pub fn to_string(&self) -> String { chars_to_string(&self.name) }
}
impl PartialEq for Name {
	fn eq(&self, other: &Name) -> bool { 
		let mut retval = true;
		if self.name.len() != other.name.len() { retval = false; }
		else {
			for i in 0..self.name.len() {
				if self.name[i] == other.name[i] { continue }
				else {
					retval = false;
					break;
				}
			}
		}
		retval
	}
}
impl Eq for Name {}
impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
impl Clone for Name { fn clone(&self) -> Name { *self } }
impl fmt::Debug for Name {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
impl fmt::Display for Name {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CellID { name: Name, }
impl<'a> CellID {
	pub fn new(n: &str) -> Result<CellID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(CellID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &str) -> Result<CellID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(CellID { name: n}),
			Err(e) => Err(e)
		}
	}
	pub fn get_name(&self) -> Name { self.name }
	pub fn to_string(&self) -> String { format!("CellID: {}", self.name.to_string()) }
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TreeID { name: Name, }
impl<'a> TreeID {
	pub fn new(n: &str) -> Result<TreeID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(TreeID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &str) -> Result<TreeID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(TreeID { name: n}),
			Err(e) => Err(e)
		}
	}
	pub fn get_name(&self) -> Name { self.name }
	pub fn to_string(&self) -> String { format!("TreeID: {}", self.name.to_string()) }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: Name, }
impl TenantID {
	pub fn new(n: &str) -> Result<TenantID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(TenantID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &str) -> Result<TenantID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(TenantID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> Name { self.name }
	pub fn to_string(&self) -> String { format!("TenantID: {}", self.name.to_string()) }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PortID { name: Name, }
impl PortID {
	pub fn new(n: &str) -> Result<PortID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(PortID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &str) -> Result<PortID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(PortID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> Name { self.name }
	pub fn to_string(&self) -> String { format!("PortID: {}", self.name.to_string()) }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct LinkID { name: Name, }
impl LinkID {
	pub fn new(n: &str) -> Result<LinkID,NameError> { 
		match Name::new(n) {
			Ok(name) => Ok(LinkID { name: name}),
			Err(e) => Err(e)
		}
	}
	pub fn add_component(&self, s: &str) -> Result<LinkID,NameError> { 
		match self.name.add_component(s) {
			Ok(n) => Ok(LinkID { name: n}),
			Err(e) => Err(e)
		}		
	}
	pub fn get_name(&self) -> Name { self.name }
	pub fn to_string(&self) -> String { format!("LinkID: {}", self.name.to_string()) }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }

// Errors
use std::error::Error;
#[derive(Debug)]
pub enum NameError {
	Format(FormatError),
	Size(SizeError)
}
impl Error for NameError {
	fn description(&self) -> &str { 
		match *self {
			NameError::Format(ref err) => err.description(),
			NameError::Size(ref err) => err.description()
		}	 
	}
	fn cause(&self) -> Option<&Error> {  
		match *self {
			NameError::Format(_) => None,
			NameError::Size(_) => None
		}
	}
}
impl fmt::Display for NameError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			NameError::Format(ref err) => write!(f, "Name Format Error: {}", err),
			NameError::Size(ref err) => write!(f, "Name Size Error: {}", err)
		}
	}
}
impl From<SizeError> for NameError {
	fn from(err: SizeError) -> NameError { NameError::Size(err) }
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
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError {
	pub fn new(n: usize) -> SizeError {
		SizeError { msg: format!("'{}' more than {} characters", n, MAX_NAME_SIZE) }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, "'{}", self.msg) 
	}
}
