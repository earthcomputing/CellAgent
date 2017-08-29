use std::fmt;
use std::collections::HashMap;

use name::{Name,TenantID};

#[derive(Clone)]
pub struct Tenant { 
	id: TenantID, 
	ncells: CellNo, 
	children: HashMap<TenantID,Box<Tenant>>,
}
#[deny(unused_must_use)]
impl Tenant {
	pub fn new(id: &'static str, n: CellNo, parent_id: Option<TenantID>) -> Result<Tenant> {
		let name = match parent_id {
			Some(p) => Ok(p.add_component(id).chain_err(|| ErrorKind::TenantError)?),
			None => TenantID::new(id)
		}; 
		match name {
			Ok(name) => Ok(Tenant { id: name, ncells: n, children: HashMap::new(), }),
			Err(err) => Err(err.into())
		}
	}
	pub fn get_id(&self) -> TenantID { self.id.clone() }
//	pub fn get_ncells(&self) -> usize { self.ncells }
	//pub fn get_size(&self) -> usize { self.ncells }
	pub fn get_children(&self) -> &HashMap<TenantID,Box<Tenant>> { &self.children }
//	pub fn get_subtenant(&self, id: TenantID) -> Option<&Box<Tenant>> {
//		self.children.get(&id)
//	}
//	pub fn get_mut_subtenant(&mut self, id: TenantID) -> Option<&mut Box<Tenant>> {
//		self.children.get_mut(&id)
//	}
	pub fn create_subtenant(&mut self, id: &'static str, n: CellNo) -> Result<Tenant> {
		if *self.ncells < *n {
			Err(ErrorKind::Quota(n, "create_subtenant".to_string(), self.ncells).into())
		} else {
			let tenant = Tenant::new(id, n, Some(self.get_id()));
			match tenant {	
				Ok(t) => {
					if !self.children.contains_key(&t.get_id()) {
						self.children.insert(t.get_id(),Box::new(t.clone()));
						Ok(t)	
					} else { 
						Err(ErrorKind::DuplicateName(id.to_string(), "create_subtenant".to_string()).into()) 
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
		s = s + &format!("{:?} {} cells", self.id, *self.ncells);
		write!(f, "{}", s) 
	} 
}
// Errors
use config::CellNo;
error_chain! {
	links {
		Name(::name::Error, ::name::ErrorKind);
	}
	errors { TenantError
		DuplicateName(tenant_id: String, func_name: String) {
			display("{}: Tenant: A tenant named '{}' already exists.", func_name, tenant_id)
		}
		Quota(request: CellNo, func_name: String, available: CellNo) {
			display("{}: Tenant: You asked for {} cells, but only {} are available", func_name, request.0, available.0)
		}
	}
}
