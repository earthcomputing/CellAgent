use std::fmt;
use std::marker::Sized;

use failure::{Error, Fail, ResultExt};
use uuid::Uuid;

use config::{SEPARATOR, CellNo};
use utility::{PortNumber, S};

// Using String means names are not Copy
type NameType = String;
pub trait Name: Sized {
	fn get_name(&self) -> &str;
	fn get_uuid(&self) -> Uuid;
	fn create_from_string(&self, n: String) -> Self;
	// Default implementations
	fn stringify(&self) -> String { self.get_name().to_string() }
	fn from_str(&self, s: &str) -> Result<Self, Error> {
		// Names may not contain blanks
		match s.find(' ') {
			Some(_) => Err(NameError::Format { name: s.to_string(), func_name: "from_str" }.into()),
			None => Ok(self.create_from_string(s.to_string()))
		}
	}
	fn add_component(&self, s: &str) -> Result<Self, Error> {
		match s.find(' ') {
			Some(_) => Err(NameError::Format{ name: s.to_string(), func_name: "add_component" }.into()),
			None => self.from_str(&([self.get_name(),s].join(SEPARATOR)))
		}
	}		
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NameType, uuid: Uuid }
impl CellID {
	pub fn new(CellNo(n): CellNo) -> Result<CellID, NameError> {
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
	pub fn new(cell_id: &CellID, port_number: PortNumber) -> Result<PortID, Error> {
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
	pub fn new(n: &str) -> Result<TreeID, Error> {
		let str = n.to_string();
		let tree_uuid = Uuid::new_v4();
		match n.find(' ') {
			None => Ok(TreeID { name: str, uuid: tree_uuid }),
			Some(_) => Err(NameError::Format { name: str, func_name: "TreeID::new" }.into())
		}
	}
	pub fn append2file(&self) -> Result<(), Error> {
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
pub struct UptreeID { name: NameType, uuid: Uuid}
impl<'a> UptreeID {
	pub fn new(n: &str) -> Result<UptreeID, Error> {
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(UptreeID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(NameError::Format{ name: str, func_name: "UptreeID::new" }.into())
		}
	}
}
impl Name for UptreeID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> UptreeID { UptreeID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for UptreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: NameType, uuid: Uuid }
impl TenantID {
	pub fn new(n: &str) -> Result<TenantID, Error> {
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(TenantID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(NameError::Format { name: str, func_name: "TenantID::new" }.into())
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
	pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<LinkID, Error> {
		let name = [left_id.get_name(), rite_id.get_name()].join(SEPARATOR);
		Ok(LinkID { name: name, uuid: Uuid::new_v4() })
	}
}
impl Name for LinkID {
	fn get_name(&self) -> &str { &self.name }
	fn get_uuid(&self) -> Uuid { self.uuid }
	fn create_from_string(&self, n: String) -> LinkID { LinkID { name: n, uuid: Uuid::new_v4() } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmID { name: NameType, uuid: Uuid}
impl VmID {
	pub fn new(cell_id: &CellID, name: &String) -> Result<VmID, Error> {
		let name = format!("VM:{}+{}", cell_id, name);
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
	pub fn new(n: &str) -> Result<ContainerID, Error> {
		let str = n.to_string();
		match n.find(' ') {
			None => Ok(ContainerID { name: str, uuid: Uuid::new_v4() }),
			Some(_) => Err(NameError::Format { name: str, func_name: "ContainerID::new" }.into())
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
#[derive(Debug, Fail)]
pub enum NameError {
	#[fail(display = "NameError::Format {}: '{}' contains blanks.", func_name, name)]
	Format { func_name: &'static str, name: String }
}
