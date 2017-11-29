use std;
use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};

use serde;
use serde_json;
use uuid::Uuid;

use cellagent::{CellAgent};
use config::{MAX_PORTS, CellNo, MsgID, PathLength, PortNo, TableIndex};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use nalcell::CellConfig;
use name::{Name, CellID, TreeID};
use noc::Noc;
use packet::{Packet, Packetizer, Serializer};
use service::Service;
use traph;
use uptree_spec::{AllowedTree, Manifest};
use utility::{DEFAULT_USER_MASK, S, Mask, Path, PortNumber};
use vm::VirtualMachine;

static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> MsgID { MsgID(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) as u64) } 
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgType {
	Discover,
	DiscoverD,
	Manifest,
	StackTree,
	TreeName,
}
impl MsgType {
	pub fn get_msg(packets: &Vec<Packet>) -> Result<Box<Message>, Error> {
		let serialized = Packetizer::unpacketize(packets)?;
		let type_msg = serde_json::from_str::<TypePlusMsg>(&serialized)?;
		let msg_type = type_msg.get_type();
		let serialized_msg = type_msg.get_serialized_msg();		
		Ok(match msg_type {
			MsgType::Discover  => Box::new(serde_json::from_str::<DiscoverMsg>(&serialized_msg)?),
			MsgType::DiscoverD => Box::new(serde_json::from_str::<DiscoverDMsg>(&serialized_msg)?),
			MsgType::Manifest  => Box::new(serde_json::from_str::<ManifestMsg>(&serialized_msg)?),
			MsgType::StackTree => Box::new(serde_json::from_str::<StackTreeMsg>(&serialized_msg)?),
			MsgType::TreeName  => Box::new(serde_json::from_str::<TreeNameMsg>(&serialized_msg)?),
		})		
	}
	// A hack for printing debug output only for a specific message type
	pub fn is_type(packet: Packet, type_of_msg: &str) -> bool {
		match format!("{}", packet).find(type_of_msg) {
			Some(_) => true,
			None => false
		}		
	}
}
impl fmt::Display for MsgType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgType::Discover  => write!(f, "Discover"),
			MsgType::DiscoverD => write!(f, "DiscoverD"),
			MsgType::Manifest  => write!(f, "Manifest"),
			MsgType::StackTree => write!(f, "StackTree"),
			MsgType::TreeName  => write!(f, "TreeName"),
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
impl fmt::Display for TypePlusMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}: {}", self.msg_type, self.serialized_msg)
	}
}

pub trait Message: fmt::Display {
	fn get_header(&self) -> &MsgHeader;
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn get_count(&self) -> MsgID { self.get_header().get_count() }
	fn get_tree_id(&self, tree_name: &String) -> Result<&TreeID, MessageError> {
		let tree_map = self.get_header().get_tree_map();
		Ok(match tree_map.get(tree_name) {
			Some(id) => id,
			None => return Err(MessageError::TreeMapEntry { tree_name: tree_name.clone(), func_name: "get_tree_id" })
		})
	}
	fn to_packets(&self, tree_id: &TreeID) -> Result<Vec<Packet>, Error>
			where Self:std::marker::Sized + serde::Serialize {
		let bytes = Serializer::serialize(self)?;
		let direction = self.get_header().get_direction();
		let packets = Packetizer::packetize(tree_id, &bytes, direction,);
		Ok(packets)
	}
	// There has to be a better way to handle different message receivers, but I didn't realize when I 
	// built the code that I was assuming that only the cell agent would be handling messages.  I tried 
	// a couple of restructurings of the code that didn't work, so I'm going with this simple kludge so 
	// I can get on with things.
	fn process_ca(&mut self, cell_agent: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		Err(MessageError::Process { func_name: "process_ca" }.into())
	}
	fn process_noc(&self, noc: &Noc) -> Result<&Vec<AllowedTree>, Error> {
		Err(MessageError::Process { func_name: "process_noc" }.into())
	}
	fn process_vm(&mut self, vm: &mut VirtualMachine) -> Result<(), Error> {
		Err(MessageError::Process { func_name: "process_vm" }.into())
	}
	fn process_service(&mut self, service: &mut Service) -> Result<(), Error> {
		Err(MessageError::Process { func_name: "process_service" }.into())
	}
	fn get_payload(&self) -> &MsgPayload;
	fn get_payload_discover(&self) -> Result<&DiscoverPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_discover" }.into()) }
	fn get_payload_discoverd(&self) -> Result<&DiscoverDPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_discoverd" }.into()) }
	fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_stack_tree" }.into()) }
	fn get_payload_manifest(&self) -> Result<&ManifestMsgPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_manifest" }.into()) }
	fn get_payload_tree_names(&self) -> Result<&TreeNameMsgPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_tree_names" }.into()) }
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
	pub fn update_discover_msg(&mut self, cell_id: &CellID, index: TableIndex) {
		let hops = self.update_hops();
		let path = self.update_path();
		self.payload.set_hops(hops);
		self.payload.set_path(path);
		self.payload.set_index(index);
		self.payload.set_sending_cell(cell_id.clone());
	}
	fn update_hops(&self) -> PathLength { self.payload.hops_plus_one() }
	fn update_path(&self) -> Path { self.payload.get_path() } // No change per hop
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn get_payload_discover(&self) -> Result<&DiscoverPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let port_number = PortNumber::new(port_no, ca.get_no_ports())?;
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
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("true"));
			eqns.insert(GvmEqn::Xtnd("true"));
			eqns.insert(GvmEqn::Save("false"));
			let gvm_equation = GvmEquation::new(eqns, Vec::new());
			let entry = match ca.update_traph(new_tree_id, port_number, status, Some(&gvm_equation),
					children, senders_index, hops, Some(path)) {
				Ok(e) => e,
				Err(err) => return Err(MessageError::Message { func_name: "process_ca", handler: "discover entry" }.into())
			};
			if exists { 
				return Ok(()); // Don't forward if traph exists for this tree - Simple quenching
			}
			my_index = entry.get_index();
			// Send DiscoverD to sender
			let discoverd_msg = DiscoverDMsg::new(new_tree_id.clone(), my_index);
			let packets = discoverd_msg.to_packets(new_tree_id)?;
			//println!("DiscoverMsg {}: sending discoverd for tree {} packet {} {}",ca.get_id(), new_tree_id, packets[0].get_count(), discoverd_msg);
			let mask = Mask::new(port_number);
			match ca.send_msg(ca.get_connected_ports_tree_id().get_uuid(), &packets, mask) {
				Ok(_) => (),
				Err(err) => return Err(MessageError::Message { func_name: "process_ca", handler: "discover send connected ports" }.into())
			};
			// Forward Discover on all except port_no with updated hops and path
		}
		self.update_discover_msg(&ca.get_id(), my_index);
		let packets = self.to_packets(&ca.get_control_tree_id())?;
		let user_mask = DEFAULT_USER_MASK.all_but_port(PortNumber::new(port_no, ca.get_no_ports())?);
		//println!("DiscoverMsg {}: forwarding packet {} on connected ports {}", ca.get_id(), packets[0].get_count(), self);
		ca.add_saved_discover(&packets); // Discover message are always saved for late port connect
		match ca.send_msg(ca.get_connected_ports_tree_id().get_uuid(), &packets, user_mask){
			Ok(_) => (),
			Err(err) => return Err(MessageError::Message { func_name: "process_ca", handler: "discover send msg" }.into())
		}
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
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_eqn = GvmEquation::new(eqns, Vec::new());
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
	fn get_payload_discoverd(&self) -> Result<&DiscoverDPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let tree_name = self.payload.get_tree_name();
		let tree_id = self.get_tree_id(tree_name)?;
		let my_index = self.payload.get_table_index();
		let mut children = HashSet::new();
		let port_number = PortNumber::new(port_no, MAX_PORTS)?;
		children.insert(port_number);
		//println!("DiscoverDMsg {}: process msg {} processing {} {} {}", ca.get_id(), self.get_header().get_count(), port_no, my_index, tree_id);
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("false"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_eqn = GvmEquation::new(eqns, Vec::new());
		match ca.update_traph(tree_id, port_number, traph::PortStatus::Child, Some(&gvm_eqn), 
			&mut children, my_index, PathLength(CellNo(0)), None) {
			Ok(_) => (),				
			Err(err) => return Err( MessageError::Message { func_name: "process_ca", handler: "discoverd update traph" }.into())
		};
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
	pub fn new(new_tree_id: &TreeID, base_tree_id: &TreeID, manifest: &Manifest) -> Result<StackTreeMsg, MessageError> {
		let mut tree_map = HashMap::new();
		tree_map.insert(new_tree_id.stringify(), new_tree_id.clone());
		tree_map.insert(base_tree_id.stringify(), base_tree_id.clone()); 
		let header = MsgHeader::new(MsgType::StackTree, MsgDirection::Leafward, tree_map);
		let payload = StackTreeMsgPayload::new(new_tree_id, base_tree_id.stringify(), manifest)?;
		Ok(StackTreeMsg { header: header, payload: payload})
	}
}
impl Message for StackTreeMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		//println!("Cell {}: Stack tree msg {}", ca.get_id(), self);
		let tree_map = self.header.get_tree_map();
		let tree_name = self.payload.get_tree_name();
		if let Some(gvm_eqn) = self.payload.get_gvm_eqn() {
			let black_tree_name = self.payload.get_black_tree_name();
			if let Some(black_tree_id) = tree_map.get(black_tree_name) {
				if let Some(tree_id) = tree_map.get(tree_name) {
					let manifest = Manifest::new(black_tree_name, CellConfig::Large, &tree_name, Vec::new(), 
						Vec::new(), Vec::new(), &gvm_eqn)?;
					match ca.stack_tree(&tree_id, &msg_tree_id, black_tree_id, &manifest) {
						Ok(_) => (),
						Err(err) => return Err(MessageError::Message { func_name: "process_ca", handler: "stack tree" }.into())
					}
				} else {
					return Err(MessageError::TreeMapEntry { tree_name: tree_name.to_string(), func_name: "process stack tree (black)" }.into());
				}			
			} else {
				return Err(MessageError::TreeMapEntry { tree_name: S(black_tree_name), func_name: "process stack tree" }.into());
			}
		} else {
			return Err(MessageError::ManifestGvm { func_name: "process" }.into());
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
	manifest: Manifest,
}
impl StackTreeMsgPayload {
	fn new(tree_id: &TreeID, base_tree_name: String, manifest: &Manifest) -> Result<StackTreeMsgPayload, MessageError> {
		Ok(StackTreeMsgPayload { tree_name: tree_id.stringify(), black_tree_name: base_tree_name, 
				manifest: manifest.clone() })
	}
	pub fn get_black_tree_name(&self) -> &String { &self.black_tree_name }
	pub fn get_tree_name(&self) -> &String { &self.tree_name}
	pub fn get_manifest(&self) -> &Manifest { &self.manifest }
}
impl MsgPayload for StackTreeMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { Some(&self.manifest.get_gvm()) }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for StackTreeMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Tree {} stacked on black tree {}", self.tree_name, self.black_tree_name)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMsg {
	header: MsgHeader,
	payload: ManifestMsgPayload
}
impl ManifestMsg {
	pub fn new(manifest: &Manifest) -> ManifestMsg {
		// Note that direction is rootward so cell agent will get the message
		let header = MsgHeader::new(MsgType::Manifest, MsgDirection::Rootward, HashMap::new());
		let payload = ManifestMsgPayload::new(&manifest);
		ManifestMsg { header: header, payload: payload }
	}
}
impl Message for ManifestMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn get_payload_manifest(&self) -> Result<&ManifestMsgPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let manifest = self.payload.get_manifest();
		match ca.deploy(port_no, &manifest) {
			Ok(_) => (),
			Err(err) => {
				println!("--- Problem processing ManifestMsg");
				return Err(MessageError::Message { func_name: "process_ca", handler: "manifest" }.into())
			}
		}
		Ok(())		
	}
}
impl fmt::Display for ManifestMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ManifestMsgPayload {
	tree_name: String,
	manifest: Manifest 
}
impl ManifestMsgPayload {
	fn new(manifest: &Manifest) -> ManifestMsgPayload {
		let tree_name = manifest.get_new_tree_name();
		ManifestMsgPayload { tree_name: tree_name.clone(), manifest: manifest.clone() }
	}
	fn get_manifest(&self) -> &Manifest { &self.manifest }
}
impl MsgPayload for ManifestMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for ManifestMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Manifest: {}", self.get_manifest());
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNameMsg {
	header: MsgHeader,
	payload: TreeNameMsgPayload
}
impl TreeNameMsg {
	pub fn new(id: &TreeID, allowed_trees: &Vec<AllowedTree>) -> TreeNameMsg {
		// Note that direction is rootward so cell agent will get the message
		let header = MsgHeader::new(MsgType::TreeName, MsgDirection::Rootward, HashMap::new());
		let payload = TreeNameMsgPayload::new(id, allowed_trees);
		TreeNameMsg { header: header, payload: payload }
	}
}
impl Message for TreeNameMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn get_payload_tree_names(&self) -> Result<&TreeNameMsgPayload, Error> { Ok(&self.payload) }
	fn process_noc(&self, noc: &Noc) -> Result<&Vec<AllowedTree>, Error> {
		let allowed_trees = self.get_payload_tree_names()?.get_allowed_trees();
		Ok(allowed_trees)		
	}
}
impl fmt::Display for TreeNameMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TreeNameMsgPayload {
	tree_name: String,
	allowed_trees: Vec<AllowedTree>
}
impl TreeNameMsgPayload {
	fn new(tree_id: &TreeID, allowed_trees: &Vec<AllowedTree>) -> TreeNameMsgPayload {
		TreeNameMsgPayload { tree_name: S(tree_id.get_name()), allowed_trees: allowed_trees.clone() }
	}
	fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
}
impl MsgPayload for TreeNameMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for TreeNameMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Tree name for border cell {}", self.tree_name);
		write!(f, "{}", s)
	}
}
// Errors
use failure::{Error, Fail};
#[derive(Debug, Fail)]
pub enum MessageError {
    #[fail(display = "Message {}: Invalid message type {} from packet assembler", func_name, msg_type)]
    InvalidMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "Message {}: No GVM in manifest", func_name)]
    ManifestGvm { func_name: &'static str },
    #[fail(display = "Message {}: Message error from {}", func_name, handler)]
    Message { func_name: &'static str, handler: &'static str },
    #[fail(display = "Message {}: Wrong payload for this message type", func_name)]
    Payload { func_name: &'static str },
    #[fail(display = "Message {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
    #[fail(display = "Message {}: No tree named {} in map", func_name, tree_name)]
    TreeMapEntry { func_name: &'static str, tree_name: String }
}
/*
fn map_cellagent_errors(err: ::cellagent::Error) -> ::message::Error {
	::message::ErrorKind::CellAgent(Box::new(err)).into()
}
error_chain! {
	foreign_links {
		Serialize(::serde_json::Error);
	}
	links {
		Manifest(::uptree_spec::Error, ::uptree_spec::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		Packet(::packet::Error, ::packet::ErrorKind);
		Utility(::utility::Error, ::utility::ErrorKind);
	}
	errors { 
		CellAgent(err: Box<::cellagent::Error>)
		InvalidMsgType(func_name: String, msg_type: MsgType) {
			display("Message {}: Invalid message type {} from packet assembler", func_name, msg_type)
		}
		ManifestGvm(func_name: String) {
			display("Message {}: No GVM in manifest", func_name)
		}
		Payload(func_name: String) {
			display("Message {}: Wrong payload for this message type", func_name)
		}
		Process(func_name: String) {
			display("Message {}: Wrong message process function called", func_name)
		}
		TreeMapEntry(tree_name: String, func_name: String) {
			display("Message {}: No tree named {} in map", func_name, tree_name)
		}		
	}
}
*/