use std::{fmt, fmt::Write,
          collections::HashMap};

use failure::{Error};

use crate::name::{CellID};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::uuid_ec::Uuid;
use crate::blueprint::BlueprintError::DefaultNumPhysPortsPerCell;

#[derive(Debug, Clone, Default)]
pub struct RoutingTable {
    id: CellID,
    entries: HashMap<Uuid, RoutingTableEntry>,
    order: Vec<Uuid>, // So I can print out the entries in the order they were created for debugging
    connected_ports: Vec<u8>
}
impl RoutingTable {
    pub fn new(id: CellID) -> RoutingTable {
        let mut routing_table: RoutingTable = Default::default();
        routing_table.id = id;
        routing_table
    }
    pub fn get_entry(&self, uuid: Uuid) -> Result<RoutingTableEntry, Error> {
        let _f = "get_entry";
        Ok(*(self.entries
            .get(&uuid)
            .ok_or::<Error>(RoutingTableError::Uuid { func_name: _f, cell_id: self.id, uuid }.into())?))
    }
    pub fn set_entry(&mut self, entry: RoutingTableEntry) {
        let _f = "set_entry";
        let uuid = entry.get_uuid();
        if !self.entries.contains_key(&uuid) { self.order.push(uuid); } // So I can print entries in order
        self.entries.insert(uuid, entry);
    }
    pub fn delete_entry(&mut self, uuid: Uuid) {
        let _f = "delete_entry";
        self.entries.remove(&uuid);
    }
}
impl fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nRouting Table");
        write!(s, "\n Tree UUID  In Use Send? Parent Mask ")?;
        for key in &self.order {
            let entry = self.entries.get(key);
            if entry.is_some() { write!(s, "\n{}", entry.unwrap())?; }
        }
        write!(f, "{}", s)
    }
}
// Errors
#[derive(Debug, Fail)]
pub enum RoutingTableError {
    //#[fail(display = "RoutingTableError::Chain {} {}", func_name, comment)]
    //Chain { func_name: &'static str, comment: String },
    #[fail(display = "RoutingTableError::Uuid {}: {:?} is not a valid routing table uuid on cell {}", func_name, uuid, cell_id)]
    Uuid { func_name: &'static str, uuid: Uuid, cell_id: CellID}
}
