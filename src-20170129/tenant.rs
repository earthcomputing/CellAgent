use std::fmt;
use std::collections::HashMap;
use name::TenantID;

#[derive(Clone, PartialEq)]
pub struct Tenant { 
	name: TenantID, 
	ncells: usize, 
	children: HashMap<TenantID,Box<Tenant>>,
}

impl Tenant {
	pub fn new(id: &str, n: usize, parent_id: Option<TenantID>) -> Option<Tenant> {
		let name = match parent_id {
			Some(p) => p.add_component(id),
			None => Some(TenantID::new(id).expect("Tenant: bad name"))
		}; 
		match name {
			Some(name) => Some(Tenant { name: name, ncells: n, children: HashMap::new(), }),
			None => None
		}
	}
	pub fn get_name(&self) -> TenantID { self.name }
	//pub fn get_size(&self) -> usize { self.ncells }
	pub fn get_children(&self) -> &HashMap<TenantID,Box<Tenant>> { &self.children }
	pub fn get_subtenant(&self, id: TenantID) -> Option<&Box<Tenant>> {
		self.children.get(&id)
	}
	pub fn get_mut_subtenant(&mut self, id: TenantID) -> Option<&mut Box<Tenant>> {
		self.children.get_mut(&id)
	}
	pub fn create_subtenant(&mut self, id: &str, n:usize) -> Option<Tenant> {
		let tenant = Tenant::new(id, n, Some(self.get_name()));
		match tenant.clone() {	
			Some(t) => {
				if !self.children.contains_key(&t.get_name()) {
					self.children.insert(t.get_name(),Box::new(t));
					tenant	
				} else { None }
			},
			None => tenant
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
