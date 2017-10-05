use std::fmt;
use std::marker::Sized;
use uuid::Uuid;

use config::{SEPARATOR, CellNo};
use utility::{PortNumber};

// Using String means names are not Copy
type NameType = String;
pub trait Name: Sized {
	fn get_name(&self) -> &str;
	fn get_uuid(&self) -> Uuid;
	fn create_from_string(&self, n: String) -> Self;
	// Default implementations
	fn stringify(&self) -> String { self.get_name().to_string() }
	fn from_str(&self, s: &str) -> Result<Self> {
		// Names may not contain blanks
		match s.find(' ') {
			Some(_) => Err(ErrorKind::Format(s.to_string(), "from_str".to_string()).into()), 
			None => Ok(self.create_from_string(s.to_string()))
		}
	}
	fn add_component(&self, s: &str) -> Result<Self> {	
		match s.find(' ') {
			Some(_) => Err(ErrorKind::Format(s.to_string(), "add_component".to_string()).into()),
			None => self.from_str(&([self.get_name(),s].join(SEPARATOR)))
		}
	}		
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NameType, uuid: Uuid }
impl CellID {
	pub fn new(CellNo(n): CellNo) -> Result<CellID> { 
		let name = format!("C:{}",n);
		Ok(CellID { name: name, uuid: Uuid::new_v4() })
	}
}
impl Name for CellID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> CellID { CellID { name: n, uuid: Uuid::new_v4() } }	
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortID { name: NameType, uuid: Uuid }
impl PortID {
	pub fn new(cell_id: &CellID, port_number: PortNumber) -> Result<PortID> { 
		let name = [cell_id.get_name(), &format!("P:{}",port_number)].join(SEPARATOR); 
		Ok(PortID { name: name, uuid: Uuid::new_v4() })
	}
}
impl Name for PortID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> PortID { PortID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TreeID { name: NameType, uuid: Uuid}
impl<'a> TreeID {
	pub fn new(n: &str) -> Result<TreeID> { 
		let str = n.to_string();
		let tree_uuid = Uuid::new_v4();
		match n.find(' ') {
			None => Ok(TreeID { name: str, uuid: tree_uuid }),
			Some(_) => Err(ErrorKind::Format(str, "TreeID::new".to_string()).into())
		}
	}
	pub fn append2file(&self) -> Result<()> {
		let json = ::serde_json::to_string(&self)?;
		::utility::append2file(json)?;
		Ok(())
	}
}
impl Name for TreeID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> TreeID { TreeID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UpTraphID { name: NameType, uuid: Uuid}
impl<'a> UpTraphID {
	pub fn new(n: &str) -> Result<UpTraphID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(UpTraphID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(ErrorKind::Format(str, "UpTraphID::new".to_string()).into())
		}
	}
}
impl Name for UpTraphID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> UpTraphID { UpTraphID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for UpTraphID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: NameType, uuid: Uuid }
impl TenantID {
	pub fn new(n: &str) -> Result<TenantID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(TenantID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(ErrorKind::Format(str, "TenantID::new".to_string()).into())
		}
	}
}
impl Name for TenantID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> TenantID { TenantID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LinkID { name: NameType, uuid: Uuid}
impl LinkID {
	pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<LinkID> { 
		let name = [left_id.get_name(),rite_id.get_name()].join(SEPARATOR);
		Ok(LinkID { name: name, uuid: Uuid::new_v4() })
	}
}
impl Name for LinkID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> LinkID { LinkID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct VmID { name: NameType, uuid: Uuid}
impl VmID {
	pub fn new(cell_id: &CellID, id_no: usize) -> Result<VmID> { 
		let name = format!("VM:{}+{}", cell_id, id_no);
		Ok(VmID { name: name, uuid: Uuid::new_v4() })
	}
}
impl Name for VmID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> VmID { VmID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for VmID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerID { name: NameType, uuid: Uuid}
impl ContainerID {
	pub fn new(n: &str) -> Result<ContainerID> { 
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(ContainerID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(ErrorKind::Format(str, "ContainerID::new".to_string()).into())
		}
	}
}
impl Name for ContainerID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> ContainerID { ContainerID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for ContainerID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
// Errors
error_chain! {
	foreign_links {
		Serialize(::serde_json::Error);
	}
	links {
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors {
		Format(name: String, func_name: String) {
			display("Name {}: '{}' contains blanks.", func_name, name)
		}
	}
}
