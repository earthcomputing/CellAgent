use std::fmt;
use std::collections::HashMap;
use std::error::Error;
use name::{NameError,TenantID};

#[derive(Clone, PartialEq)]
pub struct Tenant { 
	name: TenantID, 
	ncells: usize, 
	children: HashMap<TenantID,Box<Tenant>>,
}

impl Tenant {
	pub fn new(id: &str, n: usize, parent_id: Option<TenantID>) -> Result<Tenant,TenantErrors> {
		let name = match parent_id {
			Some(p) => p.add_component(id),
			None => TenantID::new(id)
		}; 
		match name {
			Ok(name) => Ok(Tenant { name: name, ncells: n, children: HashMap::new(), }),
			Err(e) => Err(TenantErrors::TenantNameError(e))
		}
	}
	pub fn get_name(&self) -> TenantID { self.name }
	pub fn get_ncells(&self) -> usize { self.ncells }
	//pub fn get_size(&self) -> usize { self.ncells }
	pub fn get_children(&self) -> &HashMap<TenantID,Box<Tenant>> { &self.children }
	pub fn get_subtenant(&self, id: TenantID) -> Option<&Box<Tenant>> {
		self.children.get(&id)
	}
	pub fn get_mut_subtenant(&mut self, id: TenantID) -> Option<&mut Box<Tenant>> {
		self.children.get_mut(&id)
	}
	pub fn create_subtenant(&mut self, id: &str, n:usize) -> Result<Tenant,TenantErrors> {
		if self.ncells < n {
			Err(TenantErrors::TenantBuildError(TenantError { msg: TenantError::not_enough_cells(n, self.ncells) }))
		} else {
			let tenant = Tenant::new(id, n, Some(self.get_name()));
			match tenant {	
				Ok(t) => {
					if !self.children.contains_key(&t.get_name()) {
						self.children.insert(t.get_name(),Box::new(t.clone()));
						Ok(t)	
					} else { 
						Err(TenantErrors::TenantBuildError(TenantError { msg: TenantError::tenant_already_exists(id) })) 
					}
				},
				Err(_) => tenant
			}
		}
	}
	pub fn to_string(&self) -> String {
		let mut s = "Tenant: ".to_string();
		s = s + &format!("{:?} {} cells", self.name, self.ncells);
		s
	}
}

impl fmt::Debug for Tenant { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) } 
}
#[derive(Debug)]
pub enum TenantErrors {
	TenantNameError(NameError),
	TenantBuildError(TenantError)
}
impl fmt::Display for TenantErrors {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			TenantErrors::TenantNameError(ref err) => write!(f, "Tenant Name Error: {}", err),
			TenantErrors::TenantBuildError(ref err) => write!(f, "Tenant Build Error: {}", err),
		}
	}
}
impl Error for TenantErrors {
	fn description(&self) -> &str {
		match *self {
			TenantErrors::TenantNameError(ref err) => err.description(),
			TenantErrors::TenantBuildError(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			TenantErrors::TenantNameError(ref err) => Some(err),
			TenantErrors::TenantBuildError(ref err) => Some(err),
		}
	}
}
pub struct TenantError { msg: String }
impl TenantError { 
	fn set_msg(msg: String) -> TenantError { TenantError { msg: msg } } 
	fn tenant_already_exists(id: &str) -> String { 
		format!("Tenant: Tenant {} already exists.",id) 
	}
	fn not_enough_cells(n: usize, available: usize) -> String {
		format!("Tenant: You asked for {} but only {} available", n, available)
	}
}
impl Error for TenantError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Debug for TenantError {
	fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
		write!(f, "TenantError: {}", self.msg)
	}
}
impl fmt::Display for TenantError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "TenantError: {}", self.msg)
	}
}
