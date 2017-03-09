use std::fmt;
use std::thread;
use std::sync::mpsc::SendError;
use std::collections::HashMap;
use config::{MAX_ENTRIES, MAX_PORTS};
use message::{Sender,Receiver};
use nalcell::{EntrySender, PortStatusReceiver};
use name::{CellID, TreeID};
use traph::Traph;
use routing_table::RoutingTableError;
use routing_table_entry::RoutingTableEntry;
use utility::mask_from_port_nos;

type IndexArray = [usize; MAX_PORTS as usize];
type PortArray = [u8; MAX_PORTS as usize];

const CONTROL_TREE: &'static str = "Control";
const CONNECTED_PORTS_TREE: &'static str = "Connected";
const OTHER_INDICES: IndexArray = [0; MAX_PORTS as usize];

#[derive(Debug)]
pub struct CellAgent {
	cell_id: CellID,
	free_indices: Vec<usize>,
	traphs: HashMap<TreeID,Traph>,
	send_entry_to_pe: EntrySender
}
impl CellAgent {
	pub fn new(cell_id: &CellID, send_to_pe: Sender, recv_from_pe: Receiver, send_entry_to_pe: EntrySender,
			recv_from_port: PortStatusReceiver) -> Result<CellAgent, CellAgentError> {
		let mut free_indices = Vec::new();
		for i in 2..MAX_ENTRIES { free_indices.push(i); } // O reserved for control tree, 1 for connected tree
		free_indices.reverse();
		let mut ca = CellAgent { cell_id: cell_id.clone(), free_indices: free_indices, traphs: HashMap::new(),
			 send_entry_to_pe: send_entry_to_pe};
		let tree_id = try!(TreeID::new(CONTROL_TREE));
		let entry = try!(ca.new_tree(0, tree_id, 0, vec![0], 0, None));
		try!(ca.send_entry_to_pe.send(entry));
		let tree_id = try!(TreeID::new(CONNECTED_PORTS_TREE));
		let entry = try!(ca.new_tree(1, tree_id, 0, vec![], 0, None));
		try!(ca.send_entry_to_pe.send(entry));
		//thread::spawn( move || { CellAgent::work(cell_id.clone(), send_to_pe, recv_from_pe); } );
		Ok(ca)
	}
	pub fn new_tree(&mut self, index: usize, tree_id: TreeID, parent_no: u8, children: Vec<u8>, 
					hops: usize, path: Option<&TreeID>) -> Result<RoutingTableEntry, CellAgentError> {
		let mask = try!(mask_from_port_nos(children));
		self.traphs.insert(tree_id.clone(), Traph::new(tree_id, 0));
		Ok(RoutingTableEntry::new(index, false, 0 as u8, mask, OTHER_INDICES))
	}
	fn use_index(&mut self) -> Result<usize,CellAgentError> {
		match self.free_indices.pop() {
			Some(i) => Ok(i),
			None => Err(CellAgentError::Size(SizeError::new()))
		}
	}
	pub fn work(cell_id: CellID, send_to_pe: Sender, recv_from_pe: Receiver) {
		println!("Cell Agent on cell {} is working", cell_id);
	}
}
// Errors
use std::error::Error;
use name::NameError;
use utility::UtilityError;
#[derive(Debug)]
pub enum CellAgentError {
	Name(NameError),
	Size(SizeError),
	Utility(UtilityError),
	Routing(RoutingTableError),
	Send(SendError<RoutingTableEntry>),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::Size(ref err) => err.description(),
			CellAgentError::Utility(ref err) => err.description(),
			CellAgentError::Routing(ref err) => err.description(),
			CellAgentError::Send(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::Size(ref err) => Some(err),
			CellAgentError::Utility(ref err) => Some(err),
			CellAgentError::Routing(ref err) => Some(err),
			CellAgentError::Send(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Name(_) => write!(f, "Cell Agent Name Error caused by"),
			CellAgentError::Size(_) => write!(f, "Cell Agent Size Error caused by"),
			CellAgentError::Utility(_) => write!(f, "Cell Agent Utility Error caused by"),
			CellAgentError::Routing(_) => write!(f, "Cell Agent Routing Table Error caused by"),
			CellAgentError::Send(_) => write!(f, "Cell Agent Send Error caused by"),
		}
	}
}
impl From<NameError> for CellAgentError {
	fn from(err: NameError) -> CellAgentError { CellAgentError::Name(err) }
}
impl From<UtilityError> for CellAgentError {
	fn from(err: UtilityError) -> CellAgentError { CellAgentError::Utility(err) }
}
impl From<RoutingTableError> for CellAgentError{
	fn from(err: RoutingTableError) -> CellAgentError { CellAgentError::Routing(err) }
}
impl From<SendError<RoutingTableEntry>> for CellAgentError{
	fn from(err: SendError<RoutingTableEntry>) -> CellAgentError { CellAgentError::Send(err) }
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
impl From<SizeError> for CellAgentError {
	fn from(err: SizeError) -> CellAgentError { CellAgentError::Size(err) }
}
