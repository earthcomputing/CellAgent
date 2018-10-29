// Want names to be stack allocated, but String goes on the heap

use std::fmt;
use std::hash::{Hash,Hasher};
use config::MAX_NAME_SIZE;

const SEPARATOR: char = '+';
// Names may not contain blanks
struct Name { name: [char; MAX_NAME_SIZE], }
impl Name {
	fn new(n: &str) -> Option<Self> {
		let mut name = [' '; MAX_NAME_SIZE];
		match n.find(' ') {
			Some(_) => {
				let msg = format!("Name: '{}' contains blanks.", n);
				None
			},
			None => {
				if n.len() <= MAX_NAME_SIZE { 
					for c in n.char_indices() { name[c.0] = c.1; } 
					Some(Name { name: name, })
				} else { 
					let msg = format!("Name: '{}' more than {} characters", n, MAX_NAME_SIZE);
					None 
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
	fn add_component(&self, s: &str) -> Option<Self> {
		let mut retval = None;
		let mut n = self.name;
		let mut s_plus = s.to_string();
		s_plus.insert(0,SEPARATOR);
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
			retval = Some(Name { name: n, });
		}
		retval
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
	pub fn new(n: &str) -> Option<CellID> { 
		match Name::new(n) {
			Some(name) => Some(CellID { name: name}),
			None => None
		}
	}
	pub fn add_component(&self, s: &str) -> Option<CellID> { 
		match self.name.add_component(s) {
			Some(n) => Some(CellID { name: n}),
			None => None
		}
	}
	pub fn get_name(&self) -> CellID { CellID { name: self.name.get_name() } }
	pub fn to_string(&self) -> String { format!("CellID: {}", self.name.to_string()) }
}
impl fmt::Debug for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: Name, }
impl TenantID {
	pub fn new(n: &str) -> Option<TenantID> { 
		match Name::new(n) {
			Some(name) => Some(TenantID { name: name}),
			None => None
		}
	}
	pub fn add_component(&self, s: &str) -> Option<TenantID> { 
		match self.name.add_component(s) {
			Some(n) => Some(TenantID { name: n}),
			None => None
		}		
	}
	pub fn get_name(&self) -> TenantID { TenantID { name: self.name.get_name() } }
	pub fn to_string(&self) -> String { format!("TenantID: {}", self.name.to_string()) }
}
impl fmt::Debug for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
