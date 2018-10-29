use std::fmt;
use std::collections::HashMap;
use std::error::Error;
use name::{TenantID};
use errors::*;

#[derive(Clone, PartialEq)]
pub struct Tenant { 
	name: TenantID, 
	ncells: usize, 
	children: HashMap<TenantID,Box<Tenant>>,
}

impl Tenant {
	pub fn new(id: &str, n: usize, parent_id: Option<TenantID>) -> Result<Tenant,TenantError> {
		let name = match parent_id {
			Some(p) => p.add_component(id),
			None => TenantID::new(id)
		}; 
		match name {
			Ok(name) => Ok(Tenant { name: name, ncells: n, children: HashMap::new(), }),
			Err(err) => Err(TenantError::from(err))
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
	pub fn create_subtenant(&mut self, id: &str, n:usize) -> Result<Tenant,TenantError> {
		if self.ncells < n {
			Err(TenantError::Quota(QuotaError::new(n, self.ncells) ))
		} else {
			let tenant = Tenant::new(id, n, Some(self.get_name()));
			match tenant {	
				Ok(t) => {
					if !self.children.contains_key(&t.get_name()) {
						self.children.insert(t.get_name(),Box::new(t.clone()));
						Ok(t)	
					} else { 
						Err(TenantError::DuplicateName(DuplicateNameError::new(id) )) 
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
