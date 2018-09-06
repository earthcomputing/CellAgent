use std;
use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};

use serde;
use serde_json;

use cellagent::{CellAgent};
use config::{ByteArray, CellNo, MsgID, PathLength, PortNo};
use gvm_equation::{GvmEquation, GvmEqn};
use name::{Name, CellID, SenderID, TreeID};
use packet::{Packet, Packetizer, Serializer};
use uptree_spec::{AllowedTree, Manifest};
use utility::{S, Path, TraceHeader};

static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> MsgID { MsgID(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) as u64) }

pub type MsgTreeMap = HashMap<String, TreeID>;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum MsgType {
    Application,
	Discover,
	DiscoverD,
    Failover,
	Manifest,
	StackTree,
    StackTreeD,
	TreeName,
}
impl MsgType {
    // Used for debug hack in packet_engine
    pub fn get_msg(packets: &Vec<Packet>) -> Result<Box<dyn Message>, Error> {
        let _f = "get_msg";
        let bytes = Packetizer::unpacketize(packets).context(MessageError::Chain { func_name: _f, comment: S("unpacketize")})?;
        //println!("Message get_msg: serialized {}, packets {:?}", serialized, packets);
        let serialized = ::std::str::from_utf8(&bytes)?;
		let type_msg = serde_json::from_str::<TypePlusMsg>(&serialized).context(MessageError::Chain { func_name: "get_msg", comment: S("deserialize MsgType")})?;
		let msg_type = type_msg.get_type();
		let serialized_msg = type_msg.get_serialized_msg();
		Ok(match msg_type {
            MsgType::Application => Box::new(serde_json::from_str::<ApplicationMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverMsg")})?),
			MsgType::Discover    => Box::new(serde_json::from_str::<DiscoverMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverMsg")})?),
            MsgType::DiscoverD   => Box::new(serde_json::from_str::<DiscoverDMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverDMsg")})?),
            MsgType::Failover    => Box::new(serde_json::from_str::<FailoverMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("FailoverMsg")})?),
			MsgType::Manifest    => Box::new(serde_json::from_str::<ManifestMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("ManifestMsg")})?),
            MsgType::StackTree   => Box::new(serde_json::from_str::<StackTreeMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("StackTreeMsg")})?),
            MsgType::StackTreeD  => Box::new(serde_json::from_str::<StackTreeDMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("StackTreeDMsg")})?),
			MsgType::TreeName    => Box::new(serde_json::from_str::<TreeNameMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("TreeNameMsg")})?),
		})		
	}
    pub fn msg_from_bytes(bytes: &ByteArray) -> Result<Box<dyn Message>, Error> {
        let _f = "msg_from_bytes";
        let serialized = ::std::str::from_utf8(bytes)?;
        let type_msg = serde_json::from_str::<TypePlusMsg>(serialized).context(MessageError::Chain { func_name: _f, comment: S("deserialize MsgType")})?;
        let msg_type = type_msg.get_type();
        let serialized_msg = type_msg.get_serialized_msg();
        Ok(match msg_type {
            MsgType::Application => Box::new(serde_json::from_str::<ApplicationMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverMsg")})?),
            MsgType::Discover    => Box::new(serde_json::from_str::<DiscoverMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverMsg")})?),
            MsgType::DiscoverD   => Box::new(serde_json::from_str::<DiscoverDMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("DiscoverDMsg")})?),
            MsgType::Failover    => Box::new(serde_json::from_str::<FailoverMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("FailoverMsg")})?),
            MsgType::Manifest    => Box::new(serde_json::from_str::<ManifestMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("ManifestMsg")})?),
            MsgType::StackTree   => Box::new(serde_json::from_str::<StackTreeMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("StackTreeMsg")})?),
            MsgType::StackTreeD  => Box::new(serde_json::from_str::<StackTreeDMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("StackTreeDMsg")})?),
            MsgType::TreeName    => Box::new(serde_json::from_str::<TreeNameMsg>(&serialized_msg).context(MessageError::Chain { func_name: _f, comment: S("TreeNameMsg")})?),
        })
    }
	// A hack for printing debug output only for a specific message type
	pub fn is_type(packet: &Packet, type_of_msg: MsgType) -> bool {
		match format!("{}", packet).find(&(S(type_of_msg) + "\\")) {
			Some(_) => true,
			None => false
		}		
	}
	// A hack for finding the message type
    pub fn msg_type(packet: &Packet) -> MsgType {
        if      MsgType::is_type(packet, MsgType::Application) { MsgType::Application }
        else if MsgType::is_type(packet, MsgType::Discover)    { MsgType::Discover }
        else if MsgType::is_type(packet, MsgType::DiscoverD)   { MsgType::DiscoverD }
        else if MsgType::is_type(packet, MsgType::Failover)    { MsgType::Failover }
        else if MsgType::is_type(packet, MsgType::Manifest)    { MsgType::Manifest }
        else if MsgType::is_type(packet, MsgType::StackTree)   { MsgType::StackTree }
        else if MsgType::is_type(packet, MsgType::StackTreeD)  { MsgType::StackTreeD }
        else if MsgType::is_type(packet, MsgType::TreeName)    { MsgType::TreeName }
        else { panic!("Invalid message type") }
    }
}
impl fmt::Display for MsgType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
            MsgType::Application => write!(f, "Application"),
			MsgType::Discover    => write!(f, "Discover"),
            MsgType::DiscoverD   => write!(f, "DiscoverD"),
            MsgType::Failover    => write!(f, "Failover"),
			MsgType::Manifest    => write!(f, "Manifest"),
			MsgType::StackTree   => write!(f, "StackTree"),
            MsgType::StackTreeD  => write!(f, "StackTreeD"),
			MsgType::TreeName    => write!(f, "TreeName"),
		}
	}
}
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TcpMsgType {
    Application,
    DeleteTree,
    Manifest,
    Query,
    StackTree,
    TreeName,
}
impl fmt::Display for TcpMsgType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TcpMsgType::Application  => write!(f, "Application"),
            TcpMsgType::DeleteTree   => write!(f, "DeleteTree"),
            TcpMsgType::Manifest     => write!(f, "Manifest"),
            TcpMsgType::Query        => write!(f, "Query"),
            TcpMsgType::StackTree    => write!(f, "StackTree"),
            TcpMsgType::TreeName     => write!(f, "TreeName"),
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
		TypePlusMsg { msg_type, serialized_msg }
	}
	fn get_type(&self) -> MsgType { self.msg_type }
	fn get_serialized_msg(&self) -> &str { &self.serialized_msg }
}
impl fmt::Display for TypePlusMsg {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}: {}", self.msg_type, self.serialized_msg)
	}
}
pub trait Message {
	fn get_header(&self) -> &MsgHeader;
    fn get_payload(&self) -> &MsgPayload;
    fn get_msg_type(&self) -> MsgType;
    fn get_tree_id(&self) -> Option<&TreeID> { None }
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
    fn is_ait(&self) -> bool { self.get_header().get_ait() }
    fn is_blocking(&self) -> bool;
    fn value(&self) -> serde_json::Value;
//	fn get_count(&self) -> MsgID { self.get_header().get_count() }
    fn to_bytes(&self) -> Result<ByteArray, Error> where Self: serde::Serialize + Sized {
        let _f = "to_bytes";
        let bytes = Serializer::serialize(self).context(MessageError::Chain { func_name: _f, comment: S("")})?;
        Ok(ByteArray(*bytes))
    }
    fn to_packets(&self, tree_id: &TreeID) -> Result<Vec<Packet>, Error>
			where Self:std::marker::Sized + serde::Serialize {
        let _f = "to_packets";
		let bytes = Serializer::serialize(self).context(MessageError::Chain { func_name: _f, comment: S("")})?;
        let packets = Packetizer::packetize(&tree_id.get_uuid(), &ByteArray(*bytes), self.is_blocking());
		Ok(packets)
	}
	fn process_ca(&mut self, _cell_agent: &mut CellAgent, _port_no: PortNo, _msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        let _f = "process_ca";
        Err(MessageError::Process { func_name: _f }.into()) }
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{} {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
pub trait MsgPayload: fmt::Display {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgHeader {
	msg_count: MsgID, // Debugging only?
    is_ait: bool,
    sender_id: SenderID, // Used to find set of AllowedTrees
	msg_type: MsgType,
	direction: MsgDirection,
	tree_map: MsgTreeMap,
}
impl MsgHeader {
	pub fn new(sender_id: &SenderID, is_ait: bool, msg_type: MsgType, direction: MsgDirection) -> MsgHeader {
		let msg_count = get_next_count();
		MsgHeader { sender_id: sender_id.clone(), is_ait, msg_type, direction, msg_count, tree_map: HashMap::new() }
	}
	pub fn get_msg_type(&self) -> MsgType { self.msg_type }
//	pub fn get_count(&self) -> MsgID { self.msg_count }
    pub fn get_ait(&self) -> bool { self.is_ait }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
    pub fn get_sender_id(&self) -> &SenderID { &self.sender_id }
    pub fn get_tree_map(&self) -> &MsgTreeMap { &self.tree_map }
    pub fn set_tree_map(&mut self, tree_map: MsgTreeMap) { self.tree_map = tree_map; } // Should this be set in new()?
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
	pub fn new(sender_id: &SenderID, tree_id: &TreeID, sending_cell_id: &CellID,
			hops: PathLength, path: Path) -> DiscoverMsg {
		let header = MsgHeader::new(sender_id, true,MsgType::Discover, MsgDirection::Leafward);
		//println!("DiscoverMsg: msg_count {}", header.get_count());
		let payload = DiscoverPayload::new(tree_id, &sending_cell_id, hops, path);
		DiscoverMsg { header, payload }
	}
	pub fn update(&self, cell_id: &CellID) -> DiscoverMsg {
        let mut msg = self.clone();
		let hops = self.update_hops();
		let path = self.update_path();
		msg.payload.set_hops(hops);
		msg.payload.set_path(path);
		msg.payload.set_sending_cell(cell_id.clone());
        msg
	}
	pub fn update_hops(&self) -> PathLength { self.payload.hops_plus_one() }
	pub fn update_path(&self) -> Path { self.payload.get_path() } // No change per hop
    pub fn get_payload(&self) -> &DiscoverPayload { &self.payload }
}
impl Message for DiscoverMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.tree_id) }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value where Self: serde::Serialize {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
	fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        cell_agent.process_discover_msg(self, port_no, trace_header)
	}
}
impl fmt::Display for DiscoverMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverPayload {
	tree_id: TreeID,
	sending_cell_id: CellID,
	hops: PathLength,
	path: Path,
	gvm_eqn: GvmEquation,
}
impl DiscoverPayload {
	fn new(tree_id: &TreeID, sending_cell_id: &CellID, hops: PathLength, path: Path)
            -> DiscoverPayload {
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_eqn = GvmEquation::new(eqns, Vec::new());
		DiscoverPayload { tree_id: tree_id.clone(), sending_cell_id: sending_cell_id.clone(),
			hops, path, gvm_eqn }
	}
	//fn get_sending_cell(&self) -> CellID { self.sending_cell_id.clone() }
	pub fn get_hops(&self) -> PathLength { self.hops }
	pub fn hops_plus_one(&self) -> PathLength { PathLength(CellNo(**self.hops + 1)) }
    pub fn get_path(&self) -> Path { self.path }
    pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
    pub fn set_hops(&mut self, hops: PathLength) { self.hops = hops; }
    pub fn set_path(&mut self, path: Path) { self.path = path; }
    pub fn set_sending_cell(&mut self, sending_cell_id: CellID) { self.sending_cell_id = sending_cell_id; }
}
impl MsgPayload for DiscoverPayload {}
impl fmt::Display for DiscoverPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Tree {}, sending cell {}, hops {}, path {}", self.tree_id,
			self.sending_cell_id, **self.hops, self.path);
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDMsg {
	header: MsgHeader,
	payload: DiscoverDPayload
}
impl DiscoverDMsg {
	pub fn new(sender_id: &SenderID, tree_id: &TreeID, path: Path) -> DiscoverDMsg {
		// Note that direction is leafward so we can use the connected ports tree
		// If we send rootward, then the first recipient forwards the DiscoverD
		let header = MsgHeader::new(sender_id, true,MsgType::DiscoverD, MsgDirection::Leafward);
		let payload = DiscoverDPayload::new(tree_id, path);
		DiscoverDMsg { header, payload }
	}
    pub fn get_payload(&self) -> &DiscoverDPayload { &self.payload }
}
impl Message for DiscoverDMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_tree_id()) }
    fn is_blocking(&self) -> bool { true }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
	fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, _msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_discover_d_msg(&self, port_no, trace_header)
    }
}
impl fmt::Display for DiscoverDMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {
	tree_id: TreeID,
    path: Path
}
impl DiscoverDPayload {
	fn new(tree_id: &TreeID, path: Path) -> DiscoverDPayload {
		DiscoverDPayload { tree_id: tree_id.clone(), path }
	}
	pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
    pub fn get_path(&self) -> Path { self.path }
}
impl MsgPayload for DiscoverDPayload {}
impl fmt::Display for DiscoverDPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "My Tree {}", self.tree_id)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverMsg {
    header: MsgHeader,
    payload: FailoverMsgPayload
}
impl FailoverMsg {
    pub fn new(sender_id: &SenderID, rw_tree_id: &TreeID, path: Path) -> FailoverMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the FailoverMsg
        let header = MsgHeader::new(sender_id, true,MsgType::Failover, MsgDirection::Leafward);
        let payload = FailoverMsgPayload::new(rw_tree_id, path);
        FailoverMsg { header, payload }
    }
    pub fn get_payload(&self) -> &FailoverMsgPayload { &self.payload }
}
impl Message for FailoverMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_rootward_tree_id()) }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, _msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_failover_msg(&self, port_no, trace_header)
    }
}
impl fmt::Display for FailoverMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverMsgPayload {
    rw_tree_id: TreeID,
    path: Path
}
impl FailoverMsgPayload {
    fn new(rw_tree_id: &TreeID, path: Path) -> FailoverMsgPayload {
        FailoverMsgPayload { rw_tree_id: rw_tree_id.clone(), path }
    }
    pub fn get_rootward_tree_id(&self) -> &TreeID { &self.rw_tree_id }
    pub fn get_path(&self) -> Path { self.path }
}
impl MsgPayload for FailoverMsgPayload {}
impl fmt::Display for FailoverMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rootward Tree {}, root port {}", self.rw_tree_id, self.path)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeMsg {
	header: MsgHeader,
	payload: StackTreeMsgPayload
}
impl StackTreeMsg {
	pub fn new(sender_id: &SenderID, new_tree_name: &AllowedTree, new_tree_id: &TreeID, parent_tree_id: &TreeID,
               direction: MsgDirection, gvm_eqn: &GvmEquation) -> StackTreeMsg {
		let header = MsgHeader::new( sender_id, true,MsgType::StackTree, direction);
		let payload = StackTreeMsgPayload::new(new_tree_name, new_tree_id, parent_tree_id, gvm_eqn);
		StackTreeMsg { header, payload}
	}
    pub fn get_payload(&self) -> &StackTreeMsgPayload { &self.payload }
}
impl Message for StackTreeMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_new_tree_id()) }
    fn is_blocking(&self) -> bool { true }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
	fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        cell_agent.process_stack_tree_msg(&self, port_no, msg_tree_id, trace_header)
	}
}
impl fmt::Display for StackTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct StackTreeMsgPayload {
    allowed_tree: AllowedTree,
    new_tree_id: TreeID,
    parent_tree_id: TreeID,
    gvm_eqn: GvmEquation
}
impl StackTreeMsgPayload {
    fn new(allowed_tree: &AllowedTree, new_tree_id: &TreeID, parent_tree_id: &TreeID, gvm_eqn: &GvmEquation) -> StackTreeMsgPayload {
        StackTreeMsgPayload { allowed_tree: allowed_tree.clone(), new_tree_id: new_tree_id.clone(),
            parent_tree_id: parent_tree_id.clone(), gvm_eqn: gvm_eqn.clone() }
    }
    pub fn get_allowed_tree(&self) -> &AllowedTree { &self.allowed_tree }
    pub fn get_new_tree_id(&self) -> &TreeID { &self.new_tree_id }
    pub fn get_parent_tree_id(&self) -> &TreeID { &self.parent_tree_id }
    pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
}
impl MsgPayload for StackTreeMsgPayload {}
impl fmt::Display for StackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tree {} stacked on tree {} {}", self.new_tree_id, self.parent_tree_id, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeDMsg {
    header: MsgHeader,
    payload: StackTreeMsgDPayload
}
impl StackTreeDMsg {
    pub fn new(sender_id: &SenderID, tree_id: &TreeID) -> StackTreeDMsg {
        let header = MsgHeader::new(sender_id, true,MsgType::StackTreeD, MsgDirection::Leafward);
        let payload = StackTreeMsgDPayload::new(tree_id);
        StackTreeDMsg { header, payload}
    }
//    fn update_payload(&self, payload: StackTreeMsgDPayload) -> StackTreeDMsg {
//        StackTreeDMsg { header: self.header.clone(), payload }
//    }
    pub fn get_payload(&self) -> &StackTreeMsgDPayload { &self.payload }
//    pub fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_tree_id()) }
}
impl Message for StackTreeDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(self.payload.get_tree_id()) }
    fn is_blocking(&self) -> bool { true }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,_msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        cell_agent.process_stack_tree_d_msg(&self, port_no, trace_header)
    }
}
impl fmt::Display for StackTreeDMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct StackTreeMsgDPayload {
    tree_id: TreeID,
}
impl StackTreeMsgDPayload {
    fn new(tree_id: &TreeID) -> StackTreeMsgDPayload {
        StackTreeMsgDPayload { tree_id: tree_id.clone() }
    }
    pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
}
impl MsgPayload for StackTreeMsgDPayload {}
impl fmt::Display for StackTreeMsgDPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tree {}", self.tree_id)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMsg {
	header: MsgHeader,
	payload: ManifestMsgPayload
}
impl ManifestMsg {
	pub fn new(sender_id: &SenderID, is_ait: bool, deploy_tree_id: &TreeID, tree_map: &MsgTreeMap, manifest: &Manifest) -> ManifestMsg {
		// Note that direction is leafward so cell agent will get the message
		let mut header = MsgHeader::new(sender_id, is_ait, MsgType::Manifest, MsgDirection::Leafward);
        header.set_tree_map(tree_map.clone());
		let payload = ManifestMsgPayload::new(deploy_tree_id, &manifest);
		ManifestMsg { header, payload }
	}
    pub fn get_payload(&self) -> &ManifestMsgPayload { &self.payload }
}
impl Message for ManifestMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_deploy_tree_id()) }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
	fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        cell_agent.process_manifest_msg(&self, port_no, msg_tree_id, trace_header)
	}
}
impl fmt::Display for ManifestMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ManifestMsgPayload {
    deploy_tree_id: TreeID,
	tree_name: AllowedTree,
	manifest: Manifest 
}
impl ManifestMsgPayload {
	fn new(deploy_tree_id: &TreeID, manifest: &Manifest) -> ManifestMsgPayload {
        let tree_name = manifest.get_deployment_tree();
		ManifestMsgPayload { deploy_tree_id: deploy_tree_id.clone(), tree_name: tree_name.clone(),
            manifest: manifest.clone() }
	}
	pub fn get_manifest(&self) -> &Manifest { &self.manifest }
//	pub fn get_tree_name(&self) -> String { S(self.tree_name.get_name()) }
    pub fn get_deploy_tree_id(&self) -> &TreeID { &self.deploy_tree_id }
}
impl MsgPayload for ManifestMsgPayload {}
impl fmt::Display for ManifestMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Manifest: {}", self.get_manifest());
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMsg {
    header: MsgHeader,
    payload: ApplicationMsgPayload
}
impl ApplicationMsg {
    pub fn new(sender_id: &SenderID, is_ait: bool, tree_id: &TreeID, direction: MsgDirection, body: &str) -> ApplicationMsg {
        let header = MsgHeader::new(sender_id, is_ait,MsgType::Application, direction);
        let payload = ApplicationMsgPayload::new(tree_id, body);
        ApplicationMsg { header, payload }
    }
    pub fn get_payload(&self) -> &ApplicationMsgPayload { &self.payload }
    pub fn get_tree_id(&self) -> &TreeID { self.payload.get_tree_id() }
}
impl Message for ApplicationMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_tree_id(&self) -> Option<&TreeID> { Some(&self.payload.get_tree_id()) }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo, msg_tree_id: &TreeID, is_ait: bool,
                  trace_header: &mut TraceHeader) -> Result<(), Error> {
        cell_agent.process_application_msg(self, port_no,  msg_tree_id, is_ait, trace_header)
    }
}
impl fmt::Display for ApplicationMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ApplicationMsgPayload {
    tree_id: TreeID,
    body: ByteArray,
}
impl ApplicationMsgPayload {
    fn new(tree_id: &TreeID, body: &str) -> ApplicationMsgPayload {
        ApplicationMsgPayload { tree_id: tree_id.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
}
impl MsgPayload for ApplicationMsgPayload {}
impl fmt::Display for ApplicationMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(body) = ::std::str::from_utf8(&self.body) {
            let s = format!("Application message {}", body);
            write!(f, "{}", s)
        } else {
            write!(f, "Error converting application message body from bytes to string")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNameMsg {
    header: MsgHeader,
    payload: TreeNameMsgPayload
}
impl TreeNameMsg {
    pub fn new(sender_id: &SenderID, tree_name: &str) -> TreeNameMsg {
        // Note that direction is rootward so cell agent will get the message
        let header = MsgHeader::new(sender_id, false,MsgType::TreeName, MsgDirection::Rootward);
        let payload = TreeNameMsgPayload::new(tree_name);
        TreeNameMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TreeNameMsgPayload { &self.payload }
    pub fn get_tree_name(&self) -> &String { self.payload.get_tree_name() }
}
impl Message for TreeNameMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TreeNameMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TreeNameMsgPayload {
    tree_name: String,
}
impl TreeNameMsgPayload {
    fn new(tree_name: &str) -> TreeNameMsgPayload {
        TreeNameMsgPayload { tree_name: S(tree_name) }
    }
    fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl MsgPayload for TreeNameMsgPayload {}
impl fmt::Display for TreeNameMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("Tree name for border cell {}", self.tree_name);
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum MessageError {
	#[fail(display = "MessageError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
//    #[fail(display = "MessageError::Gvm {}: No GVM for this message type {}", func_name, msg_type)]
//    Gvm { func_name: &'static str, msg_type: MsgType },
//    #[fail(display = "MessageError::InvalidMsgType {}: Invalid message type {} from packet assembler", func_name, msg_type)]
//    InvalidMsgType { func_name: &'static str, msg_type: MsgType },
//    #[fail(display = "MessageError::Message {}: Message error from {}", func_name, handler)]
//    Message { func_name: &'static str, handler: &'static str },
//    #[fail(display = "MessageError::NoGmv {}: No GVM in StackTreeMsg", func_name)]
//    NoGvm { func_name: &'static str },
//    #[fail(display = "MessageError::Payload {}: Wrong payload for type {}", func_name, msg_type)]
//    Payload { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "MessageError::Process {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
//    #[fail(display = "MessageError::TreeID {}: No TreeID ", func_name)]
//    TreeID { func_name: &'static str },
//    #[fail(display = "MessageError::TreeMapEntry {}: No tree named {} in map", func_name, tree_name)]
//    TreeMapEntry { func_name: &'static str, tree_name: String }
}
