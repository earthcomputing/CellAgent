use std::fmt;
use std::collections::HashMap;

use failure::{Error, Fail, ResultExt};

use name::{Name, TenantID};
use utility::S;

#[derive(Clone)]
pub struct Tenant { 
	id: TenantID, 
	ncells: CellNo, 
	children: HashMap<TenantID,Box<Tenant>>,
}
#[deny(unused_must_use)]
impl Tenant {
	pub fn new(id: &'static str, n: CellNo, parent_id: Option<TenantID>) -> Result<Tenant, Error> {
		let name = match parent_id {
			Some(p) => Ok(p.add_component(id).context(TenantError::Chain { func_name: "new", comment: S("")})?),
			None => TenantID::new(id)
		}?; 
		Ok(Tenant { id: name, ncells: n, children: HashMap::new() })
	}
	pub fn get_id(&self) -> TenantID { self.id.clone() }
//	pub fn get_ncells(&self) -> usize { self.ncells }
	//pub fn get_size(&self) -> usize { self.ncells }
//	pub fn get_children(&self) -> &HashMap<TenantID,Box<Tenant>> { &self.children }
//	pub fn get_subtenant(&self, id: TenantID) -> Option<&Box<Tenant>> {
//		self.children.get(&id)
//	}
//	pub fn get_mut_subtenant(&mut self, id: TenantID) -> Option<&mut Box<Tenant>> {
//		self.children.get_mut(&id)
//	}
	pub fn create_subtenant(&mut self, id: &'static str, n: CellNo) -> Result<Tenant, Error> {
		if *self.ncells < *n {
			Err(TenantError::Quota { request: n, func_name: "create_subtenant", available: self.ncells }.into())
		} else {
			let tenant = Tenant::new(id, n, Some(self.get_id()));
			match tenant {	
				Ok(t) => {
					if !self.children.contains_key(&t.get_id()) {
						self.children.insert(t.get_id(),Box::new(t.clone()));
						Ok(t)	
					} else { 
						Err(TenantError::DuplicateName { func_name: "create_subtenant", tenant_name: S(id) }.into())
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
#[derive(Debug, Fail)]
pub enum TenantError {
	#[fail(display = "TenantError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
	#[fail(display = "TenantError::DuplicateName {}: A tenant named '{}' already exists.", func_name, tenant_name)]
	DuplicateName { func_name: &'static str, tenant_name: String },
    #[fail(display = "TenantError::Quota {}: You asked for {:?} cells, but only {:?} are available", func_name, request, available)]
    Quota { func_name: &'static str, request: CellNo, available: CellNo }
}
