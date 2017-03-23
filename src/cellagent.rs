use std::fmt;
use std::str;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{SendError, RecvError};
use std::hash::Hash;
use std::collections::HashMap;
use std::sync::mpsc;
use serde;
use crossbeam::Scope;
use config::{MAX_ENTRIES, MAX_PORTS, CHUNK_ID_SIZE};
use nalcell::{PortNumber, EntrySender, PortStatusReceiver};
use message::{Message, MsgPayload, DiscoverMsg};
use name::{Name, CellID, TreeID};
use packet::{Packet, Packetizer, PacketizerError};
use port::Port;
use routing_table::RoutingTableError;
use routing_table_entry::RoutingTableEntry;
use port::PortStatus;
use traph::{Traph, TraphError};
use utility::{int_to_mask, mask_from_port_nos};

pub type SendPacketSmall = mpsc::Sender<Packet>;
pub type ReceivePacketSmall = mpsc::Receiver<Packet>;
pub type SendPacketError = SendError<Packet>;

type IndexArray = [usize; MAX_PORTS as usize];
type PortArray = [u8; MAX_PORTS as usize];

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";
const OTHER_INDICES: IndexArray = [0; MAX_PORTS as usize];

#[derive(Debug)]
pub struct CellAgent {
	cell_id: CellID,
	ports: Box<[Port]>,
	connected_ports_tree_id: TreeID,
	free_indices: Vec<usize>,
	traphs: Arc<Mutex<HashMap<TreeID,Traph>>>,
}
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, ports: Box<[Port]>,
			send_to_pe: SendPacketSmall, recv_from_pe: ReceivePacketSmall, send_entry_to_pe: EntrySender, 
			recv_from_port: PortStatusReceiver) -> Result<CellAgent, CellAgentError> {
		let control_tree_id = try!(TreeID::new(CONTROL_TREE_NAME));
		let connected_tree_id = try!(TreeID::new(CONNECTED_PORTS_TREE_NAME));
		let mut free_indices = Vec::new();
		for i in 2..MAX_ENTRIES { free_indices.push(i); } // O reserved for control tree, 1 for connected tree
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		let mut ca = CellAgent { cell_id: cell_id.clone(), ports: ports,
			connected_ports_tree_id: connected_tree_id.clone(), free_indices: free_indices, traphs: traphs };
		// Set up predefined trees
		let entry = try!(ca.new_tree(0, control_tree_id, 0, vec![PortNumber { port_no: 0 }], 0, None));
		try!(send_entry_to_pe.send(entry));
		let connected_entry = try!(ca.new_tree(1, connected_tree_id.clone(), 0, vec![], 0, None));
		try!(send_entry_to_pe.send(entry));
		try!(ca.initiate_discover(connected_tree_id, &send_entry_to_pe));
		try!(ca.port_status(scope, connected_entry, recv_from_port, send_entry_to_pe));
		//thread::spawn( move || { CellAgent::work(cell_id.clone(), send_to_pe, recv_from_pe); } );
		Ok(ca)
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("\nCell Agent {}", self.cell_id);
		for (_, traph) in self.traphs.lock().unwrap().iter() {
			s = s + &traph.stringify();
		}
		s
	}
	pub fn get_no_ports(&self) -> usize { self.ports.len() }	
	pub fn initiate_discover(&mut self, connected_tree_id: TreeID, send_entry_to_pe: &EntrySender) -> Result<(), CellAgentError>{
		// Create my tree
		let index = try!(self.use_index());
		let tree_id = try!(TreeID::new(self.cell_id.get_name()));
		let entry = try!(self.new_tree(index, tree_id.clone(), 0, vec![], 0, None)); 
		try!(send_entry_to_pe.send(entry));
		let msg = DiscoverMsg::new(connected_tree_id, tree_id, self.cell_id.clone(), 0, 0);
		println!("Msg: {}", msg);
		try!(self.send_msg(msg, 0));
		Ok(())
	}
	fn send_msg<T>(&self, msg: T, other_index: u32) -> Result<(), CellAgentError> 
			where T: Message + Hash + serde::Serialize {
		let packets = try!(Packetizer::packetize(&msg, other_index));
		//let deserialized: DiscoverMsg = try!(serde_json::from_str(&serialized));
		Ok(())
	}
	pub fn new_tree(&mut self, index: usize, tree_id: TreeID, parent_no: u8, children: Vec<PortNumber>, 
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
	pub fn get_free_port_mut (&mut self) -> Result<&mut Port,CellAgentError> {
		for p in &mut self.ports.iter_mut() {
			if !p.is_connected() & !p.is_border() { return Ok(p); }
		}
		Err(CellAgentError::NoFreePort(NoFreePortError::new(self.cell_id.clone())))
	}
	pub fn work(cell_id: CellID, send_to_pe: SendPacketSmall, recv_from_pe: ReceivePacketSmall) {
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
	Packetizer(PacketizerError),
	BadPacket(BadPacketError),
	Utility(UtilityError),
	NoFreePort(NoFreePortError),
	Routing(RoutingTableError),
	Send(SendError<RoutingTableEntry>),
	Recv(RecvError),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Packetizer(ref err) => err.description(),
			CellAgentError::BadPacket(ref err) => err.description(),
			CellAgentError::NoFreePort(ref err) => err.description(),
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
			CellAgentError::Packetizer(ref err) => Some(err),
			CellAgentError::BadPacket(ref err) => Some(err),
			CellAgentError::NoFreePort(ref err) => Some(err),
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
			CellAgentError::Packetizer(ref err) => write!(f, "Cell Agent Packetizer Error caused by {}", err),
			CellAgentError::BadPacket(ref err) => write!(f, "Cell Agent Bad Packet Error caused by {}", err),
			CellAgentError::NoFreePort(ref err) => write!(f, "Cell Agent No Free Port Error caused by {}", err),
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
pub struct BadPacketError { msg: String }
impl BadPacketError { 
	pub fn new() -> BadPacketError {
		BadPacketError { msg: format!("No packet created by packetizer") }
	}
}
impl Error for BadPacketError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for BadPacketError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<BadPacketError> for CellAgentError {
	fn from(err: BadPacketError) -> CellAgentError { CellAgentError::BadPacket(err) }
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
#[derive(Debug)]
pub struct NoFreePortError { msg: String }
impl NoFreePortError { 
	pub fn new(cell_id: CellID) -> NoFreePortError {
		NoFreePortError { msg: format!("All ports have been assigned for cell {}", cell_id) }
	}
}
impl Error for NoFreePortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NoFreePortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NoFreePortError> for CellAgentError {
	fn from(err: NoFreePortError) -> CellAgentError { CellAgentError::NoFreePort(err) }
}
impl From<PacketizerError> for CellAgentError {
	fn from(err: PacketizerError) -> CellAgentError { CellAgentError::Packetizer(err) }
}
