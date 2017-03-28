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
use nalcell::{PortNumber, PortNumberError, EntrySender, PortStatusReceiver};
use message::{Message, MsgType, MsgPayload, DiscoverMsg};
use name::{Name, CellID, TreeID};
use packet::{Packet, Packetizer, PacketizerError, UnpacketizeError};
use port::Port;
use routing_table::RoutingTableError;
use routing_table_entry::RoutingTableEntry;
use port::PortStatus;
use traph::{Traph, TraphError};
use utility::{int_to_mask, mask_from_port_nos, ints_from_mask, UnimplementedError};

pub type SendPacketCaToPe = mpsc::Sender<(u32, u16, Packet)>;
pub type ReceivePacketPeFromCa = mpsc::Receiver<(u32, u16, Packet)>;

type IndexArray = [usize; MAX_PORTS as usize];

const CONTROL_TREE_NAME: &'static str = "Control";
const CONNECTED_PORTS_TREE_NAME: &'static str = "Connected";
const OTHER_INDICES: IndexArray = [0; MAX_PORTS as usize];
const BASE_TENANT_MASK: u16 = 255;   // All ports
const DEFAULT_USER_MASK: u16 = 254;  // All ports except port 0
const CONTROL_TREE_OTHER_INDEX: u32 = 0;
const CONNECTED_TREE_INDEX: u32 = 1;

#[derive(Debug)]
pub struct CellAgent {
	cell_id: CellID,
	ports: Box<[Port]>,
	connected_ports_tree_id: TreeID,
	free_indices: Vec<usize>,
	traphs: Arc<Mutex<HashMap<TreeID,Traph>>>,
	tenant_masks: Vec<u16>,
	send_to_pe: SendPacketCaToPe,
	recv_from_pe: ReceivePacketPeFromCa,
}
impl CellAgent {
	pub fn new(scope: &Scope, cell_id: &CellID, ports: Box<[Port]>,
			send_to_pe: SendPacketCaToPe, recv_from_pe: ReceivePacketPeFromCa, send_entry_to_pe: EntrySender, 
			recv_from_port: PortStatusReceiver) -> Result<CellAgent, CellAgentError> {
		let tenant_masks = vec![BASE_TENANT_MASK];
		let control_tree_id = try!(TreeID::new(CONTROL_TREE_NAME));
		let connected_tree_id = try!(TreeID::new(CONNECTED_PORTS_TREE_NAME));
		let mut free_indices = Vec::new();
		for i in 2..MAX_ENTRIES { free_indices.push(i); } // O reserved for control tree, 1 for connected tree
		free_indices.reverse();
		let traphs = Arc::new(Mutex::new(HashMap::new()));
		let mut ca = CellAgent { cell_id: cell_id.clone(), ports: ports, traphs: traphs,
			connected_ports_tree_id: connected_tree_id.clone(), free_indices: free_indices,
			tenant_masks: tenant_masks, send_to_pe: send_to_pe.clone(), recv_from_pe: recv_from_pe };
		// Set up predefined trees
		let entry = try!(ca.new_tree(0, control_tree_id, 0, vec![PortNumber { port_no: 0 }], 0, None));
		try!(send_entry_to_pe.send(entry));
		let connected_entry = try!(ca.new_tree(1, connected_tree_id.clone(), 0, vec![], 0, None));
		try!(send_entry_to_pe.send(entry));
		try!(ca.port_status(scope, connected_tree_id, connected_entry, recv_from_port, send_entry_to_pe));
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
	pub fn get_no_ports(&self) -> u8 { self.ports.len() as u8 }	
	pub fn new_tree(&mut self, index: usize, tree_id: TreeID, parent_no: u8, children: Vec<PortNumber>, 
					hops: usize, path: Option<&TreeID>) -> Result<RoutingTableEntry, CellAgentError> {
		let mask = try!(mask_from_port_nos(children));
		let traph = try!(Traph::new(tree_id.clone(), index));
		self.traphs.lock().unwrap().insert(tree_id.clone(), traph);
		Ok(RoutingTableEntry::new(index, true, 0 as u8, mask, OTHER_INDICES))
	}
	fn port_status(&mut self, scope: &Scope, connected_tree_id: TreeID, control_tree_entry: RoutingTableEntry,
			recv_from_port: PortStatusReceiver, send_entry_to_pe: EntrySender) -> Result<(), CellAgentError>{
		// Create my tree
		let index = try!(self.use_index());
		let tree_id = try!(TreeID::new(self.cell_id.get_name()));
		let entry = try!(self.new_tree(index, tree_id.clone(), 0, vec![], 0, None)); 
		try!(send_entry_to_pe.send(entry));
		let tenant_mask = self.tenant_masks[0];//.last().expect("CellAgent: initiate_discover: No tenant mask");
		let mut entry = control_tree_entry.clone();	
		// I'm getting a lifetime error when I have PortNumber::new inside the spawn
		let no_ports = self.get_no_ports();
		let cell_id = self.cell_id.clone();
		let send_to_pe = self.send_to_pe.clone();
		scope.spawn( move || -> Result<(), CellAgentError> {
			loop {
				let (port_no, status) = try!(recv_from_port.recv());
				let port_no_mask = try!(int_to_mask(port_no));
				let port_number = try!(PortNumber::new(port_no, no_ports));
				match  status {
					PortStatus::Connected => { 
						let mask = port_no_mask | entry.get_mask();
						entry.set_mask(mask);
						try!(send_entry_to_pe.send(entry));
						let msg = DiscoverMsg::new(connected_tree_id.clone(), tree_id.clone(), 
									cell_id.clone(), 0, port_number);
						println!("CellAgent {} sending packet", cell_id);
						try!(CellAgent::send_msg(msg, CONNECTED_TREE_INDEX, CONTROL_TREE_OTHER_INDEX, 
								tenant_mask & DEFAULT_USER_MASK, send_to_pe.clone()));
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
	fn send_msg<T>(msg: T, this_index: u32, other_index: u32, mask: u16,
				send_to_pe: SendPacketCaToPe) -> Result<(), CellAgentError> 
			where T: Message + Hash + serde::Serialize + fmt::Display {
		let packets = try!(Packetizer::packetize(&msg, other_index, [false;4]));
		for packet in packets.iter() {
			try!(send_to_pe.send((this_index, mask, **packet)));
		}
		Ok(())
	}
	fn recv_msg<T>(&self, packets: Vec<Box<Packet>>) -> Result<(), CellAgentError>
			where T: Message + Hash + serde::Deserialize + fmt::Display {
		let msg: T = match Packetizer::unpacketize(&packets) {
			Ok(m) => {
				if packets[0].get_size() == 0 { // At least one packet if I get here
					return Err(CellAgentError::Utility(UtilityError::Unimplemented(UnimplementedError::new("Streaming")))); 
				} else {
					m
				}
			},
			Err(e) => return Err(CellAgentError::Packetizer(e))
		};
		match msg.get_msg_type() {
			MsgType::Discover  => return Err(CellAgentError::Utility(UtilityError::Unimplemented(UnimplementedError::new("Discover")))),
			MsgType::DiscoverD => return Err(CellAgentError::Utility(UtilityError::Unimplemented(UnimplementedError::new("DiscoverD")))),
		};
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
	pub fn work(cell_id: CellID, send_to_pe: SendPacketCaToPe, recv_from_pe: ReceivePacketPeFromCa) {
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
	PortNumber(PortNumberError),
	BadPacket(BadPacketError),
	Utility(UtilityError),
	NoFreePort(NoFreePortError),
	Routing(RoutingTableError),
	SendTableEntry(SendError<RoutingTableEntry>),
	SendPacket(SendError<(u32,u16,Packet)>),
	Recv(RecvError),
}
impl Error for CellAgentError {
	fn description(&self) -> &str {
		match *self {
			CellAgentError::Packetizer(ref err) => err.description(),
			CellAgentError::PortNumber(ref err) => err.description(),
			CellAgentError::BadPacket(ref err) => err.description(),
			CellAgentError::NoFreePort(ref err) => err.description(),
			CellAgentError::Name(ref err) => err.description(),
			CellAgentError::Size(ref err) => err.description(),
			CellAgentError::Tree(ref err) => err.description(),
			CellAgentError::Traph(ref err) => err.description(),
			CellAgentError::Utility(ref err) => err.description(),
			CellAgentError::Routing(ref err) => err.description(),
			CellAgentError::SendTableEntry(ref err) => err.description(),
			CellAgentError::SendPacket(ref err) => err.description(),
			CellAgentError::Recv(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			CellAgentError::Packetizer(ref err) => Some(err),
			CellAgentError::PortNumber(ref err) => Some(err),
			CellAgentError::BadPacket(ref err) => Some(err),
			CellAgentError::NoFreePort(ref err) => Some(err),
			CellAgentError::Name(ref err) => Some(err),
			CellAgentError::Size(ref err) => Some(err),
			CellAgentError::Tree(ref err) => Some(err),
			CellAgentError::Traph(ref err) => Some(err),
			CellAgentError::Utility(ref err) => Some(err),
			CellAgentError::Routing(ref err) => Some(err),
			CellAgentError::SendTableEntry(ref err) => Some(err),
			CellAgentError::SendPacket(ref err) => Some(err),
			CellAgentError::Recv(ref err) => Some(err),
		}
	}
}
impl fmt::Display for CellAgentError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CellAgentError::Packetizer(ref err) => write!(f, "Cell Agent Packetizer Error caused by {}", err),
			CellAgentError::PortNumber(ref err) => write!(f, "Cell Agent PortNumber Error caused by {}", err),
			CellAgentError::BadPacket(ref err) => write!(f, "Cell Agent Bad Packet Error caused by {}", err),
			CellAgentError::NoFreePort(ref err) => write!(f, "Cell Agent No Free Port Error caused by {}", err),
			CellAgentError::Name(ref err) => write!(f, "Cell Agent Name Error caused by {}", err),
			CellAgentError::Size(ref err) => write!(f, "Cell Agent Size Error caused by {}", err),
			CellAgentError::Tree(ref err) => write!(f, "Cell Agent Tree Error caused by {}", err),
			CellAgentError::Traph(ref err) => write!(f, "Cell Agent Traph Error caused by {}", err),
			CellAgentError::Utility(ref err) => write!(f, "Cell Agent Utility Error caused by {}", err),
			CellAgentError::Routing(ref err) => write!(f, "Cell Agent Routing Table Error caused by {}", err),
			CellAgentError::SendTableEntry(ref err) => write!(f, "Cell Agent Send Table Entry Error caused by {}", err),
			CellAgentError::SendPacket(ref err) => write!(f, "Cell Agent Send Packet Error caused by {}", err),
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
	fn from(err: SendError<RoutingTableEntry>) -> CellAgentError { CellAgentError::SendTableEntry(err) }
}
impl From<SendError<(u32,u16,Packet)>> for CellAgentError{
	fn from(err: SendError<(u32,u16,Packet)>) -> CellAgentError { CellAgentError::SendPacket(err) }
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
impl From<PortNumberError> for CellAgentError {
	fn from(err: PortNumberError) -> CellAgentError { CellAgentError::PortNumber(err) }
}
