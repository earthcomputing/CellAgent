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
use packet::{Packet, Packetizer, Serializer};
use traph;
use uptree_spec::Manifest;
use utility::{DEFAULT_USER_MASK, S, Mask, Path, PortNumber};

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
	pub fn get_msg(packets: &Vec<Packet>) -> Result<Box<Message>> {
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
	fn to_packets(&self, tree_id: &TreeID) -> Result<Vec<Packet>> 
			where Self:std::marker::Sized + serde::Serialize {
		let bytes = Serializer::serialize(self)?;
		let direction = self.get_header().get_direction();
		let packets = Packetizer::packetize(tree_id, &bytes, direction,)?;		
		Ok(packets)
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
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
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
				Err(err) => return Err(map_cellagent_errors(err))
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
				Err(err) => return Err(map_cellagent_errors(err))
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
			Err(err) => return Err(map_cellagent_errors(err))
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
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
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
			Err(err) => return Err(map_cellagent_errors(err))
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
	pub fn new(new_tree_id: &TreeID, base_tree_id: &TreeID, manifest: &Manifest) -> Result<StackTreeMsg> {
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
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		//println!("Cell {}: Stack tree msg {}", ca.get_id(), self);
		let tree_map = self.header.get_tree_map();
		let tree_name = self.payload.get_tree_name();
		if let Some(gvm_eqn) = self.payload.get_gvm_eqn() {
			let black_tree_name = self.payload.get_black_tree_name();
			if let Some(black_tree_id) = tree_map.get(black_tree_name) {
				if let Some(tree_id) = tree_map.get(tree_name) {
					let manifest = Manifest::new(black_tree_name, CellConfig::Large, &tree_name, Vec::new(), 
						Vec::new(), Vec::new(), &gvm_eqn)?;
					match ca.stack_tree(&tree_id, &tree_uuid, black_tree_id, &manifest) {
						Ok(_) => (),
						Err(err) => return Err(map_cellagent_errors(err))
					}
				} else {
					return Err(ErrorKind::TreeMapEntry(tree_name.to_string(), S("process stack tree (black)")).into());
				}			
			} else {
				return Err(ErrorKind::TreeMapEntry(black_tree_name.to_string(), S("process stack tree")).into());
			}
		} else {
			return Err(ErrorKind::ManifestGvm(S("process")).into());
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
	fn new(tree_id: &TreeID, base_tree_name: String, manifest: &Manifest) -> Result<StackTreeMsgPayload> {
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
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		let manifest = self.payload.get_manifest();
		match ca.deploy(port_no, &manifest) {
			Ok(_) => (),
			Err(err) => {
				println!("--- Problem processing ManifestMsg");
				return Err(map_cellagent_errors(err))
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
		let mut s = format!("Manifest: {}", self.get_manifest());
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNameMsg {
	header: MsgHeader,
	payload: TreeNameMsgPayload
}
impl TreeNameMsg {
	pub fn new(id: &str) -> TreeNameMsg {
		// Note that direction is rootward so cell agent will get the message
		let header = MsgHeader::new(MsgType::TreeName, MsgDirection::Rootward, HashMap::new());
		let payload = TreeNameMsgPayload::new(id);
		TreeNameMsg { header: header, payload: payload }
	}
}
impl Message for TreeNameMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn process(&mut self, ca: &mut CellAgent, tree_uuid: Uuid, port_no: PortNo) -> Result<()> {
		// Never called, since message goes to NOC rather than CellAgent
		Ok(())		
	}
}
impl fmt::Display for TreeNameMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {}", self.header, self.payload)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TreeNameMsgPayload {
	tree_name: String 
}
impl TreeNameMsgPayload {
	fn new(id: &str) -> TreeNameMsgPayload {
		TreeNameMsgPayload { tree_name: id.to_string() }
	}
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl MsgPayload for TreeNameMsgPayload {
	fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
	fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl fmt::Display for TreeNameMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Tree name for border cell {}", self.tree_name);
		write!(f, "{}", s)
	}
}
// Errors
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
		TreeMapEntry(tree_name: String, func_name: String) {
			display("Message {}: No tree named {} in map", func_name, tree_name)
		}		
	}
}
