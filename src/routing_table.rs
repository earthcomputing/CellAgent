use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use config::{MAX_ENTRIES, MAX_PORTS};
use name::{TreeID};
use routing_table_entry::{RoutingTableEntry, RoutingTableEntryError};
use traph::Traph;

#[derive(Debug)]
pub struct RoutingTable {
	entries: Vec<RoutingTableEntry>,
	connected_ports: Vec<u8>
}
impl RoutingTable {
	pub fn new() -> Result<RoutingTable,RoutingTableError> {
		let default_entry = RoutingTableEntry::new(0, false, 0, 0, [0; MAX_PORTS as usize]);
		let mut entries = Vec::new();
		for i in 1..MAX_ENTRIES {
			let mut entry = RoutingTableEntry::new(0, false, 0, 0, [0; MAX_PORTS as usize]); 
			entry.set_index(i);
			entries.push(entry);
		}
		Ok(RoutingTable { entries: entries, connected_ports: Vec::new() })
	}
	pub fn get_entry(&self, index: u32) -> Result<RoutingTableEntry, RoutingTableError> { 
		match self.entries.get(index as usize) {
			Some(e) => Ok(*e),
			None => Err(RoutingTableError::Index(IndexError::new(index)))
		}
	}
	pub fn set_entry(&mut self, entry: RoutingTableEntry) { self.entries[entry.get_index()] = entry; }
	pub fn stringify(&self) -> String {
		let mut s = format!("\nRouting Table with {} Entries", MAX_ENTRIES);
		s = s + &format!("\n Index In Use Parent Mask             Indices");
		for entry in self.entries.iter() {
			if entry.get_index() < 8 { s = s + &entry.stringify(); }
		}
		s
	}
}
// Errors
use std::error::Error;
use name::NameError;
#[derive(Debug)]
pub enum RoutingTableError {
	Name(NameError),
	Size(SizeError),
	Index(IndexError),
	RoutingTableEntry(RoutingTableEntryError)
}
impl Error for RoutingTableError {
	fn description(&self) -> &str {
		match *self {
			RoutingTableError::Name(ref err) => err.description(),
			RoutingTableError::Size(ref err) => err.description(),
			RoutingTableError::Index(ref err) => err.description(),
			RoutingTableError::RoutingTableEntry(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			RoutingTableError::Name(ref err) => Some(err),
			RoutingTableError::Size(ref err) => Some(err),
			RoutingTableError::Index(ref err) => Some(err),
			RoutingTableError::RoutingTableEntry(ref err) => Some(err),
		}
	}
}
impl fmt::Display for RoutingTableError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			RoutingTableError::Name(ref err) => write!(f, "Routing Table Name Error caused by {}", err),
			RoutingTableError::Size(ref err) => write!(f, "Routing Table Size Error caused by {}", err),
			RoutingTableError::Index(ref err) => write!(f, "Routing Table Index Error caused by {}", err),
			RoutingTableError::RoutingTableEntry(ref err) => write!(f, "Routing Table Entry Error caused by {}", err),
		}
	}
}
impl From<NameError> for RoutingTableError {
	fn from(err: NameError) -> RoutingTableError { RoutingTableError::Name(err) }
}
impl From<RoutingTableEntryError> for RoutingTableError {
	fn from(err: RoutingTableEntryError) -> RoutingTableError { RoutingTableError::RoutingTableEntry(err) }
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError { 
	pub fn new() -> SizeError {
		SizeError { msg: format!("No more room in routing table") }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<SizeError> for RoutingTableError {
	fn from(err: SizeError) -> RoutingTableError { RoutingTableError::Size(err) }
}
#[derive(Debug)]
pub struct IndexError { msg: String }
impl IndexError { 
	pub fn new(index: u32) -> IndexError {
		IndexError { msg: format!("{} is not a valid routing table index", index) }
	}
}
impl Error for IndexError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for IndexError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<IndexError> for RoutingTableError {
	fn from(err: IndexError) -> RoutingTableError { RoutingTableError::Index(err) }
}
