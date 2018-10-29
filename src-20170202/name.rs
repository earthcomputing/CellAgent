// Want names to be stack allocated, but String goes on the heap

use std::fmt;
use std::error::Error;
use std::hash::{Hash,Hasher};
use config::MAX_NAME_SIZE;
use errors::*;

const SEPARATOR: char = '+';
// Names may not contain blanks
struct Name { name: [char; MAX_NAME_SIZE], }
impl Name {
	fn new(n: &str) -> Result<Name,NameError> {
		let mut name = [' '; MAX_NAME_SIZE];
		match n.find(' ') {
			Some(_) => { Err(NameError::Format(FormatError::new(n) )) },
			None => {
				if n.len() <= MAX_NAME_SIZE { 
					for c in n.char_indices() { name[c.0] = c.1; } 
					Ok(Name { name: name, })
				} else { 
					Err(NameError::Size(SizeError::new(n.len()) ))
				}
			}
		}		
	}
	fn get_name(&self) -> Self { self.clone() }
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
	fn to_string(&self) -> String {
		let mut s = String::new();
		for c in self.name.iter() {
			if *c == ' ' { break; }
			s.push(*c);
		}
		s
	}
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
impl Copy for Name {}
impl Clone for Name { fn clone(&self) -> Name { *self } }
impl fmt::Debug for Name {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CellID { name: Name, }
impl CellID {
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
	pub fn get_name(&self) -> CellID { CellID { name: self.name.get_name() } }
	pub fn to_string(&self) -> String { format!("CellID: {}", self.name.to_string()) }
}
impl fmt::Debug for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
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
	pub fn get_name(&self) -> TenantID { TenantID { name: self.name.get_name() } }
	pub fn to_string(&self) -> String { format!("TenantID: {}", self.name.to_string()) }
}
impl fmt::Debug for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
