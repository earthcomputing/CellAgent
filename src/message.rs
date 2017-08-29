use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};

use serde_json;
use uuid::Uuid;

use cellagent::{CellAgent};
use config::{MAX_PORTS, CellNo, MsgID, PathLength, PortNo, TableIndex};
use container::Service;
use gvm_equation::{GvmEquation, GvmVariable, GvmVariableType};
use name::{Name, CellID, TreeID, UpTraphID};
use packet::{Packet, Packetizer, Serializer};
use traph;
use utility::{DEFAULT_USER_MASK, Mask, Path, PortNumber};

static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> MsgID { MsgID(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) as u64) } 
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TypePlusMsg {
	msg_type: MsgType,
	serialized_msg: String
}
impl TypePlusMsg {
	pub fn new(msg_type: MsgType, serialized_msg: String) -> TypePlusMsg {
		TypePlusMsg { msg_type: msg_type, serialized_msg: serialized_msg }
	}
	fn get_type(&self) -> MsgType { self.msg_type }
	fn get_serialized_msg(&self) -> &str { &self.serialized_msg }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgType {
	Discover,
	DiscoverD,
	SetupVM,
	StackTree,
	Placeholder
}
impl MsgType {
	pub fn get_type_serialized(packets: &Vec<Packet>) -> Result<(MsgType, String)> {
		let serialized = Packetizer::unpacketize(packets).chain_err(|| ErrorKind::MessageError)?;
		let type_msg: TypePlusMsg = serde_json::from_str(&serialized).chain_err(|| ErrorKind::MessageError)?;
		let msg_type = type_msg.get_type();
		let serialized_msg = type_msg.get_serialized_msg();
		Ok((msg_type, serialized_msg.to_string()))
	}
}
impl fmt::Display for MsgType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgType::Discover  => write!(f, "Discover"),
			MsgType::DiscoverD => write!(f, "DiscoverD"),
			MsgType::StackTree => write!(f, "StackTree"),
			MsgType::SetupVM   => write!(f, "SetupVM"),
			_ => write!(f, "{} is an undefined type", self)
		}
	}
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgDirection {
	Rootward,
	Leafward
}
impl fmt::Display for MsgDirection {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgDirection::Rootward => write!(f, "Rootward"),
			MsgDirection::Leafward => write!(f, "Leafward")
		}
	}
}

pub trait Message: fmt::Display {
	fn get_header(&self) -> &MsgHeader;
	fn get_payload(&self) -> &MsgPayload;
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn get_count(&self) -> MsgID { self.get_header().get_count() }
	fn get_tree_id(&self, tree_name: &String) -> Result<&TreeID> {
		let tree_map = self.get_header().get_tree_map();
		Ok(match tree_map.get(tree_name) {
			Some(id) => id,
			None => return Err(ErrorKind::TreeMapEntry(tree_name.clone(), "get_tree_id".to_string()).into())
		})
	}
	fn process(&mut self, cell_agent: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()>;
}
pub trait MsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation>;
	fn get_tree_name(&self) -> &String;
}
type TreeMap = HashMap<String, TreeID>;
#[derive(Debug, Clone, Serialize, Deserialize)]
// Header may not contain '{' or '}' or a separate object, such as TreeID
pub struct MsgHeader {
	msg_count: MsgID,
	msg_type: MsgType,
	direction: MsgDirection,
	tree_map: TreeMap
}
impl MsgHeader {
	pub fn new(msg_type: MsgType, direction: MsgDirection, tree_map: TreeMap) -> MsgHeader {
		let msg_count = get_next_count();
		MsgHeader { msg_type: msg_type, direction: direction, msg_count: msg_count, tree_map: tree_map }
	}
	pub fn get_msg_type(&self) -> MsgType { self.msg_type }
	pub fn get_count(&self) -> MsgID { self.msg_count }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
	pub fn get_tree_map(&self) -> &TreeMap { &self.tree_map }
	//pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for MsgHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Message {} {} '{}'", *self.msg_count, self.msg_type, self.direction);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverMsg {
	header: MsgHeader,
	payload: DiscoverPayload
}
impl DiscoverMsg {
	pub fn new(tree_id: &TreeID, my_index: TableIndex, sending_cell_id: &CellID, 
			hops: PathLength, path: Path) -> DiscoverMsg {
		let tree_name = tree_id.stringify();
		let mut tree_map = HashMap::new();
		tree_map.insert(tree_name.clone(), tree_id.clone());
		let header = MsgHeader::new(MsgType::Discover, MsgDirection::Leafward, tree_map);
		//println!("DiscoverMsg: msg_count {}", header.get_count());
		let payload = DiscoverPayload::new(tree_name, my_index, &sending_cell_id, hops, path);
		DiscoverMsg { header: header, payload: payload }
	}
	pub fn update_discover_msg(&mut self, cell_id: CellID, index: TableIndex) {
		let hops = self.update_hops();
		let path = self.update_path();
		self.payload.set_hops(hops);
		self.payload.set_path(path);
		self.payload.set_index(index);
		self.payload.set_sending_cell(cell_id);
	}
	fn update_hops(&self) -> PathLength { self.payload.hops_plus_one() }
	fn update_path(&self) -> Path { self.payload.get_path() } // No change per hop
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		let port_number = PortNumber::new(port_no, ca.get_no_ports()).chain_err(|| ErrorKind::MessageError)?;
		let hops = self.payload.get_hops();
		let path = self.payload.get_path();
		let my_index;
		{ // Limit scope of immutable borrow of self on the next line
			let new_tree_id = self.get_tree_id(self.payload.get_tree_name())?;
			let senders_index = self.payload.get_index();
			let children = &mut HashSet::new();
			//println!("DiscoverMsg: tree_id {}, port_number {}", tree_id, port_number);
			let exists = ca.exists(&new_tree_id);  // Have I seen this tree before?
			let status = if exists { traph::PortStatus::Pruned } else { traph::PortStatus::Parent };
			let gvm_equation = GvmEquation::new("true", "true", "true", "true", Vec::new());
			let entry = ca.update_traph(&new_tree_id, port_number, status, Some(gvm_equation),
					children, senders_index, hops, Some(path)).chain_err(|| ErrorKind::MessageError)?;
			if exists { 
				return Ok(()); // Don't forward if traph exists for this tree - Simple quenching
			}
			my_index = entry.get_index();
			// Send DiscoverD to sender
			let discoverd_msg = DiscoverDMsg::new(new_tree_id.clone(), my_index);
			let direction = discoverd_msg.get_header().get_direction();
			let bytes = Serializer::serialize(&discoverd_msg).chain_err(|| ErrorKind::MessageError)?;
			let packets = Packetizer::packetize(&ca.get_connected_ports_tree_id(), bytes, direction).chain_err(|| ErrorKind::MessageError)?;
			//println!("DiscoverMsg {}: sending discoverd for tree {} packet {} {}",ca.get_id(), new_tree_id, packets[0].get_count(), discoverd_msg);
			let mask = Mask::new(port_number);
			ca.send_msg(ca.get_connected_ports_tree_id().get_uuid(), &packets, mask).chain_err(|| ErrorKind::MessageError)?;
			// Forward Discover on all except port_no with updated hops and path
		}
		self.update_discover_msg(ca.get_id(), my_index);
		let control_tree_index = 0;
		let direction = self.get_header().get_direction();
		let bytes = Serializer::serialize(&self.clone()).chain_err(|| ErrorKind::MessageError)?;
		let packets = Packetizer::packetize(&ca.get_control_tree_id(), bytes, direction).chain_err(|| ErrorKind::MessageError)?;
		let user_mask = DEFAULT_USER_MASK.all_but_port(PortNumber::new(port_no, ca.get_no_ports()).chain_err(|| ErrorKind::MessageError)?);
		//println!("DiscoverMsg {}: forwarding packet {} on connected ports {}", ca.get_id(), packets[0].get_count(), self);
		ca.add_saved_msg(&packets); // Discover message are always saved for late port connect
		ca.send_msg(ca.get_connected_ports_tree_id().get_uuid(), &packets, user_mask).chain_err(|| ErrorKind::MessageError)?;
		Ok(())
	}
}
impl fmt::Display for DiscoverMsg { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("{}: {}", self.header, self.payload);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverPayload {
	tree_name: String,
	index: TableIndex,
	sending_cell_id: CellID,
	hops: PathLength,
	path: Path,
	gvm_eqn: GvmEquation,
}
impl DiscoverPayload {
	fn new(tree_name: String, index: TableIndex, sending_cell_id: &CellID, 
			hops: PathLength, path: Path) -> DiscoverPayload {
		let gvm_eqn = GvmEquation::new("true", "true", "true", "true", Vec::new());
		DiscoverPayload { tree_name: tree_name, index: index, sending_cell_id: sending_cell_id.clone(), 
			hops: hops, path: path, gvm_eqn: gvm_eqn }
	}
	//fn get_sending_cell(&self) -> CellID { self.sending_cell_id.clone() }
	fn get_hops(&self) -> PathLength { self.hops }
	fn hops_plus_one(&self) -> PathLength { PathLength(CellNo(**self.hops + 1)) }
	fn get_path(&self) -> Path { self.path }
	fn get_index(&self) -> TableIndex { self.index }
	fn set_hops(&mut self, hops: PathLength) { self.hops = hops; }
	fn set_path(&mut self, path: Path) { self.path = path; }
	fn set_index(&mut self, index: TableIndex) { self.index = index; }
	fn set_sending_cell(&mut self, sending_cell_id: CellID) { self.sending_cell_id = sending_cell_id; }
}
impl MsgPayload for DiscoverPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { Some(&self.gvm_eqn) }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for DiscoverPayload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Tree {}, sending cell {}, index {}, hops {}, path {}", self.tree_name, 
			self.sending_cell_id, *self.index, **self.hops, self.path);
		write!(f, "{}", s) 
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDMsg {
	header: MsgHeader,
	payload: DiscoverDPayload
}
impl DiscoverDMsg {
	pub fn new(tree_id: TreeID, index: TableIndex) -> DiscoverDMsg {
		// Note that direction is leafward so we can use the connected ports tree
		// If we send rootward, then the first recipient forwards the DiscoverD
		let tree_name = tree_id.stringify();
		let mut tree_map = HashMap::new();
		tree_map.insert(tree_name, tree_id.clone());
		let header = MsgHeader::new(MsgType::DiscoverD, MsgDirection::Leafward, tree_map);
		let payload = DiscoverDPayload::new(tree_id, index);
		DiscoverDMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverDMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		let tree_name = self.payload.get_tree_name();
		let tree_id = self.get_tree_id(tree_name)?;
		let my_index = self.payload.get_table_index();
		let mut children = HashSet::new();
		let port_number = PortNumber::new(port_no, MAX_PORTS).chain_err(|| ErrorKind::MessageError)?;
		children.insert(port_number);
		//println!("DiscoverDMsg {}: process msg {} processing {} {} {}", ca.get_id(), self.get_header().get_count(), port_no, my_index, tree_id);
		let gvm_eqn = GvmEquation::new("true", "true", "false", "false", Vec::new());
		ca.update_traph(&tree_id, port_number, traph::PortStatus::Child, Some(gvm_eqn), 
			&mut children, my_index, PathLength(CellNo(0)), None).chain_err(|| ErrorKind::MessageError)?;
		Ok(())
	}
}
impl fmt::Display for DiscoverDMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {
	tree_name: String,
	my_index: TableIndex,
}
impl DiscoverDPayload {
	fn new(tree_id: TreeID, index: TableIndex) -> DiscoverDPayload {
		DiscoverDPayload { tree_name: tree_id.stringify(), my_index: index }
	}
	pub fn get_table_index(&self) -> TableIndex { self.my_index }
}
impl MsgPayload for DiscoverDPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for DiscoverDPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "My table index {} for tree {}", *self.my_index, self.tree_name)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeMsg {
	header: MsgHeader,
	payload: StackTreeMsgPayload
}
impl StackTreeMsg {
	pub fn new(tree_id: &TreeID, black_tree_id: &TreeID) -> Result<StackTreeMsg> {
		let mut tree_map = HashMap::new();
		tree_map.insert(tree_id.stringify(), tree_id.clone());
		tree_map.insert(black_tree_id.stringify(), black_tree_id.clone()); 
		let header = MsgHeader::new(MsgType::StackTree, MsgDirection::Leafward, tree_map);
		let payload = StackTreeMsgPayload::new(tree_id, black_tree_id.stringify()).chain_err(|| ErrorKind::MessageError)?;
		Ok(StackTreeMsg { header: header, payload: payload})
	}
	fn build_gvm_params(&self, ca: &CellAgent, tree_id: &TreeID, gvm_eqn: GvmEquation) 
			-> Result<HashMap<GvmVariable,String>> {
		let params = HashMap::new();
		let variables = gvm_eqn.get_variables();
		for variable in variables.iter() {
			match variable.get_value().as_ref() {
				"hops" => {
					let hops = ca.get_hops(&tree_id)?;
				},
				_ => ()
			}
		}
		Ok(params)
	}
}
impl Message for StackTreeMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		//println!("Stack tree msg {}", self);
		let tree_map = self.header.get_tree_map();
		let tree_name = self.payload.get_tree_name();
		let gvm_eqn = self.payload.get_gvm_eqn();
		let black_tree_name = self.payload.get_black_tree_name();
		if let Some(black_tree_id) = tree_map.get(black_tree_name) {
			if let Some(tree_id) = tree_map.get(tree_name) {
				ca.stack_tree(&tree_id, &tree_uuid, black_tree_id, gvm_eqn).chain_err(|| ErrorKind::MessageError)?;
			} else {
				return Err(ErrorKind::TreeMapEntry(tree_name.to_string(), "process stack tree (black)".to_string()).into());
			}			
		} else {
			return Err(ErrorKind::TreeMapEntry(black_tree_name.to_string(), "process stack tree".to_string()).into());
		}
		Ok(())
	}
}
impl fmt::Display for StackTreeMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}	
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct StackTreeMsgPayload {
	tree_name: String,
	black_tree_name: String,
	gvm_eqn: GvmEquation,
}
impl StackTreeMsgPayload {
	fn new(tree_id: &TreeID, base_tree_name: String) -> Result<StackTreeMsgPayload> {
		let mut gvm_vars = Vec::new();
		gvm_vars.push(GvmVariable::new(GvmVariableType::PathLength, "hops"));
		let gvm_eqn = GvmEquation::new("hops == 0", "true", "true", "true", gvm_vars);
		Ok(StackTreeMsgPayload { tree_name: tree_id.stringify(), black_tree_name: base_tree_name, 
				gvm_eqn: gvm_eqn })
	}
	pub fn get_black_tree_name(&self) -> &String { &self.black_tree_name }
	pub fn get_tree_name(&self) -> &String { &self.tree_name}
	pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
}
impl MsgPayload for StackTreeMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { Some(&self.gvm_eqn) }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for StackTreeMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Tree {} stacked on black tree {}", self.tree_name, self.black_tree_name)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupVMsMsg {
	header: MsgHeader,
	payload: SetupVMsMsgPayload
}
impl SetupVMsMsg {
	pub fn new(id: &str, service_sets: Vec<Vec<Service>>) -> Result<SetupVMsMsg> {
		// Note that direction is rootward so cell agent will get the message
		let header = MsgHeader::new(MsgType::SetupVM, MsgDirection::Rootward, HashMap::new());
		let payload = SetupVMsMsgPayload::new(id, service_sets).chain_err(|| ErrorKind::MessageError)?;
		Ok(SetupVMsMsg { header: header, payload: payload })
	}
}
impl Message for SetupVMsMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		let service_sets = self.payload.get_service_sets().clone();
		ca.create_vms(service_sets).chain_err(|| ErrorKind::MessageError)?;		
		Ok(())
	}
}
impl fmt::Display for SetupVMsMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct SetupVMsMsgPayload {
	tree_name: String, 
	up_tree_id: UpTraphID,
	// Each set of services runs on a single VM
	// All the VMs setup in a single message are on an up-tree
	service_sets: Vec<Vec<Service>>,
}
impl SetupVMsMsgPayload {
	fn new(id: &str, service_sets: Vec<Vec<Service>>) -> Result<SetupVMsMsgPayload> {
		let up_tree_id = UpTraphID::new(id).chain_err(|| ErrorKind::MessageError)?;
		Ok(SetupVMsMsgPayload { tree_name: id.to_string(), up_tree_id: up_tree_id, service_sets: service_sets })
	}
	pub fn get_service_sets(&self) -> &Vec<Vec<Service>> { &self.service_sets }
}
impl MsgPayload for SetupVMsMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for SetupVMsMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Setup virtual machine with containers");
		for services in &self.service_sets {
			for container in services.iter() {
				s = s + &format!("{}", container);
			}
		}
		write!(f, "{}", s)
	}
}
// Errors
use errors::*;
error_chain! {
	links {
		CellAgent(::cellagent::Error, ::cellagent::ErrorKind);
		Packetizer(::packet::Error, ::packet::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { MessageError
		// Recursive type error if left in
//		Message(cell_id: CellID, msg_no: usize) {
//			description("Error processing message")
//			display("Error processing message {} on cell {}", msg_no, cell_id)
//		}
		TreeMapMissing(func_name: String) {
			display("{}: No tree map", func_name)
		}
		TreeMapEntry(tree_name: String, func_name: String) {
			display("{}: No tree named {} in map", func_name, tree_name)
		}		
	}
}
