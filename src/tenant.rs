use std::fmt;
use std::hash::{Hash,Hasher};
use std::collections::HashMap;
use std::error::Error;
use name::{Name,TenantID};

#[derive(Clone)]
pub struct Tenant { 
	id: TenantID, 
	ncells: usize, 
	children: HashMap<TenantID,Box<Tenant>>,
}

impl Tenant {
	pub fn new(id: &'static str, n: usize, parent_id: Option<TenantID>) -> Result<Tenant,TenantError> {
		let name = match parent_id {
			Some(p) => Ok(try!(p.add_component(id))),
			None => TenantID::new(id)
		}; 
		match name {
			Ok(name) => Ok(Tenant { id: name, ncells: n, children: HashMap::new(), }),
			Err(err) => Err(TenantError::from(err))
		}
	}
	pub fn get_id(&self) -> TenantID { self.id.clone() }
	pub fn get_ncells(&self) -> usize { self.ncells }
	//pub fn get_size(&self) -> usize { self.ncells }
	pub fn get_children(&self) -> &HashMap<TenantID,Box<Tenant>> { &self.children }
	pub fn get_subtenant(&self, id: TenantID) -> Option<&Box<Tenant>> {
		self.children.get(&id)
	}
	pub fn get_mut_subtenant(&mut self, id: TenantID) -> Option<&mut Box<Tenant>> {
		self.children.get_mut(&id)
	}
	pub fn create_subtenant(&mut self, id: &'static str, n:usize) -> Result<Tenant,TenantError> {
		if self.ncells < n {
			Err(TenantError::Quota(QuotaError::new(n, self.ncells) ))
		} else {
			let tenant = Tenant::new(id, n, Some(self.get_id()));
			match tenant {	
				Ok(t) => {
					if !self.children.contains_key(&t.get_id()) {
						self.children.insert(t.get_id(),Box::new(t.clone()));
						Ok(t)	
					} else { 
						Err(TenantError::DuplicateName(DuplicateNameError::new(id) )) 
					}
				},
				Err(_) => tenant
			}
		}
	}
}

impl fmt::Debug for Tenant { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = "Tenant: ".to_string();
		s = s + &format!("{:?} {} cells", self.id, self.ncells);
		write!(f, "{}", s) 
	} 
}
// Errors
use name::{NameError};
#[derive(Debug)]
pub enum TenantError {
	Name(NameError),
	DuplicateName(DuplicateNameError),
	Quota(QuotaError)
}
impl Error for TenantError {
	fn description(&self) -> &str {
		match *self {
			TenantError::DuplicateName(ref err) => err.description(),
			TenantError::Quota(ref err) => err.description(),
			TenantError::Name(ref err) => err.description()
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TenantError::DuplicateName(_) => None,
			TenantError::Quota(_) => None,
			TenantError::Name(ref err) => Some(err)
		}
	}
}
impl fmt::Display for TenantError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TenantError::DuplicateName(ref err) => write!(f, "Tenant Name Error: {}", err),
			TenantError::Quota(ref err) => write!(f, "Tenant Quota Error: {}", err),
			TenantError::Name(_) => write!(f, "Tenant Name Error caused by")
		}
	}
}
#[derive(Debug)]
pub struct QuotaError { msg: String }
impl QuotaError { 
	pub fn new(n: usize, available: usize) -> QuotaError {
		QuotaError { msg: format!("You asked for {} cells, but only {} are available", n, available) }
	}
}
impl Error for QuotaError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for QuotaError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<QuotaError> for TenantError {
	fn from(err: QuotaError) -> TenantError { TenantError::Quota(err) }
}
#[derive(Debug)]
pub struct DuplicateNameError { msg: String }
impl DuplicateNameError {
	pub fn new(id: &str) -> DuplicateNameError {
		DuplicateNameError { msg: format!("A tenant named '{}' already exists.", id) }
	}
}
impl Error for DuplicateNameError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for DuplicateNameError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<DuplicateNameError> for TenantError {
	fn from(err: DuplicateNameError) -> TenantError { TenantError::DuplicateName(err) }
}
impl From<NameError> for TenantError {
	fn from(err: NameError) -> TenantError { TenantError::Name(err) }
}
