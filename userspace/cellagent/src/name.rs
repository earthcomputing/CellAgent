use std::{fmt, marker::Sized};
use std::string::String;

use arrayvec::ArrayString;

use crate::config::{MAX_CHARS, SEPARATOR};
use crate::utility::{PortNo, PortNumber, S};
use crate::uuid_ec::Uuid;

type NameType = ArrayString<[u8; MAX_CHARS]>;

fn str_to_chars(string: &str) -> NameType {
    let _f = "str_to_chars";
    ArrayString::from(string).expect(&format!("String |{}| is longer than {} characters {}", string, MAX_CHARS, string.len()))
}
fn str_from_chars(chars: NameType) -> String {
    let _f = "str_from_chars";
    chars.as_str().to_owned()
}

pub trait Name: Sized {
    fn get_name(&self) -> String;
    fn get_uuid(&self) -> Uuid;
    fn create_from_string(&self, n: &str) -> Self;
    // Default implementations
    fn stringify(&self) -> String { self.get_name() }
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
            None => self.name_from_str(&([self.get_name(), S(s)].join(SEPARATOR)))
        }
    }
    fn is_name(&self, name: &str) -> bool { self.get_name() == name }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct CellID {
    name: NameType,
    uuid: Uuid
}
impl CellID {
    pub fn new(name: &str) -> Result<CellID, NameError> {
        Ok(CellID { name: str_to_chars(name), uuid: Uuid::new() })
    }
    pub fn get_name(&self) -> String {
        str_from_chars(self.name)
    }
}
impl Name for CellID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> CellID { CellID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for CellID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortID {
    name: NameType,
    uuid: Uuid
}
impl PortID {
    pub fn new(cell_id: CellID, port_number: PortNumber) -> Result<PortID, Error> {
        let name = str_to_chars(&([cell_id.get_name(), format!("P:{}", port_number)].join(SEPARATOR)));
        Ok(PortID { name, uuid: Uuid::new() })
    }
}
impl Name for PortID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> PortID { PortID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for PortID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TreeID {
    name: NameType,
    uuid: Uuid
}
impl TreeID {
    pub fn new(name: &str) -> Result<TreeID, Error> {
        match name.find(' ') {
            None => Ok(TreeID { name: str_to_chars(&(S("Tree:") + name)), uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name: S(name), func_name: "TreeID::new" }.into())
        }
    }
    pub fn to_port_tree_id(&self, port_number: PortNumber) -> PortTreeID {
        let mut uuid = self.uuid;
        uuid.set_port_number(port_number);
        PortTreeID { name: str_to_chars(&self.get_name()), uuid }
    }
    pub fn to_port_tree_id_0(&self) -> PortTreeID { self.to_port_tree_id(PortNumber::new0()) }
}
impl Name for TreeID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> TreeID { TreeID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl Default for TreeID {
    fn default() -> Self {
        TreeID { name: str_to_chars("Default"), uuid: Uuid::new() }
    }
}
impl fmt::Display for TreeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut uuid = S(self.uuid);
        uuid.truncate(8);
        let s = format!("{} {}", str_from_chars(self.name), uuid);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortTreeID {
    name: NameType,
    uuid: Uuid
}
impl PortTreeID {
    pub fn to_tree_id(&self) -> TreeID {
        let mut uuid = self.uuid; // Copy so next line doesn't change self.uuid
        uuid.remove_port_no();
        TreeID { name: str_to_chars(&self.get_name()), uuid }
    }
    pub fn get_port_no(&self) -> PortNo { self.uuid.get_port_no() }
    pub fn _transfer_port_number(&mut self, other: PortTreeID) {
        self.uuid.set_port_no(other.get_port_no());
    }
}
impl Name for PortTreeID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, _name: &str) -> PortTreeID { unimplemented!() }
}
impl Default for PortTreeID {
    fn default() -> Self {
        TreeID::default().to_port_tree_id_0()
    }
}
impl fmt::Display for PortTreeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut uuid = S(self.uuid);
        uuid.truncate(8);
        let s = format!("{} {}", str_from_chars(self.name), uuid);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UptreeID {
    name: NameType,
    uuid: Uuid
}
impl UptreeID {
    pub fn new(n: &str) -> Result<UptreeID, Error> {
        let name = str_to_chars(n);
        match n.find(' ') {
            None => Ok(UptreeID { name, uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format{ name: S(n), func_name: "UptreeID::new" }.into())
        }
    }
}
impl Name for UptreeID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> UptreeID { UptreeID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for UptreeID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantID {
    name: NameType,
    uuid: Uuid
}
impl TenantID {
    /*
    pub fn new(n: &str) -> Result<TenantID, Error> {
        let name = S(n);
        match n.find(' ') {
            None => Ok(TenantID { name, uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name, func_name: "TenantID::new" }.into())
        }
    }
    */
}
impl Name for TenantID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> TenantID { TenantID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for TenantID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LinkID {
    name: NameType,
    uuid: Uuid
}
impl LinkID {
    pub fn new(left_id: PortID, rite_id: PortID) -> Result<LinkID, Error> {
        let name = str_to_chars(&[left_id.get_name(), rite_id.get_name()].join(SEPARATOR));
        Ok(LinkID { name, uuid: Uuid::new() })
    }
}
impl Name for LinkID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> LinkID { LinkID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for LinkID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmID {
    name: NameType,
    uuid: Uuid
}
impl VmID {
    pub fn new(cell_id: CellID, name: &str) -> Result<VmID, Error> {
        let name = str_to_chars(&format!("VM:{}+{}", cell_id, name));
        Ok(VmID { name, uuid: Uuid::new() })
    }
}
impl Name for VmID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> VmID { VmID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for VmID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OriginatorID { // Originator of a message that may be passed through many cells
    name: NameType,
    uuid: Uuid
}
impl OriginatorID {
    pub fn new(cell_id: CellID, name: &str) -> Result<OriginatorID, Error> {
        let name = str_to_chars(&format!("Originator:{}+{}", cell_id, name));
        // All OriginatorIDs with the same name have the same UUID
        Ok(OriginatorID { name, uuid: Uuid::default() })
    }
}
impl Name for OriginatorID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> OriginatorID { OriginatorID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for OriginatorID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerID {
    name: NameType,
    uuid: Uuid
}
impl ContainerID {
    pub fn new(n: &str) -> Result<ContainerID, Error> {
        let name = str_to_chars(n);
        match n.find(' ') {
            None => Ok(ContainerID { name, uuid: Uuid::new() }),
            Some(_) => Err(NameError::Format { name: S(n), func_name: "ContainerID::new" }.into())
        }
    }
}
impl Name for ContainerID {
    fn get_name(&self) -> String { str_from_chars(self.name) }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn create_from_string(&self, name: &str) -> ContainerID { ContainerID { name: str_to_chars(name), uuid: Uuid::new() } }
}
impl fmt::Display for ContainerID { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(self.name).fmt(f) } }
// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum NameError {
//  #[fail(display = "NameError::Chain {} {}", func_name, comment)]
//  Chain { func_name: &'static str, comment: String },
    #[fail(display = "NameError::Format {}: '{}' contains blanks.", func_name, name)]
    Format { func_name: &'static str, name: String }
}