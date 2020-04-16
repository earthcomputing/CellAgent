use std;
use std::{fmt,
          collections::{HashMap, HashSet}};

use serde;
use serde_json;

use crate::app_message::{SenderMsgSeqNo, AppMsgDirection, AppInterapplicationMsg, get_next_count};
use crate::cellagent::{CellAgent};
use crate::config::{CellQty, PathLength};
use crate::gvm_equation::{GvmEquation, GvmEqn};
use crate::name::{Name, CellID, PortTreeID, SenderID, TreeID};
use crate::packet::{Packet, Packetizer, Serializer};
use crate::packet_engine::NumberOfPackets;
use crate::uptree_spec::{AllowedTree, Manifest};
use crate::utility::{ByteArray, PortNo, S, Path};

pub type MsgTreeMap = HashMap<String, TreeID>; // Must be String for serialization

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum MsgType {
    Entl,        // Needed for the msg_type hack, otherwise panic
    DeleteTree,
    Discover,
    DiscoverD,
    Failover,
    FailoverD,
    Hello,
    Interapplication,
    Manifest,
    Prepare,
    PrepareD,
    StackTree,
    StackTreeD,
    TreeName
}
impl MsgType {
    // Used for debug hack in packet_engine
    pub fn get_msg(packets: &Vec<Packet>) -> Result<Box<dyn Message>, Error> {
        let _f = "get_msg";
        let bytes = Packetizer::unpacketize(packets).context(MessageError::Chain { func_name: _f, comment: S("unpacketize")})?;
        //println!("Message get_msg: serialized {}, packets {:?}", serialized, packets);
        let serialized = bytes.to_string()?;
        let msg = serde_json::from_str(&serialized)?;
        Ok(msg)
    }
    pub fn msg_from_bytes(bytes: &ByteArray) -> Result<Box<dyn Message>, Error> {
        let _f = "msg_from_bytes";
        let serialized = bytes.to_string()?;
        let msg = serde_json::from_str(&serialized)?;
        Ok(msg)
    }
    // A hack for printing debug output only for a specific message type
    pub fn is_type(packet: &Packet, type_of_msg: MsgType) -> bool {
        format!("{}", packet).find(&(S(type_of_msg) + "\\")).is_some()
    }
    // A hack for finding the message type
    pub fn msg_type(packet: &Packet) -> MsgType {
        if      MsgType::is_type(packet, MsgType::Interapplication) { MsgType::Interapplication }
        else if MsgType::is_type(packet, MsgType::DeleteTree)  { MsgType::DeleteTree }
        else if MsgType::is_type(packet, MsgType::Discover)    { MsgType::Discover }
        else if MsgType::is_type(packet, MsgType::DiscoverD)   { MsgType::DiscoverD }
        else if MsgType::is_type(packet, MsgType::Failover)    { MsgType::Failover }
        else if MsgType::is_type(packet, MsgType::FailoverD)   { MsgType::FailoverD }
        else if MsgType::is_type(packet, MsgType::Hello)       { MsgType::Hello }
        else if MsgType::is_type(packet, MsgType::Manifest)    { MsgType::Manifest }
        else if MsgType::is_type(packet, MsgType::Prepare)     { MsgType::Prepare }
        else if MsgType::is_type(packet, MsgType::PrepareD)    { MsgType::PrepareD }
        else if MsgType::is_type(packet, MsgType::StackTree)   { MsgType::StackTree }
        else if MsgType::is_type(packet, MsgType::StackTreeD)  { MsgType::StackTreeD }
        else if MsgType::is_type(packet, MsgType::TreeName)    { MsgType::TreeName }
        else { MsgType::Entl }
    }
}
impl fmt::Display for MsgType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MsgType::Entl              => "Entl",
            MsgType::DeleteTree        => "DeleteTree",
            MsgType::Discover          => "Discover",
            MsgType::DiscoverD         => "DiscoverD",
            MsgType::Failover          => "Failover",
            MsgType::FailoverD         => "FailoverD",
            MsgType::Hello             => "Hello",
            MsgType::Interapplication  => "Interapplication",
            MsgType::Manifest          => "Manifest",
            MsgType::Prepare           => "Prepare",
            MsgType::PrepareD          => "PrepareD",
            MsgType::StackTree         => "StackTree",
            MsgType::StackTreeD        => "StackTreeD",
            MsgType::TreeName          => "TreeName",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum MsgDirection {
    Rootward,
    Leafward
}
impl From<AppMsgDirection> for MsgDirection {
    fn from(d: AppMsgDirection) -> Self {
        match d {
            AppMsgDirection::Rootward => MsgDirection::Rootward,
            AppMsgDirection::Leafward => MsgDirection::Leafward
        }
    }
}
impl fmt::Display for MsgDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MsgDirection::Rootward => "Rootward",
            MsgDirection::Leafward => "Leafward"
        };
        write!(f, "{}", s)
    }
}
#[typetag::serde(tag = "ec_msg_type")]
pub trait Message {
    fn get_header(&self) -> &MsgHeader;
    fn get_payload(&self) -> &dyn MsgPayload;
    fn get_msg_type(&self) -> MsgType;
    fn get_port_tree_id(&self) -> PortTreeID { PortTreeID::default() }
    fn is_rootward(&self) -> bool {
        match self.get_header()._get_direction() {
            MsgDirection::Rootward => true,
            MsgDirection::Leafward => false
        }
    }
    fn is_leafward(&self) -> bool { !self.is_rootward() }
    fn is_ait(&self) -> bool { self.get_header().get_ait() }
    fn value(&self) -> serde_json::Value;
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.get_header().get_sender_msg_seq_no() }
    fn get_sender_id(&self) -> SenderID { self.get_header().get_sender_id() }
    fn to_bytes(&self) -> Result<ByteArray, Error> where Self: serde::Serialize + Sized {
        let _f = "to_bytes";
        let bytes = Serializer::serialize(self).context(MessageError::Chain { func_name: _f, comment: S("")})?;
        Ok(ByteArray::new(&bytes))
    }
    fn to_packets(&self, tree_id: TreeID) -> Result<Vec<Packet>, Error>
            where Self:std::marker::Sized + serde::Serialize {
        let _f = "to_packets";
        let bytes = Serializer::serialize(self).context(MessageError::Chain { func_name: _f, comment: S("")})?;
        let packets = Packetizer::packetize(&tree_id.get_uuid(),
                                            self.get_sender_msg_seq_no(), &ByteArray::new(&bytes))?;
        Ok(packets)
    }
    fn process_ca(&mut self, _cell_agent: &mut CellAgent, _port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error>;
}
impl fmt::Display for dyn Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{} {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[typetag::serde(tag = "app_msg_payload_type")]
pub trait MsgPayload: fmt::Display {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgHeader {
    sender_msg_seq_no: SenderMsgSeqNo, // Debugging only?
    sender_id: SenderID, // Used to find set of AllowedTrees
    is_ait: bool,
    msg_type: MsgType,
    direction: MsgDirection,
    tree_map: MsgTreeMap,
}
impl MsgHeader {
    pub fn new(sender_id: SenderID, is_ait: bool, tree_map: MsgTreeMap,
               msg_type: MsgType, direction: MsgDirection) -> MsgHeader {
        let msg_count = get_next_count();
        MsgHeader { sender_id, is_ait, msg_type, direction, sender_msg_seq_no: msg_count, tree_map: tree_map.clone() }
    }
    pub fn get_msg_type(&self) -> MsgType { self.msg_type }
    pub fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
    pub fn get_sender_id(&self) -> SenderID { self.sender_id }
    pub fn get_ait(&self) -> bool { self.is_ait }
    pub fn _get_direction(&self) -> MsgDirection { self.direction }
    pub fn get_tree_map(&self) -> &MsgTreeMap { &self.tree_map }
}
impl fmt::Display for MsgHeader { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        let s = format!("Message {} {} '{}'", *self.sender_msg_seq_no, self.msg_type, self.direction);
        write!(f, "{}", s) 
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverMsg {
    header: MsgHeader,
    payload: DiscoverPayload
}
impl DiscoverMsg {
    pub fn new(sender_id: SenderID, port_tree_id: PortTreeID, hops: PathLength, path: Path) -> DiscoverMsg {
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::Discover, MsgDirection::Leafward);
        let payload = DiscoverPayload::new(port_tree_id, hops, path);
        DiscoverMsg { header, payload }
    }
    pub fn update(&self, cell_id: CellID) -> DiscoverMsg {
        let mut msg = self.clone();
        let hops = self.update_hops();
        let path = self.update_path();
        msg.payload.set_hops(hops);
        msg.payload.set_path(path);
        msg
    }
    pub fn update_hops(&self) -> PathLength { self.payload.hops_plus_one() }
    pub fn update_path(&self) -> Path { self.payload.get_path() } // No change per hop
    pub fn get_payload(&self) -> &DiscoverPayload { &self.payload }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id() }
}
#[typetag::serde]
impl Message for DiscoverMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id().clone() }
    fn value(&self) -> serde_json::Value where Self: serde::Serialize {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_discover_msg(self, port_no)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct DiscoverPayload {
    port_tree_id: PortTreeID,
    root_port_no: PortNo,
    hops: PathLength,
    path: Path,
    gvm_eqn: GvmEquation,
}
impl DiscoverPayload {
    fn new(port_tree_id: PortTreeID,  hops: PathLength, path: Path) -> DiscoverPayload {
        let mut eqns = HashSet::new();
        eqns.insert(GvmEqn::Recv("true"));
        eqns.insert(GvmEqn::Send("true"));
        eqns.insert(GvmEqn::Xtnd("true"));
        eqns.insert(GvmEqn::Save("false"));
        let gvm_eqn = GvmEquation::new(&eqns, &Vec::new());
        let root_port_no = port_tree_id.get_port_no();
        DiscoverPayload { port_tree_id: port_tree_id.clone(), root_port_no, hops, path, gvm_eqn }
    }
    pub fn get_hops(&self) -> PathLength { self.hops }
    pub fn hops_plus_one(&self) -> PathLength { PathLength(CellQty(**self.hops + 1)) }
    pub fn get_path(&self) -> Path { self.path }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.port_tree_id }
    pub fn _get_root_port_no(&self) -> PortNo { self.root_port_no }
    pub fn set_hops(&mut self, hops: PathLength) { self.hops = hops; }
    pub fn set_path(&mut self, path: Path) { self.path = path; }
}
#[typetag::serde]
impl MsgPayload for DiscoverPayload {}
impl fmt::Display for DiscoverPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Port tree {}, hops {}, path {}", self.port_tree_id,
                        **self.hops, self.path);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDMsg {
    header: MsgHeader,
    payload: DiscoverDPayload
}
impl DiscoverDMsg {
    pub fn new(in_reply_to: SenderMsgSeqNo, sender_id: SenderID,
               port_tree_id: PortTreeID, path: Path, discover_type: DiscoverDType) -> DiscoverDMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the DiscoverD
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::DiscoverD, MsgDirection::Leafward);
        let payload = DiscoverDPayload::new(in_reply_to,
                                            port_tree_id, path, discover_type);
        DiscoverDMsg { header, payload }
    }
    pub fn get_payload(&self) -> &DiscoverDPayload { &self.payload }
    pub fn get_in_reply_to(&self) -> SenderMsgSeqNo { self.payload.get_in_reply_to() }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id() }
    pub fn get_path(&self) -> Path { self.payload.get_path() }
    pub fn get_discoverd_type(&self) -> DiscoverDType { self.payload.get_discover_type() }
}
#[typetag::serde]
impl Message for DiscoverDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool,) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_discover_d_msg(&self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDPayload {
    in_reply_to: SenderMsgSeqNo,
    port_tree_id: PortTreeID,
    root_port_no: PortNo,
    path: Path,
    discover_type: DiscoverDType
}
impl DiscoverDPayload {
    fn new(in_reply_to: SenderMsgSeqNo, port_tree_id: PortTreeID,
           path: Path, discover_type: DiscoverDType) -> DiscoverDPayload {
        let root_port_no = port_tree_id.get_port_no();
        DiscoverDPayload { in_reply_to, port_tree_id: port_tree_id.clone(),
            path, root_port_no, discover_type }
    }
    fn get_in_reply_to(&self) -> SenderMsgSeqNo { self.in_reply_to }
    fn get_port_tree_id(&self) -> PortTreeID { self.port_tree_id }
    fn get_path(&self) -> Path { self.path }
    fn get_discover_type(&self) -> DiscoverDType { self.discover_type }
}
#[typetag::serde]
impl MsgPayload for DiscoverDPayload {}
impl fmt::Display for DiscoverDPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "My Tree {} {}", self.port_tree_id, self.discover_type)
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiscoverDType {
    Parent,
    NonParent
}
impl fmt::Display for DiscoverDType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoverDType::Parent => write!(f, "First"),
            DiscoverDType::NonParent => write!(f, "Subsequent")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareMsg {
    header: MsgHeader,
    payload: PreparePayload
}
impl PrepareMsg {
    pub fn new(in_reply_to: SenderMsgSeqNo, sender_id: SenderID, tree_id: TreeID) -> PrepareMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the Prepare
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::Prepare, MsgDirection::Leafward);
        let payload = PreparePayload::new(in_reply_to, tree_id);
        PrepareMsg { header, payload }
    }
    pub fn get_payload(&self) -> &PreparePayload { &self.payload }
    pub fn get_tree_id(&self) -> TreeID { self.payload.get_tree_id() }
}
#[typetag::serde]
impl Message for PrepareMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool,) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_prepare_msg(&self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparePayload {
    in_reply_to: SenderMsgSeqNo,
    tree_id: TreeID,
}
impl PreparePayload {
    fn new(in_reply_to: SenderMsgSeqNo, tree_id: TreeID,) -> PreparePayload {
        PreparePayload { in_reply_to, tree_id }
    }
    fn get_tree_id(&self) -> TreeID { self.tree_id }
}
#[typetag::serde]
impl MsgPayload for PreparePayload {}
impl fmt::Display for PreparePayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree {}", self.tree_id)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareDMsg {
    header: MsgHeader,
    payload: PrepareDPayload
}
impl PrepareDMsg {
    pub fn new(in_reply_to: SenderMsgSeqNo, sender_id: SenderID,  tree_id: TreeID) -> PrepareDMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the PrepareD
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::DiscoverD, MsgDirection::Leafward);
        let payload = PrepareDPayload::new(in_reply_to, tree_id);
        PrepareDMsg { header, payload }
    }
    pub fn get_payload(&self) -> &PrepareDPayload { &self.payload }
    pub fn get_tree_id(&self) -> TreeID { self.payload.get_tree_id() }
}
#[typetag::serde]
impl Message for PrepareDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_prepare_d_msg(&self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareDPayload {
    in_reply_to: SenderMsgSeqNo,
    tree_id: TreeID,
}
impl PrepareDPayload {
    fn new(in_reply_to: SenderMsgSeqNo, tree_id: TreeID) -> PrepareDPayload {
        PrepareDPayload { in_reply_to, tree_id }
    }
    fn get_tree_id(&self) -> TreeID { self.tree_id }
}
#[typetag::serde]
impl MsgPayload for PrepareDPayload {}
impl fmt::Display for PrepareDPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree {}", self.tree_id)
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum PrepareDType {
    Parent,
    NonParent
}
impl fmt::Display for PrepareDType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareDType::Parent => write!(f, "First"),
            PrepareDType::NonParent => write!(f, "Subsequent")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverMsg {
    header: MsgHeader,
    payload: FailoverMsgPayload
}
impl FailoverMsg {
    pub fn new(sender_id: SenderID, rw_port_tree_id: PortTreeID, lw_port_tree_id: PortTreeID,
               path: Path, broken_tree_ids: &HashSet<PortTreeID>) -> FailoverMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the FailoverMsg
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::Failover, MsgDirection::Leafward);
        let payload = FailoverMsgPayload::new(rw_port_tree_id, lw_port_tree_id,
                                              broken_tree_ids, path);
        FailoverMsg { header, payload }
    }
    pub fn get_payload(&self) -> &FailoverMsgPayload { &self.payload }
}
#[typetag::serde]
impl Message for FailoverMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_failover_msg(&self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverMsgPayload {
    rw_port_tree_id: PortTreeID,
    lw_port_tree_id: PortTreeID,
    broken_tree_ids: HashSet<PortTreeID>,
    broken_path: Path,
}
impl FailoverMsgPayload {
    fn new(rw_port_tree_id: PortTreeID, lw_port_tree_id: PortTreeID,
           broken_tree_ids: &HashSet<PortTreeID>, path: Path)
                -> FailoverMsgPayload {
        FailoverMsgPayload { rw_port_tree_id: rw_port_tree_id.clone(), lw_port_tree_id: lw_port_tree_id.clone(),
            broken_tree_ids: broken_tree_ids.clone(),
            broken_path: path
        }
    }
    pub fn get_rw_port_tree_id(&self) -> PortTreeID { self.rw_port_tree_id }
    pub fn get_lw_port_tree_id(&self) -> PortTreeID { self.lw_port_tree_id }
    pub fn get_broken_port_tree_ids(&self) -> &HashSet<PortTreeID> { &self.broken_tree_ids }
    pub fn get_broken_path(&self) -> Path { self.broken_path }
}
#[typetag::serde]
impl MsgPayload for FailoverMsgPayload {}
impl fmt::Display for FailoverMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rootward Tree {}, root port {}", self.rw_port_tree_id, self.broken_path)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverDMsg {
    header: MsgHeader,
    payload: FailoverDMsgPayload
}
impl FailoverDMsg {
    pub fn new(in_reply_to: SenderMsgSeqNo, sender_id: SenderID, response: FailoverResponse,
               no_packets: NumberOfPackets, failover_payload: &FailoverMsgPayload) -> FailoverDMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the FailoverMsg
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::FailoverD, MsgDirection::Leafward);
        let payload = FailoverDMsgPayload::new(in_reply_to, response,
                                               no_packets, failover_payload);
        FailoverDMsg { header, payload }
    }
    pub fn get_payload(&self) -> &FailoverDMsgPayload { &self.payload }
}
#[typetag::serde]
impl Message for FailoverDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_failover_d_msg(self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverDMsgPayload {
    in_reply_to: SenderMsgSeqNo,
    response: FailoverResponse,
    no_packets: NumberOfPackets,
    failover_payload: FailoverMsgPayload
}
impl FailoverDMsgPayload {
    fn new(in_reply_to: SenderMsgSeqNo, response: FailoverResponse, no_packets: NumberOfPackets,
           failover_payload: &FailoverMsgPayload) -> FailoverDMsgPayload {
        FailoverDMsgPayload { in_reply_to, response, no_packets, failover_payload: failover_payload.clone() }
    }
    pub fn get_response(&self) -> FailoverResponse { self.response }
    pub fn get_number_of_packets(&self) -> NumberOfPackets { self.no_packets }
    pub fn get_failover_payload(&self) -> &FailoverMsgPayload { &self.failover_payload }
    pub fn _get_rw_port_tree_id(&self) -> PortTreeID { self.failover_payload.get_rw_port_tree_id() }
    pub fn _get_lw_port_tree_id(&self) -> PortTreeID { self.failover_payload.get_lw_port_tree_id() }
}
#[typetag::serde]
impl MsgPayload for FailoverDMsgPayload {}
impl fmt::Display for FailoverDMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Response {} Failover payload {}", self.response, self.failover_payload)
    }
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum FailoverResponse {
    Success,
    Failure
}
impl fmt::Display for FailoverResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FailoverResponse::Success => write!(f, "Success"),
            FailoverResponse::Failure => write!(f, "Failure")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMsg {
    header: MsgHeader,
    payload: HelloMsgPayload
}
impl HelloMsg {
    pub fn new(sender_id: SenderID, cell_id: CellID, port_no: PortNo) -> HelloMsg {
        // Note that direction is leafward so we can use the connected ports tree
        // If we send rootward, then the first recipient forwards the FailoverMsg
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::Hello, MsgDirection::Leafward);
        let payload = HelloMsgPayload::new(cell_id, port_no);
        HelloMsg { header, payload }
    }
    pub fn get_payload(&self) -> &HelloMsgPayload { &self.payload }
}
#[typetag::serde]
impl Message for HelloMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.process_hello_msg(self, port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMsgPayload {
    cell_id: CellID,
    port_no: PortNo
}
impl HelloMsgPayload {
    fn new(cell_id: CellID, port_no: PortNo) -> HelloMsgPayload {
        HelloMsgPayload { cell_id, port_no }
    }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_port_no(&self) -> &PortNo { &self.port_no }
}
#[typetag::serde]
impl MsgPayload for HelloMsgPayload {}
impl fmt::Display for HelloMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Neigbor cell id {} neighbor port {}", self.cell_id, *self.port_no)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTreeMsg {
    header: MsgHeader,
    payload: DeleteTreeMsgPayload
}
impl DeleteTreeMsg {
    pub fn new(sender_id: SenderID, delete_tree_id: TreeID) -> DeleteTreeMsg {
        let header = MsgHeader::new(
            sender_id, false, HashMap::new(),
            MsgType::DeleteTree,
            MsgDirection::Leafward);
        let payload = DeleteTreeMsgPayload::new(delete_tree_id);
        DeleteTreeMsg { header, payload }
    }
    pub fn get_delete_tree_id(&self) -> &TreeID { &self.payload.get_delete_tree_id() }
}
#[typetag::serde]
impl Message for DeleteTreeMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, _port_no: PortNo,
                  _msg_port_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        let delete_tree_id = self.get_delete_tree_id();
        cell_agent.process_delete_tree_msg(delete_tree_id)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeleteTreeMsgPayload {
    delete_tree_id: TreeID
}
impl DeleteTreeMsgPayload {
    pub fn new(delete_tree_id: TreeID) -> DeleteTreeMsgPayload {
        DeleteTreeMsgPayload { delete_tree_id }
    }
    fn get_delete_tree_id(&self) -> &TreeID { &self.delete_tree_id }
}
#[typetag::serde]
impl MsgPayload for DeleteTreeMsgPayload {}
impl fmt::Display for DeleteTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.delete_tree_id)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeMsg {
    header: MsgHeader,
    payload: StackTreeMsgPayload
}
impl StackTreeMsg {
    pub fn new(sender_id: SenderID, new_tree_name: &AllowedTree, new_tree_id: TreeID, parent_tree_id: TreeID,
               direction: MsgDirection, gvm_eqn: &GvmEquation) -> StackTreeMsg {
        let header = MsgHeader::new( sender_id, true, HashMap::new(),
                                     MsgType::StackTree, direction);
        let payload = StackTreeMsgPayload::new(new_tree_name,
                                               new_tree_id, parent_tree_id, gvm_eqn);
        StackTreeMsg { header, payload}
    }
    pub fn get_payload(&self) -> &StackTreeMsgPayload { &self.payload }
    fn _get_port_tree_id(&self) -> PortTreeID { self.payload._get_port_tree_id() }
}
#[typetag::serde]
impl Message for StackTreeMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  msg_port_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_stack_tree_msg(self, port_no, msg_port_tree_id)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct StackTreeMsgPayload {
    allowed_tree: AllowedTree,
    new_port_tree_id: PortTreeID,
    parent_port_tree_id: PortTreeID,
    gvm_eqn: GvmEquation
}
impl StackTreeMsgPayload {
    fn new(allowed_tree: &AllowedTree, new_tree_id: TreeID, parent_tree_id: TreeID, gvm_eqn: &GvmEquation) -> StackTreeMsgPayload {
        StackTreeMsgPayload { allowed_tree: allowed_tree.clone(), new_port_tree_id: new_tree_id.to_port_tree_id_0(),
            parent_port_tree_id: parent_tree_id.to_port_tree_id_0(), gvm_eqn: gvm_eqn.clone() }
    }
    pub fn get_allowed_tree(&self) -> &AllowedTree { &self.allowed_tree }
    pub fn get_new_port_tree_id(&self) -> PortTreeID { self.new_port_tree_id }
    pub fn get_parent_port_tree_id(&self) -> PortTreeID { self.parent_port_tree_id }
    pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
    fn _get_port_tree_id(&self) -> PortTreeID { self.new_port_tree_id }
}
#[typetag::serde]
impl MsgPayload for StackTreeMsgPayload {}
impl fmt::Display for StackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree {} stacked on tree {} {}", self.new_port_tree_id, self.parent_port_tree_id, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTreeDMsg {
    header: MsgHeader,
    payload: StackTreeMsgDPayload
}
impl StackTreeDMsg {
    pub fn new(in_reply_to: SenderMsgSeqNo, sender_id: SenderID, port_tree_id: PortTreeID,
               parent_port_tree_id: PortTreeID, join: bool) -> StackTreeDMsg {
        let header = MsgHeader::new(sender_id, true, HashMap::new(),
                                    MsgType::StackTreeD, MsgDirection::Leafward);
        let payload = StackTreeMsgDPayload::new(in_reply_to, port_tree_id, parent_port_tree_id, join);
        StackTreeDMsg { header, payload}
    }
    pub fn get_payload(&self) -> &StackTreeMsgDPayload { &self.payload }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id() }
    pub fn get_parent_port_tree_id(&self) -> PortTreeID { self.payload.get_parent_port_tree_id() }
    pub fn is_joining(&self) -> bool { self.payload.get_join() }
}
#[typetag::serde]
impl Message for StackTreeDMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.header.msg_type }
    fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id().clone() }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_stack_tree_d_msg(&self, port_no)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct StackTreeMsgDPayload {
    in_reply_to: SenderMsgSeqNo,
    port_tree_id: PortTreeID,
    parent_port_tree_id: PortTreeID,
    join: bool,
}
impl StackTreeMsgDPayload {
    fn new(in_reply_to: SenderMsgSeqNo, port_tree_id: PortTreeID,
           parent_port_tree_id: PortTreeID, join: bool)  -> StackTreeMsgDPayload {
        StackTreeMsgDPayload { in_reply_to, port_tree_id, parent_port_tree_id, join }
    }
    fn get_port_tree_id(&self) -> PortTreeID { self.port_tree_id }
    fn get_parent_port_tree_id(&self) -> PortTreeID { self.parent_port_tree_id }
    fn get_join(&self) -> bool { self.join }
}
#[typetag::serde]
impl MsgPayload for StackTreeMsgDPayload {}
impl fmt::Display for StackTreeMsgDPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "In reply to {} Tree {} on parent {}", *self.in_reply_to,
               self.port_tree_id, self.parent_port_tree_id)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMsg {
    header: MsgHeader,
    payload: ManifestMsgPayload
}
impl ManifestMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, deploy_tree_id: TreeID, tree_map: &MsgTreeMap, manifest: &Manifest) -> ManifestMsg {
        // Note that direction is leafward so cell agent will get the message
        let header = MsgHeader::new(sender_id, is_ait, tree_map.clone(),
                                        MsgType::Manifest, MsgDirection::Leafward);

        let payload = ManifestMsgPayload::new(&deploy_tree_id.to_port_tree_id_0(), &manifest);
        ManifestMsg { header, payload }
    }
    pub fn get_payload(&self) -> &ManifestMsgPayload { &self.payload }
    pub fn _get_port_tree_id(&self) -> PortTreeID { self.payload._get_port_tree_id() }
}
#[typetag::serde]
impl Message for ManifestMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  msg_port_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_manifest_msg(&self, port_no, msg_port_tree_id)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct ManifestMsgPayload {
    deploy_port_tree_id: PortTreeID,
    tree_name: AllowedTree,
    manifest: Manifest 
}
impl ManifestMsgPayload {
    fn new(deploy_port_tree_id: &PortTreeID, manifest: &Manifest) -> ManifestMsgPayload {
        let tree_name = manifest.get_deployment_tree();
        ManifestMsgPayload { deploy_port_tree_id: deploy_port_tree_id.clone(), tree_name: tree_name.clone(),
            manifest: manifest.clone() }
    }
    pub fn get_manifest(&self) -> &Manifest { &self.manifest }
    pub fn get_deploy_port_tree_id(&self) -> PortTreeID { self.deploy_port_tree_id }
    pub fn _get_port_tree_id(&self) -> PortTreeID { self.deploy_port_tree_id }
}
#[typetag::serde]
impl MsgPayload for ManifestMsgPayload {}
impl fmt::Display for ManifestMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Manifest: {}", self.get_manifest());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterapplicationMsg {
    header: MsgHeader,
    payload: InterapplicationMsgPayload
}
impl InterapplicationMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, tree_id: TreeID, direction: MsgDirection,
               tree_map: &MsgTreeMap, app_msg: &AppInterapplicationMsg) -> InterapplicationMsg {
        let header = MsgHeader::new(sender_id, is_ait, tree_map.clone(),
                                    MsgType::Interapplication, direction);
        let payload = InterapplicationMsgPayload::new(&tree_id.to_port_tree_id_0(), app_msg);
        InterapplicationMsg { header, payload }
    }
    pub fn get_payload(&self) -> &InterapplicationMsgPayload { &self.payload }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.payload.get_port_tree_id() }
}
#[typetag::serde]
impl Message for InterapplicationMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_interapplication_msg(self, port_no)
    }
}
impl fmt::Display for InterapplicationMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.header, self.payload);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct InterapplicationMsgPayload {
    port_tree_id: PortTreeID,
    app_msg: AppInterapplicationMsg,
}
impl InterapplicationMsgPayload {
    fn new(port_tree_id: &PortTreeID, app_msg: &AppInterapplicationMsg) -> InterapplicationMsgPayload {
        InterapplicationMsgPayload { port_tree_id: port_tree_id.clone(), app_msg: app_msg.clone() }
    }
    pub fn get_app_msg(&self) -> &AppInterapplicationMsg { &self.app_msg }
    pub fn _get_tree_id(&self) -> &PortTreeID { &self.port_tree_id }
    pub fn get_port_tree_id(&self) -> PortTreeID { self.port_tree_id }
}
#[typetag::serde]
impl MsgPayload for InterapplicationMsgPayload {}
impl fmt::Display for InterapplicationMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sending application message on tree {}: {}", self.port_tree_id, self.app_msg.to_string())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNameMsg {
    header: MsgHeader,
    payload: TreeNameMsgPayload
}
impl TreeNameMsg {
    pub fn _new(sender_id: SenderID, tree_name: &str) -> TreeNameMsg {
        // Note that direction is rootward so cell agent will get the message
        let header = MsgHeader::new(sender_id, false, HashMap::new(),
                                    MsgType::TreeName, MsgDirection::Rootward);
        let payload = TreeNameMsgPayload::_new(tree_name);
        TreeNameMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TreeNameMsgPayload { &self.payload }
    pub fn _get_tree_name(&self) -> &String { self.payload._get_tree_name() }
}
#[typetag::serde]
impl Message for TreeNameMsg {
    fn get_header(&self) -> &MsgHeader { &self.header }
    fn get_payload(&self) -> &dyn MsgPayload { &self.payload }
    fn get_msg_type(&self) -> MsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&mut self, cell_agent: &mut CellAgent, port_no: PortNo,
                  _msg_tree_id: PortTreeID, _is_ait: bool) -> Result<(), Error> {
        cell_agent.process_tree_name_msg(self, port_no)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TreeNameMsgPayload {
    tree_name: String,
}
impl TreeNameMsgPayload {
    fn _new(tree_name: &str) -> TreeNameMsgPayload {
        TreeNameMsgPayload { tree_name: S(tree_name) }
    }
    fn _get_tree_name(&self) -> &String { &self.tree_name }
}
#[typetag::serde]
impl MsgPayload for TreeNameMsgPayload {}
impl fmt::Display for TreeNameMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
//    Gvm { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "MessageError::InvalidAppMsgType {}: Invalid message type {} from packet assembler", func_name, msg_type)]
//    InvalidAppMsgType { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "MessageError::Message {}: Message error from {}", func_name, handler)]
//    Message { func_name: &'static str, handler: &'static str },
//    #[fail(display = "MessageError::NoGmv {}: No GVM in StackTreeMsg", func_name)]
//    NoGvm { func_name: &'static str },
//    #[fail(display = "MessageError::Payload {}: Wrong payload for type {}", func_name, msg_type)]
//    Payload { func_name: &'static str, msg_type: AppMsgType },
    #[fail(display = "MessageError::Process {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
//    #[fail(display = "MessageError::TreeID {}: No TreeID ", func_name, msg_type: AppMsgType)]
//    TreeID { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "MessageError::TreeMapEntry {}: No tree named {} in map", func_name, tree_name)]
//    TreeMapEntry { func_name: &'static str, tree_name: String }
}
