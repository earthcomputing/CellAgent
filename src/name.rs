use std::fmt;
use std::marker::Sized;
use config::{SEPARATOR};
use utility::PortNumber;
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
			Some(_) => Err(ErrorKind::Format(s.to_string()).into()),
			None => Ok(self.create_from_string(s.to_string()))
		}
	}
	fn add_component(&self, s: &str) -> Result<Self> {	
		match s.find(' ') {
			Some(_) => Err(ErrorKind::Format(s.to_string()).into()),
			None => self.from_str(&([self.get_name(),s].join(SEPARATOR)))
		}
	}		
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NAME, }
impl CellID {
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
	pub fn new(cell_id: &CellID, port_number: PortNumber) -> Result<PortID> { 
		let name = [cell_id.get_name(), &format!("P:{}",port_number)].join(SEPARATOR); 
		Ok(PortID { name: name })
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
			Some(_) => Err(ErrorKind::Format(str).into())
		}
	}
}
impl Name for TreeID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> TreeID { TreeID { name: n } }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UpTreeID { name: NAME, }
impl<'a> UpTreeID {
	pub fn new(n: &str) -> Result<UpTreeID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(UpTreeID { name: str}),
			Some(_) => Err(ErrorKind::Format(str).into())
		}
	}
}
impl Name for UpTreeID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> UpTreeID { UpTreeID { name: n } }
}
impl fmt::Display for UpTreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: NAME, }
impl TenantID {
	pub fn new(n: &str) -> Result<TenantID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(TenantID { name: str}),
			Some(_) => Err(ErrorKind::Format(str).into())
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
	pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<LinkID> { 
		let name = [left_id.get_name(),rite_id.get_name()].join(SEPARATOR);
		Ok(LinkID { name: name })
	}
}
impl Name for LinkID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> LinkID { LinkID { name: n } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmID { name: NAME, }
impl VmID {
	pub fn new(cell_id: &CellID, id_no: usize) -> Result<VmID> { 
		let name = format!("VM:{}+{}", cell_id, id_no);
		Ok(VmID {name: name })
	}
}
impl Name for VmID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> VmID { VmID { name: n } }
}
impl fmt::Display for VmID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerID { name: NAME, }
impl ContainerID {
	pub fn new(n: &str) -> Result<ContainerID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(ContainerID { name: str}),
			Some(_) => Err(ErrorKind::Format(str).into())
		}
	}
}
impl Name for ContainerID {
	fn get_name(&self) -> &str { &self.name }
	fn create_from_string(&self, n: String) -> ContainerID { ContainerID { name: n } }
}
impl fmt::Display for ContainerID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
// Errors
error_chain! {
	errors {
		Format(name: String) {
			description("Name cannot contain blanks")
			display("NameError: '{}' contains blanks.", name)
		}
		Size(name: String) {
			description("Name is too long")
			display("NameError: '{}' is longer than {} characters", name, ::config::MAX_CHARS)
		}
	}
}
