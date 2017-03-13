use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{SendError, RecvError};
use std::collections::HashMap;
use crossbeam::Scope;
use config::{MAX_ENTRIES, MAX_PORTS};
use message::{Sender,Receiver};
use nalcell::{EntrySender, PortStatusReceiver};
use name::{CellID, TreeID};
use traph::{Traph, TraphError};
use routing_table::RoutingTableError;
use routing_table_entry::RoutingTableEntry;
use port::PortStatus;
use utility::{int_to_mask, mask_from_port_nos};

type IndexArray = [usize; MAX_PORTS as usize];
type PortArray = [u8; MAX_PORTS as usize];

const CONTROL_TREE: &'static str = "Control";
const CONNECTED_PORTS_TREE: &'static str = "Connected";
const OTHER_INDICES: IndexArray = [0; MAX_PORTS as usize];

#[derive(Debug)]
pub struct CellAgent {
	cell_id: CellID,
	connected_ports_tree_id: TreeID,
	free_indices: Vec<usize>,
	traphs: Arc<Mutex<HashMap<TreeID,Traph>>>,
}
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, send_to_pe: Sender, recv_from_pe: Receiver, 
		send_entry_to_pe: EntrySender, recv_from_port: PortStatusReceiver) -> Result<CellAgent, CellAgentError> {
		let control_tree_id = try!(TreeID::new(CONTROL_TREE));
		let connected_tree_id = try!(TreeID::new(CONNECTED_PORTS_TREE));
		let mut free_indices = Vec::new();
		for i in 2..MAX_ENTRIES { free_indices.push(i); } // O reserved for control tree, 1 for connected tree
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		let mut ca = CellAgent { cell_id: cell_id.clone(), connected_ports_tree_id: connected_tree_id.clone(),
			free_indices: free_indices, traphs: traphs };
		let entry = try!(ca.new_tree(0, control_tree_id, 0, vec![0], 0, None));
		try!(send_entry_to_pe.send(entry));
		let entry = try!(ca.new_tree(1, connected_tree_id, 0, vec![], 0, None));
		try!(send_entry_to_pe.send(entry));
		try!(ca.port_status(scope, entry, recv_from_port, send_entry_to_pe));
		//thread::spawn( move || { CellAgent::work(cell_id.clone(), send_to_pe, recv_from_pe); } );
		Ok(ca)
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("\nCell Agent {}", self.cell_id);
		for (tree_id, traph) in self.traphs.lock().unwrap().iter() {
			s = s + &traph.stringify();
		}
		s
	}	
	pub fn new_tree(&mut self, index: usize, tree_id: TreeID, parent_no: u8, children: Vec<u8>, 
					hops: usize, path: Option<&TreeID>) -> Result<RoutingTableEntry, CellAgentError> {
		let mask = try!(mask_from_port_nos(children));
		let traph = try!(Traph::new(tree_id.clone(), index));
		self.traphs.lock().unwrap().insert(tree_id.clone(), traph);
		Ok(RoutingTableEntry::new(index, true, 0 as u8, mask, OTHER_INDICES))
	}
	fn port_status(&self, scope: &Scope, entry: RoutingTableEntry, 
			recv_from_port: PortStatusReceiver, send_entry_to_pe: EntrySender) -> Result<(), CellAgentError>{
		let mut entry = entry.clone();	
		scope.spawn( move || -> Result<(), CellAgentError> {
			loop {
				let (port_no, status) = try!(recv_from_port.recv());
				let port_no_mask = try!(int_to_mask(port_no));
				match  status {
					PortStatus::Connected => { 
						let mask = port_no_mask | entry.get_mask();
						entry.set_mask(mask);
						try!(send_entry_to_pe.send(entry));
					},
					PortStatus::Disconnected => {
						let mask = (!port_no_mask) & entry.get_mask();
						entry.set_mask(mask);
						try!(send_entry_to_pe.send(entry));
					}
				}
 			}
		});
		Ok(())
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
	Tree(TreeError),
	Traph(TraphError),
	Utility(UtilityError),
	Routing(RoutingTableError),
	Send(SendError<RoutingTableEntry>),
	Recv(RecvError),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::Size(ref err) => err.description(),
			CellAgentError::Tree(ref err) => err.description(),
			CellAgentError::Traph(ref err) => err.description(),
			CellAgentError::Utility(ref err) => err.description(),
			CellAgentError::Routing(ref err) => err.description(),
			CellAgentError::Send(ref err) => err.description(),
			CellAgentError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::Size(ref err) => Some(err),
			CellAgentError::Tree(ref err) => Some(err),
			CellAgentError::Traph(ref err) => Some(err),
			CellAgentError::Utility(ref err) => Some(err),
			CellAgentError::Routing(ref err) => Some(err),
			CellAgentError::Send(ref err) => Some(err),
			CellAgentError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Name(ref err) => write!(f, "Cell Agent Name Error caused by {}", err),
			CellAgentError::Size(ref err) => write!(f, "Cell Agent Size Error caused by {}", err),
			CellAgentError::Tree(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::Traph(ref err) => write!(f, "Cell Agent Traph Error caused by {}", err),
			CellAgentError::Utility(ref err) => write!(f, "Cell Agent Utility Error caused by {}", err),
			CellAgentError::Routing(ref err) => write!(f, "Cell Agent Routing Table Error caused by {}", err),
			CellAgentError::Send(ref err) => write!(f, "Cell Agent Send Error caused by {}", err),
			CellAgentError::Recv(ref err) => write!(f, "Cell Agent Receive Error caused by {}", err),
		}
	}
}
impl From<NameError> for CellAgentError {
	fn from(err: NameError) -> CellAgentError { CellAgentError::Name(err) }
}
impl From<TraphError> for CellAgentError {
	fn from(err: TraphError) -> CellAgentError { CellAgentError::Traph(err) }
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
impl From<RecvError> for CellAgentError{
	fn from(err: RecvError) -> CellAgentError { CellAgentError::Recv(err) }
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
#[derive(Debug)]
pub struct TreeError { msg: String }
impl TreeError { 
	pub fn new(tree_id: &TreeID) -> TreeError {
		TreeError { msg: format!("TreeID {} does not exist", tree_id) }
	}
}
impl Error for TreeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for TreeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<TreeError> for CellAgentError {
	fn from(err: TreeError) -> CellAgentError { CellAgentError::Tree(err) }
}
