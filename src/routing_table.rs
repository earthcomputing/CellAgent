use std::{fmt, collections::HashMap};

use failure::{Error};

use crate::name::{CellID};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::uuid_ec::Uuid;

#[derive(Debug)]
pub struct RoutingTable {
    id: CellID,
    entries: HashMap<Uuid, RoutingTableEntry>,
    order: Vec<Uuid>, // So I can print out the entries in the order they were created for debugging
    connected_ports: Vec<u8>
}
impl RoutingTable {
    pub fn new(id: CellID) -> Result<RoutingTable, Error> {
        Ok(RoutingTable { id, entries: HashMap::new(), connected_ports: Vec::new(), order: Vec::new() })
    }
    pub fn get_entry(&self, uuid: Uuid) -> Result<RoutingTableEntry, Error> {
        let _f = "get_entry";
        Ok(self.entries
            .get(&uuid)
            .ok_or_else(|| -> Error { RoutingTableError::Uuid { func_name: _f, cell_id: self.id.clone(), uuid }.into() })?
            .clone())
    }
    pub fn set_entry(&mut self, entry: RoutingTableEntry) {
        let _f = "set_entry";
        let uuid = entry.get_uuid();
        if !self.entries.contains_key(&uuid) { self.order.push(uuid); } // So I can print entries in order
        self.entries.insert(uuid, entry);
    }
}
impl fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = format!("\nRouting Table");
        s = s + &format!("\n Tree UUID  In Use Send? Parent Mask ");
        for key in &self.order {
            let entry = self.entries.get(key).unwrap();
            s = s + &format!("\n{}", entry);
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
