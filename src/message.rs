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
use service::{NOCAGENT, Service, NocAgent};
use traph;
use uptree_spec::{AllowedTree, Manifest};
use utility::{DEFAULT_USER_MASK, S, Mask, Path, PortNumber};
use vm::VirtualMachine;

static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> MsgID { MsgID(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) as u64) }

pub type MsgTreeMap = HashMap<AllowedTree, TreeID>;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum MsgType {
	Discover,
	DiscoverD,
	Manifest,
	StackTree,
    StackTreeD,
	TreeName,
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
impl MsgType {
    pub fn get_msg(packets: &Vec<Packet>) -> Result<Box<Message>, Error> {
        let serialized = Packetizer::unpacketize(packets).context(MessageError::Chain { func_name: "get_msg", comment: S("unpacketize")})?;
        //println!("Message get_msg: serialized {}, packets {:?}", serialized, packets);
		let type_msg = serde_json::from_str::<TypePlusMsg>(&serialized).context(MessageError::Chain { func_name: "get_msg", comment: S("deserialize MsgType")})?;
		let msg_type = type_msg.get_type();
		let serialized_msg = type_msg.get_serialized_msg();		
		Ok(match msg_type {
			MsgType::Discover   => Box::new(serde_json::from_str::<DiscoverMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("DiscoverMsg")})?),
			MsgType::DiscoverD  => Box::new(serde_json::from_str::<DiscoverDMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("DiscoverDMsg")})?),
			MsgType::Manifest   => Box::new(serde_json::from_str::<ManifestMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("ManifestMsg")})?),
            MsgType::StackTree  => Box::new(serde_json::from_str::<StackTreeMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("StackTreeMsg")})?),
            MsgType::StackTreeD => Box::new(serde_json::from_str::<StackTreeDMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("StackTreeDMsg")})?),
			MsgType::TreeName   => Box::new(serde_json::from_str::<TreeNameMsg>(&serialized_msg).context(MessageError::Chain { func_name: "get_msg", comment: S("TreeNameMsg")})?),
		})		
	}
	// A hack for printing debug output only for a specific message type
	pub fn is_type(packet: &Packet, type_of_msg: &str) -> bool {
		match format!("{}", packet).find(type_of_msg) {
			Some(_) => true,
			None => false
		}		
	}
}
impl fmt::Display for MsgType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			MsgType::Discover   => write!(f, "Discover"),
			MsgType::DiscoverD  => write!(f, "DiscoverD"),
			MsgType::Manifest   => write!(f, "Manifest"),
			MsgType::StackTree  => write!(f, "StackTree"),
            MsgType::StackTreeD => write!(f, "StackTreeD"),
			MsgType::TreeName   => write!(f, "TreeName"),
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

pub trait Message {
	fn get_header(&self) -> &MsgHeader;
    fn get_payload(&self) -> &MsgPayload;
    fn get_msg_type(&self) -> MsgType;
	fn is_rootward(&self) -> bool {
		match self.get_header().get_direction() {
			MsgDirection::Rootward => true,
			MsgDirection::Leafward => false
		}
	}
	fn is_leafward(&self) -> bool { !self.is_rootward() }
	fn get_count(&self) -> MsgID { self.get_header().get_count() }
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { self.get_payload().get_gvm_eqn() }
    fn to_packets(&self, tree_id: &TreeID) -> Result<Vec<Packet>, Error>
			where Self:std::marker::Sized + serde::Serialize {
		let bytes = Serializer::serialize(self).context(MessageError::Chain { func_name: "to_packets", comment: S("")})?;
		let direction = self.get_header().get_direction();
		let packets = Packetizer::packetize(tree_id, &bytes, direction);
		Ok(packets)
	}
	// If I had known then what I know now, I would have used an enum instead of a trait for Message.  I'm going with this
    // simple kludge so I can get on with things.
	fn process_ca(&mut self, cell_agent: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> { Err(MessageError::Process { func_name: "process_ca" }.into()) }
	fn process_noc(&self, noc: &Noc) -> Result<(), Error> { Err(MessageError::Process { func_name: "process_noc" }.into()) }
	fn process_vm(&mut self, vm: &mut VirtualMachine) -> Result<(), Error> { Err(MessageError::Process { func_name: "process_vm" }.into()) }
	fn process_service(&mut self, service: &mut Service) -> Result<(), Error> { Err(MessageError::Process { func_name: "process_service" }.into()) }
	fn get_payload_discover(&self) -> Result<&DiscoverPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_discover", msg_type: MsgType::Discover }.into()) }
	fn get_payload_discoverd(&self) -> Result<&DiscoverDPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_discoverd", msg_type: MsgType::DiscoverD }.into()) }
	fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_stack_tree", msg_type: MsgType::StackTree }.into()) }
    fn get_payload_stack_tree_d(&self) -> Result<&StackTreeMsgDPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_stack_tree", msg_type: MsgType::StackTree }.into()) }
	fn get_payload_manifest(&self) -> Result<&ManifestMsgPayload, Error> { Err(MessageError::Payload { func_name: "get_payload_manifest", msg_type: MsgType::Manifest }.into()) }
	fn get_payload_tree_name(&self) -> Result<&String, Error> { Err(MessageError::Payload { func_name: "get_payload_tree_names", msg_type: MsgType::TreeName }.into()) }
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
pub trait MsgPayload: fmt::Display {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation>;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgHeader {
	msg_count: MsgID,
	msg_type: MsgType,
	direction: MsgDirection,
	tree_map: MsgTreeMap,
}
impl MsgHeader {
	pub fn new(msg_type: MsgType, direction: MsgDirection) -> MsgHeader {
		let msg_count = get_next_count();
		MsgHeader { msg_type: msg_type, direction: direction, msg_count: msg_count, tree_map: HashMap::new() }
	}
	pub fn get_msg_type(&self) -> MsgType { self.msg_type }
	pub fn get_count(&self) -> MsgID { self.msg_count }
	pub fn get_direction(&self) -> MsgDirection { self.direction }
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
	pub fn new(tree_id: &TreeID, my_index: TableIndex, sending_cell_id: &CellID, 
			hops: PathLength, path: Path) -> DiscoverMsg {
		let header = MsgHeader::new(MsgType::Discover, MsgDirection::Leafward);
		//println!("DiscoverMsg: msg_count {}", header.get_count());
		let payload = DiscoverPayload::new(tree_id, my_index, &sending_cell_id, hops, path);
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
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
	fn get_payload_discover(&self) -> Result<&DiscoverPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let port_number = PortNumber::new(port_no, ca.get_no_ports()).context(MessageError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
		let hops = self.payload.get_hops();
		let path = self.payload.get_path();
		let my_index;
		{ // Limit scope of immutable borrow of self on the next line
			let new_tree_id = self.payload.get_tree_id();
			let senders_index = self.payload.get_index();
			let children = &mut HashSet::new();
			//println!("DiscoverMsg: tree_id {}, port_number {}", tree_id, port_number);
			let exists = ca.exists(new_tree_id);  // Have I seen this tree before?
            //if exists { println!("Cell {}: new_tree_id {} seen before on port {}", ca.get_id(), new_tree_id, *port_no); } else { println!("Cell {}: new_tree_id {} not seen before on port {}", ca.get_id(), new_tree_id, *port_no); }
			let status = if exists { traph::PortStatus::Pruned } else { traph::PortStatus::Parent };
			let mut eqns = HashSet::new();
			eqns.insert(GvmEqn::Recv("true"));
			eqns.insert(GvmEqn::Send("true"));
			eqns.insert(GvmEqn::Xtnd("true"));
			eqns.insert(GvmEqn::Save("false"));
			let gvm_equation = GvmEquation::new(eqns, Vec::new());
			let entry = ca.update_traph(new_tree_id, port_number, status, Some(&gvm_equation),
					children, senders_index, hops, Some(path)).context(MessageError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
			if exists { return Ok(()); } // Don't forward if traph exists for this tree - Simple quenching
            ca.update_base_tree_map(new_tree_id, new_tree_id);
			my_index = entry.get_index();
			// Send DiscoverD to sender
			let discoverd_msg = DiscoverDMsg::new(new_tree_id, my_index);
			//println!("DiscoverMsg {}: sending discoverd for tree {} packet {} {}",ca.get_id(), new_tree_id, packets[0].get_count(), discoverd_msg);
			let mask = Mask::new(port_number);
			ca.send_msg(&ca.get_connected_ports_tree_id(), &discoverd_msg, mask).context(MessageError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?;
			// Forward Discover on all except port_no with updated hops and path
		}
		self.update_discover_msg(&ca.get_id(), my_index);
		let packets = self.to_packets(&ca.get_control_tree_id()).context(MessageError::Chain { func_name: "process_ca", comment: S("")})?;
		let user_mask = DEFAULT_USER_MASK.all_but_port(PortNumber::new(port_no, ca.get_no_ports()).context(MessageError::Chain { func_name: "process_ca", comment: S("DiscoverMsg")})?);
		//println!("DiscoverMsg {}: forwarding packet {} on connected ports {}", ca.get_id(), packets[0].get_count(), self);
        let packets = ca.send_msg(&ca.get_connected_ports_tree_id(), &self.clone(), user_mask).context(MessageError::Chain {func_name: "process_ca", comment: S("DiscoverMsg")})?;
		ca.add_saved_discover(&packets); // Discover message are always saved for late port connect
		Ok(())
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
	index: TableIndex,
	sending_cell_id: CellID,
	hops: PathLength,
	path: Path,
	gvm_eqn: GvmEquation,
}
impl DiscoverPayload {
	fn new(tree_id: &TreeID, index: TableIndex, sending_cell_id: &CellID, hops: PathLength, path: Path)
            -> DiscoverPayload {
		let mut eqns = HashSet::new();
		eqns.insert(GvmEqn::Recv("true"));
		eqns.insert(GvmEqn::Send("true"));
		eqns.insert(GvmEqn::Xtnd("true"));
		eqns.insert(GvmEqn::Save("false"));
		let gvm_eqn = GvmEquation::new(eqns, Vec::new());
		DiscoverPayload { tree_id: tree_id.clone(), index: index, sending_cell_id: sending_cell_id.clone(),
			hops: hops, path: path, gvm_eqn: gvm_eqn }
	}
	//fn get_sending_cell(&self) -> CellID { self.sending_cell_id.clone() }
	fn get_hops(&self) -> PathLength { self.hops }
	fn hops_plus_one(&self) -> PathLength { PathLength(CellNo(**self.hops + 1)) }
	fn get_path(&self) -> Path { self.path }
	fn get_index(&self) -> TableIndex { self.index }
	fn get_tree_id(&self) -> &TreeID { &self.tree_id }
	fn set_hops(&mut self, hops: PathLength) { self.hops = hops; }
	fn set_path(&mut self, path: Path) { self.path = path; }
	fn set_index(&mut self, index: TableIndex) { self.index = index; }
	fn set_sending_cell(&mut self, sending_cell_id: CellID) { self.sending_cell_id = sending_cell_id; }
}
impl MsgPayload for DiscoverPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { Some(&self.gvm_eqn) }
}
impl fmt::Display for DiscoverPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Tree {}, sending cell {}, index {}, hops {}, path {}", self.tree_id,
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
	pub fn new(tree_id: &TreeID, index: TableIndex) -> DiscoverDMsg {
		// Note that direction is leafward so we can use the connected ports tree
		// If we send rootward, then the first recipient forwards the DiscoverD
		let tree_name = tree_id.stringify();
		let header = MsgHeader::new(MsgType::DiscoverD, MsgDirection::Leafward);
		let payload = DiscoverDPayload::new(tree_id, index);
		DiscoverDMsg { header: header, payload: payload }
	}
}
impl Message for DiscoverDMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
	fn get_payload_discoverd(&self) -> Result<&DiscoverDPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let tree_id = self.payload.get_tree_id();
		let my_index = self.payload.get_table_index();
		let mut children = HashSet::new();
		let port_number = PortNumber::new(port_no, MAX_PORTS).context(MessageError::Chain { func_name: "process_ca", comment: S("DiscoverDMsg")})?;
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
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverDPayload {
	tree_id: TreeID,
	my_index: TableIndex,
}
impl DiscoverDPayload {
	fn new(tree_id: &TreeID, index: TableIndex) -> DiscoverDPayload {
		DiscoverDPayload { tree_id: tree_id.clone(), my_index: index }
	}
	pub fn get_table_index(&self) -> TableIndex { self.my_index }
	fn get_tree_id(&self) -> &TreeID { &self.tree_id }
}
impl MsgPayload for DiscoverDPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
}
impl fmt::Display for DiscoverDPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "My table index {} for Tree {}", *self.my_index, self.tree_id)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeMsg {
	header: MsgHeader,
	payload: StackTreeMsgPayload
}
impl StackTreeMsg {
	pub fn new(new_tree_id: &TreeID, parent_tree_id: &TreeID, index: TableIndex, gvm_eqn: &GvmEquation) -> StackTreeMsg {
		let header = MsgHeader::new(MsgType::StackTree, MsgDirection::Leafward);
		let payload = StackTreeMsgPayload::new(new_tree_id, parent_tree_id, index, gvm_eqn);
		StackTreeMsg { header, payload}
	}
    fn update_payload(&self, payload: StackTreeMsgPayload) -> StackTreeMsg {
        StackTreeMsg { header: self.header.clone(), payload }
    }
    fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgPayload, Error> { Ok(&self.payload) }
}
impl Message for StackTreeMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgPayload, Error> { Ok(&self.payload) }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
        let f = "process_ca";
		println!("Cell {}: msg_tree_id {} Stack tree msg {}", ca.get_id(), msg_tree_id, self);
        let ref parent_tree_id = self.payload.get_parent_tree_id().clone();
        let ref new_tree_id = self.payload.get_new_tree_id().clone();
        let index = self.payload.get_table_index();
        let gvm_eqn = match self.payload.get_gvm_eqn() {
            Some(gvm) => gvm,
            None => return Err(MessageError::NoGvm { func_name: f }.into())
        };
        if let Some(mut entry) = ca.stack_tree(new_tree_id, parent_tree_id, gvm_eqn)? {
            let port_number = PortNumber::new(port_no, ca.get_no_ports())?;
            entry.add_other_index(port_number,  index);
            ca.update_entry(entry).context(MessageError::Chain { func_name: f, comment: S("")})?;
            let mut payload = self.get_payload_stack_tree()?.clone();
            payload.set_table_index(entry.get_index());
            let msg = self.update_payload(payload);
            let traph = ca.get_traph(&parent_tree_id)?;
            let parent_entry = traph.get_tree_entry(&parent_tree_id.get_uuid())?;
            let mask = parent_entry.get_mask();
            let variables = traph.get_params(gvm_eqn.get_variables())?;
            if gvm_eqn.eval_xtnd(&variables)? { ca.send_msg(ca.get_connected_ports_tree_id(), &msg, mask)?; }
            ca.send_msg(new_tree_id, &msg, DEFAULT_USER_MASK)?;
        }
		Ok(())
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
    new_tree_id: TreeID,
    parent_tree_id: TreeID,
    index: TableIndex,
    gvm_eqn: GvmEquation
}
impl StackTreeMsgPayload {
    fn new(new_tree_id: &TreeID, parent_tree_id: &TreeID, index: TableIndex, gvm_eqn: &GvmEquation) -> StackTreeMsgPayload {
        StackTreeMsgPayload { new_tree_id: new_tree_id.to_owned(), parent_tree_id: parent_tree_id.to_owned(),
            index, gvm_eqn: gvm_eqn.to_owned() }
    }
    pub fn get_new_tree_id(&self) -> &TreeID { &self.new_tree_id }
    pub fn get_parent_tree_id(&self) -> &TreeID { &self.parent_tree_id }
    pub fn get_table_index(&self) -> TableIndex { self.index }
    pub fn set_table_index(&mut self, index: TableIndex) { self.index = index }
}
impl MsgPayload for StackTreeMsgPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { Some(&self.gvm_eqn) }
}
impl fmt::Display for StackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tree {} stacked on tree {} with GVM {}", self.new_tree_id, self.parent_tree_id, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeDMsg {
    header: MsgHeader,
    payload: StackTreeMsgDPayload
}
impl StackTreeDMsg {
    pub fn new(tree_id: &TreeID, index: TableIndex,) -> StackTreeDMsg {
        let header = MsgHeader::new(MsgType::StackTreeD, MsgDirection::Rootward);
        let payload = StackTreeMsgDPayload::new(tree_id, index);
        StackTreeDMsg { header, payload}
    }
    fn update_payload(&self, payload: StackTreeMsgDPayload) -> StackTreeDMsg {
        StackTreeDMsg { header: self.header.clone(), payload }
    }
    fn get_payload_stack_tree(&self) -> Result<&StackTreeMsgDPayload, Error> { Ok(&self.payload) }
}
impl Message for StackTreeDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_payload_stack_tree_d(&self) -> Result<&StackTreeMsgDPayload, Error> { Ok(&self.payload) }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
        let f = "process_ca";
        println!("Cell {}: msg_tree_id {} Stack tree d msg {}", ca.get_id(), msg_tree_id, self);
        let ref tree_id = self.payload.get_tree_id().clone();
        let other_index = self.payload.get_table_index();
        let mut entry = ca.get_tree_entry(tree_id)?;
        entry.add_other_index(PortNumber::new(port_no,ca.get_no_ports())?, other_index);
        Ok(())
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
	index: TableIndex,
}
impl StackTreeMsgDPayload {
	fn new(tree_id: &TreeID, index: TableIndex) -> StackTreeMsgDPayload {
		StackTreeMsgDPayload { tree_id: tree_id.to_owned(), index }
	}
    pub fn get_tree_id(&self) -> &TreeID { &self.tree_id }
    pub fn get_table_index(&self) -> TableIndex { self.index }
}
impl MsgPayload for StackTreeMsgDPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
}
impl fmt::Display for StackTreeMsgDPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Tree {} at index {}", self.tree_id, *self.index)
	}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMsg {
	header: MsgHeader,
	payload: ManifestMsgPayload
}
impl ManifestMsg {
	pub fn new(tree_map: &MsgTreeMap, manifest: &Manifest) -> ManifestMsg {
		// Note that direction is leafward so cell agent will get the message
		let mut header = MsgHeader::new(MsgType::Manifest, MsgDirection::Leafward);
        header.set_tree_map(tree_map.to_owned());
		let payload = ManifestMsgPayload::new(&manifest);
		ManifestMsg { header: header, payload: payload }
	}
}
impl Message for ManifestMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
	fn get_payload_manifest(&self) -> Result<&ManifestMsgPayload, Error> { Ok(&self.payload) }
	fn process_ca(&mut self, ca: &mut CellAgent, msg_tree_id: &TreeID, port_no: PortNo) -> Result<(), Error> {
		let manifest = self.payload.get_manifest();
        let tree_map = self.header.get_tree_map();
		println!("ManifestMsg on cell {}: tree {} msg {}", ca.get_id(), msg_tree_id, manifest.get_id());
		Ok(ca.deploy(port_no, &msg_tree_id, tree_map, manifest).context(MessageError::Chain { func_name: "process_ca", comment: S("ManifestMsg")})?)
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
	tree_name: AllowedTree,
	manifest: Manifest 
}
impl ManifestMsgPayload {
	fn new(manifest: &Manifest) -> ManifestMsgPayload {
        let tree_name = manifest.get_deployment_tree();
		ManifestMsgPayload { tree_name: tree_name.clone(), manifest: manifest.clone() }
	}
	fn get_manifest(&self) -> &Manifest { &self.manifest }
	fn get_tree_name(&self) -> String { S(self.tree_name.get_name()) }
}
impl MsgPayload for ManifestMsgPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
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
	pub fn new(tree_name: &str) -> TreeNameMsg {
		// Note that direction is rootward so cell agent will get the message
		let header = MsgHeader::new(MsgType::TreeName, MsgDirection::Rootward);
		let payload = TreeNameMsgPayload::new(tree_name);
		TreeNameMsg { header: header, payload: payload }
	}
}
impl Message for TreeNameMsg {
	fn get_header(&self) -> &MsgHeader { &self.header }
	fn get_payload(&self) -> &MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_payload_tree_name(&self) -> Result<&String, Error> { Ok(self.payload.get_tree_name()) }
	fn process_noc(&self, noc: &Noc) -> Result<(), Error> {
		Ok(())
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
impl MsgPayload for TreeNameMsgPayload {
    fn get_gvm_eqn(&self) -> Option<&GvmEquation> { None }
}
impl fmt::Display for TreeNameMsgPayload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("Tree name for border cell {}", self.tree_name);
		write!(f, "{}", s)
	}
}
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum MessageError {
	#[fail(display = "MessageError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
    #[fail(display = "MessageError::Gvm {}: No GVM for this message type {}", func_name, msg_type)]
    Gvm { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "MessageError::InvalidMsgType {}: Invalid message type {} from packet assembler", func_name, msg_type)]
    InvalidMsgType { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "MessageError::Message {}: Message error from {}", func_name, handler)]
    Message { func_name: &'static str, handler: &'static str },
    #[fail(display = "MessageError::NoGmv {}: No GVM in StackTreeMsg", func_name)]
    NoGvm { func_name: &'static str },
    #[fail(display = "MessageError::Payload {}: Wrong payload for type {}", func_name, msg_type)]
    Payload { func_name: &'static str, msg_type: MsgType },
    #[fail(display = "MessageError::Process {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
    #[fail(display = "MessageError::TreeMapEntry {}: No tree named {} in map", func_name, tree_name)]
    TreeMapEntry { func_name: &'static str, tree_name: String }
}
