use std::fmt;
use std::marker::Sized;

use config::{SEPARATOR, CellNo, PortNo};
use utility::{PortNumber, S};
use uuid_ec::Uuid;

// Using String means names are not Copy
type NameType = String;
pub trait Name: Sized {
    fn get_name(&self) -> &str;
    fn get_uuid(&self) -> Uuid;
    fn create_from_string(&self, n: &str) -> Self;
    // Default implementations
    fn stringify(&self) -> String { S(self.get_name()) }
    fn name_from_str(&self, s: &str) -> Result<Self, Error> {
        // Names may not contain blanks
        match s.find(' ') {
            Some(_) => Err(NameError::Format { name: S(s), func_name: "from_str" }.into()),
            None => Ok(self.create_from_string(s))
        }
    }
    fn add_component(&self, s: &str) -> Result<Self, Error> {
        match s.find(' ') {
            Some(_) => Err(NameError::Format{ name: S(s), func_name: "add_component" }.into()),
            None => self.name_from_str(&([self.get_name(),s].join(SEPARATOR)))
        }
    }
    fn is_name(&self, name: &str) -> bool { self.get_name() == name }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellID { name: NameType, uuid: Uuid }
impl CellID {
    pub fn new(CellNo(n): CellNo) -> Result<CellID, NameError> {
        let name = format!("C:{}",n);
        Ok(CellID { name: name.clone(), uuid: Uuid::new() })
    }
}
impl Name for CellID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> CellID { CellID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortID { name: NameType, uuid: Uuid }
impl PortID {
    pub fn new(cell_id: &CellID, port_number: PortNumber) -> Result<PortID, Error> {
        let name = [cell_id.get_name(), &format!("P:{}",port_number)].join(SEPARATOR);
        Ok(PortID { name: name.clone(), uuid: Uuid::new() })
    }
}
impl Name for PortID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> PortID { PortID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TreeID { name: NameType, uuid: Uuid}
impl TreeID {
    pub fn new(name: &str) -> Result<TreeID, Error> {
        match name.find(' ') {
            None => Ok(TreeID { name: S(name), uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name: S(name), func_name: "TreeID::new" }.into())
        }
    }
    pub fn default() -> TreeID {
        TreeID { name: S("Default"), uuid: Uuid::new() }
    }
    pub fn with_root_port_number(&self, port_number: &PortNumber) -> TreeID {
        let mut uuid = self.uuid;
        uuid.set_port_number(port_number);
        TreeID { name: S(self.get_name()), uuid }
    }
    pub fn without_root_port_number(&self) -> TreeID {
        let mut uuid = self.uuid.clone();
        uuid.remove_port_no();
        TreeID { name: S(self.get_name()), uuid }
    }
    pub fn _get_port_no(&self) -> PortNo { self.uuid.get_port_no() }
    pub fn _transfer_port_number(&mut self, other: &TreeID) {
        self.uuid.set_port_no(other._get_port_no());
    }
}
impl Name for TreeID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> TreeID { TreeID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for TreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UptreeID { name: NameType, uuid: Uuid}
impl UptreeID {
    pub fn new(n: &str) -> Result<UptreeID, Error> {
        let name = S(n);
        match n.find(' ') {
            None => Ok(UptreeID { name: name.clone(), uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format{ name, func_name: "UptreeID::new" }.into())
        }
    }
}
impl Name for UptreeID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> UptreeID { UptreeID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for UptreeID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantID { name: NameType, uuid: Uuid }
impl TenantID {
    /*
    pub fn new(n: &str) -> Result<TenantID, Error> {
        let name = S(n);
        match n.find(' ') {
            None => Ok(TenantID { name: name.clone(), uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name, func_name: "TenantID::new" }.into())
        }
    }
    */
}
impl Name for TenantID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> TenantID { TenantID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LinkID { name: NameType, uuid: Uuid}
impl LinkID {
    pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<LinkID, Error> {
        let name = [left_id.get_name(), rite_id.get_name()].join(SEPARATOR);
        Ok(LinkID { name: name.clone(), uuid: Uuid::new() })
    }
}
impl Name for LinkID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> LinkID { LinkID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmID { name: NameType, uuid: Uuid}
impl VmID {
    pub fn new(cell_id: &CellID, name: &str) -> Result<VmID, Error> {
        let name = format!("VM:{}+{}", cell_id, name);
        Ok(VmID { name: name.clone(), uuid: Uuid::new() })
    }
}
impl Name for VmID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> VmID { VmID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for VmID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SenderID { name: NameType, uuid: Uuid}
impl SenderID {
    pub fn new(cell_id: &CellID, name: &str) -> Result<SenderID, Error> {
        let name = format!("Sender:{}+{}", cell_id, name);
        Ok(SenderID { name: name.clone(), uuid: Uuid::new() })
    }
}
impl Name for SenderID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> SenderID { SenderID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for SenderID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerID { name: NameType, uuid: Uuid}
impl ContainerID {
    pub fn new(n: &str) -> Result<ContainerID, Error> {
        let name = S(n);
        match n.find(' ') {
            None => Ok(ContainerID { name: name.clone(), uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name, func_name: "ContainerID::new" }.into())
        }
    }
}
impl Name for ContainerID {
    fn get_name(&self) -> &str { &self.name }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> ContainerID { ContainerID { name: S(name), uuid: Uuid::new() } }
}
impl fmt::Display for ContainerID { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.name.fmt(f) } }
// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum NameError {
//  #[fail(display = "NameError::Chain {} {}", func_name, comment)]
//  Chain { func_name: &'static str, comment: String },
    #[fail(display = "NameError::Format {}: '{}' contains blanks.", func_name, name)]
    Format { func_name: &'static str, name: String }
}
