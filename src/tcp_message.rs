use std::{fmt,
          ops::{Deref},
          sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering},
          sync::mpsc,
};

use serde;
use serde_json;

use crate::config::{ByteArray};
use crate::gvm_equation::{GvmEquation, GvmEqn};
use crate::name::{SenderID};
use crate::uptree_spec::{AllowedTree, Manifest};
use crate::utility::{S};

// This is currently at the cell level, but could be placed at the up-tree level.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SenderMsgSeqNo(pub u64);
impl Deref for SenderMsgSeqNo { type Target = u64; fn deref(&self) -> &Self::Target { &self.0 } }
static MESSAGE_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> SenderMsgSeqNo { SenderMsgSeqNo(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) as u64) }

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            TcpMsgType::Application  => "Application",
            TcpMsgType::DeleteTree   => "DeleteTree",
            TcpMsgType::Manifest     => "Manifest",
            TcpMsgType::Query        => "Query",
            TcpMsgType::StackTree    => "StackTree",
            TcpMsgType::TreeName     => "TreeName",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum TcpMsgDirection {
    Rootward,
    Leafward
}
impl fmt::Display for TcpMsgDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            TcpMsgDirection::Rootward => "Rootward",
            TcpMsgDirection::Leafward => "Leafward"
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TypePlusTcpMsg {
    msg_type: TcpMsgType,
    serialized_msg: String
}
impl TypePlusTcpMsg {
    pub fn new(msg_type: TcpMsgType, serialized_msg: String) -> TypePlusTcpMsg {
        TypePlusTcpMsg { msg_type, serialized_msg }
    }
    fn get_type(&self) -> TcpMsgType { self.msg_type }
    fn get_serialized_msg(&self) -> &str { &self.serialized_msg }
}
impl fmt::Display for TypePlusTcpMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.msg_type, self.serialized_msg)
    }
}
pub trait TcpMessage {
    fn get_header(&self) -> &TcpMsgHeader;
    fn get_payload(&self) -> &dyn TcpMsgPayload;
    fn get_msg_type(&self) -> TcpMsgType;
    fn is_rootward(&self) -> bool {
        match self.get_header().get_direction() {
            TcpMsgDirection::Rootward => true,
            TcpMsgDirection::Leafward => false
        }
    }
    fn is_leafward(&self) -> bool { !self.is_rootward() }
    fn is_ait(&self) -> bool { self.get_header().get_ait() }
    fn is_blocking(&self) -> bool;
    fn value(&self) -> serde_json::Value;
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.get_header().get_sender_msg_seq_no() } // Should prepend self.get_header().get_sender_id()
}
pub trait TcpMsgPayload: fmt::Display {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpMsgHeader {
    sender_msg_seq_no: SenderMsgSeqNo, // Debugging only?
    is_ait: bool,
    sender_id: SenderID, // Used to find set of AllowedTrees
    msg_type: TcpMsgType,
    direction: TcpMsgDirection,
    allowed_tree_names: Vec<AllowedTree>,
}
impl TcpMsgHeader {
    pub fn new(sender_id: SenderID, is_ait: bool, msg_type: TcpMsgType, direction: TcpMsgDirection) -> TcpMsgHeader {
        let msg_count = get_next_count();
        TcpMsgHeader { sender_id: sender_id.clone(), is_ait, msg_type, direction, sender_msg_seq_no: msg_count, allowed_tree_names: Vec::<AllowedTree>::new() }
    }
    pub fn get_msg_type(&self) -> TcpMsgType { self.msg_type }
    pub fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
    pub fn get_ait(&self) -> bool { self.is_ait }
    pub fn get_direction(&self) -> TcpMsgDirection { self.direction }
    pub fn get_sender_id(&self) -> &SenderID { &self.sender_id }
    pub fn get_allowed_tree_names(&self) -> &Vec<AllowedTree> { &self.allowed_tree_names }
    pub fn set_allowed_tree_names(&mut self, allowed_tree_names: Vec<AllowedTree>) { self.allowed_tree_names = allowed_tree_names; } // Should this be set in new()?
    //pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for TcpMsgHeader { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        let s = format!("Message {} {} '{}'", *self.sender_msg_seq_no, self.msg_type, self.direction);
        write!(f, "{}", s) 
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpApplicationMsg {
    header: TcpMsgHeader,
    payload: TcpApplicationMsgPayload
}
impl TcpApplicationMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, allowed_tree_name: &AllowedTree, direction: TcpMsgDirection, body: &str) -> TcpApplicationMsg {
        let header = TcpMsgHeader::new(sender_id, is_ait, TcpMsgType::Application, direction);
        let payload = TcpApplicationMsgPayload::new(&allowed_tree_name, body);
        TcpApplicationMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TcpApplicationMsgPayload { &self.payload }
    pub fn get_allowed_tree_name(&self) -> &AllowedTree { self.payload.get_allowed_tree_name() }
}
impl TcpMessage for TcpApplicationMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpApplicationMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpApplicationMsgPayload {
    allowed_tree_name: AllowedTree,
    body: ByteArray,
}
impl TcpApplicationMsgPayload {
    fn new(allowed_tree_name: &AllowedTree, body: &str) -> TcpApplicationMsgPayload {
        TcpApplicationMsgPayload { allowed_tree_name: allowed_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_allowed_tree_name(&self) -> &AllowedTree { &self.allowed_tree_name }
}
impl TcpMsgPayload for TcpApplicationMsgPayload {}
impl fmt::Display for TcpApplicationMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(body) = ::std::str::from_utf8(&self.body) {
            let s = format!("Application message {}", body);
            write!(f, "{}", s)
        } else {
            write!(f, "Error converting application message body from bytes to string")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpDeleteTreeMsg {
    header: TcpMsgHeader,
    payload: TcpDeleteTreeMsgPayload
}
impl TcpDeleteTreeMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, delete_tree_name: &AllowedTree, direction: TcpMsgDirection, body: &str) -> TcpDeleteTreeMsg {
        let header = TcpMsgHeader::new(sender_id, is_ait, TcpMsgType::DeleteTree, direction);
        let payload = TcpDeleteTreeMsgPayload::new(&delete_tree_name, body);
        TcpDeleteTreeMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TcpDeleteTreeMsgPayload { &self.payload }
    pub fn get_delete_tree_name(&self) -> &AllowedTree { self.payload.get_delete_tree_name() }
}
impl TcpMessage for TcpDeleteTreeMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpDeleteTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpDeleteTreeMsgPayload {
    delete_tree_name: AllowedTree,
    body: ByteArray,
}
impl TcpDeleteTreeMsgPayload {
    fn new(delete_tree_name: &AllowedTree, body: &str) -> TcpDeleteTreeMsgPayload {
        TcpDeleteTreeMsgPayload { delete_tree_name: delete_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_delete_tree_name(&self) -> &AllowedTree { &self.delete_tree_name }
}
impl TcpMsgPayload for TcpDeleteTreeMsgPayload {}
impl fmt::Display for TcpDeleteTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(body) = ::std::str::from_utf8(&self.body) {
            let s = format!("DeleteTree message {}", body);
            write!(f, "{}", s)
        } else {
            write!(f, "Error converting application message body from bytes to string")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpManifestMsg {
    header: TcpMsgHeader,
    payload: TcpManifestMsgPayload
}
impl TcpManifestMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, deploy_tree_name: &AllowedTree, allowed_tree_names: &Vec<AllowedTree>, manifest: &Manifest) -> TcpManifestMsg {
        // Note that direction is leafward so cell agent will get the message
        let mut header = TcpMsgHeader::new(sender_id, is_ait, TcpMsgType::Manifest, TcpMsgDirection::Leafward);
        header.set_allowed_tree_names(allowed_tree_names.clone());
        let payload = TcpManifestMsgPayload::new(&deploy_tree_name, &manifest);
        TcpManifestMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TcpManifestMsgPayload { &self.payload }
    pub fn get_deploy_tree_name(&self) -> &AllowedTree { self.payload.get_deploy_tree_name() }
}
impl TcpMessage for TcpManifestMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpManifestMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpManifestMsgPayload {
    deploy_tree_name: AllowedTree,
    tree_name: AllowedTree,
    manifest: Manifest 
}
impl TcpManifestMsgPayload {
    fn new(deploy_tree_name: &AllowedTree, manifest: &Manifest) -> TcpManifestMsgPayload {
        let tree_name = manifest.get_deployment_tree();
        TcpManifestMsgPayload { deploy_tree_name: deploy_tree_name.clone(), tree_name: tree_name.clone(),
            manifest: manifest.clone() }
    }
    pub fn get_manifest(&self) -> &Manifest { &self.manifest }
    pub fn get_deploy_tree_name(&self) -> &AllowedTree { &self.deploy_tree_name }
}
impl TcpMsgPayload for TcpManifestMsgPayload {}
impl fmt::Display for TcpManifestMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Manifest: {}", self.get_manifest());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpQueryMsg {
    header: TcpMsgHeader,
    payload: TcpQueryMsgPayload
}
impl TcpQueryMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, query_tree_name: &AllowedTree, direction: TcpMsgDirection, body: &str) -> TcpQueryMsg {
        let header = TcpMsgHeader::new(sender_id, is_ait, TcpMsgType::Query, direction);
        let payload = TcpQueryMsgPayload::new(&query_tree_name, body);
        TcpQueryMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TcpQueryMsgPayload { &self.payload }
    pub fn get_query_tree_name(&self) -> &AllowedTree { self.payload.get_query_tree_name() }
}
impl TcpMessage for TcpQueryMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpQueryMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpQueryMsgPayload {
    query_tree_name: AllowedTree,
    body: ByteArray,
}
impl TcpQueryMsgPayload {
    fn new(query_tree_name: &AllowedTree, body: &str) -> TcpQueryMsgPayload {
        TcpQueryMsgPayload { query_tree_name: query_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_query_tree_name(&self) -> &AllowedTree { &self.query_tree_name }
}
impl TcpMsgPayload for TcpQueryMsgPayload {}
impl fmt::Display for TcpQueryMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(body) = ::std::str::from_utf8(&self.body) {
            let s = format!("Query message {}", body);
            write!(f, "{}", s)
        } else {
            write!(f, "Error converting application message body from bytes to string")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpStackTreeMsg {
    header: TcpMsgHeader,
    payload: TcpStackTreeMsgPayload
}
impl TcpStackTreeMsg {
    pub fn new(sender_id: SenderID, new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree,
               direction: TcpMsgDirection, gvm_eqn: &GvmEquation) -> TcpStackTreeMsg {
        let header = TcpMsgHeader::new( sender_id, true,TcpMsgType::StackTree, direction);
        let payload = TcpStackTreeMsgPayload::new(new_tree_name, parent_tree_name, gvm_eqn);
        TcpStackTreeMsg { header, payload}
    }
    pub fn get_payload(&self) -> &TcpStackTreeMsgPayload { &self.payload }
}
impl TcpMessage for TcpStackTreeMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.header.msg_type }
    fn is_blocking(&self) -> bool { true }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpStackTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpStackTreeMsgPayload {
    new_tree_name: AllowedTree,
    parent_tree_name: AllowedTree,
    gvm_eqn: GvmEquation
}
impl TcpStackTreeMsgPayload {
    fn new(new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree, gvm_eqn: &GvmEquation) -> TcpStackTreeMsgPayload {
        TcpStackTreeMsgPayload { new_tree_name: new_tree_name.clone(), parent_tree_name: parent_tree_name.clone(),
            gvm_eqn: gvm_eqn.clone() }
    }
    pub fn get_new_tree_name(&self) -> &AllowedTree { &self.new_tree_name }
    pub fn get_parent_tree_name(&self) -> &AllowedTree { &self.parent_tree_name }
    pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
}
impl TcpMsgPayload for TcpStackTreeMsgPayload {}
impl fmt::Display for TcpStackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree {} stacked on tree {} {}", self.new_tree_name, self.parent_tree_name, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTreeNameMsg {
    header: TcpMsgHeader,
    payload: TcpTreeNameMsgPayload
}
impl TcpTreeNameMsg {
    pub fn new(sender_id: SenderID, tree_name: &str) -> TcpTreeNameMsg {
        // Note that direction is rootward so cell agent will get the message
        let header = TcpMsgHeader::new(sender_id, false,TcpMsgType::TreeName, TcpMsgDirection::Rootward);
        let payload = TcpTreeNameMsgPayload::new(tree_name);
        TcpTreeNameMsg { header, payload }
    }
    pub fn get_payload(&self) -> &TcpTreeNameMsgPayload { &self.payload }
    pub fn get_tree_name(&self) -> &String { self.payload.get_tree_name() }
}
impl TcpMessage for TcpTreeNameMsg {
    fn get_header(&self) -> &TcpMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn TcpMsgPayload { &self.payload }
    fn get_msg_type(&self) -> TcpMsgType { self.get_header().msg_type }
    fn is_blocking(&self) -> bool { false }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for TcpTreeNameMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TcpTreeNameMsgPayload {
    tree_name: String,
}
impl TcpTreeNameMsgPayload {
    fn new(tree_name: &str) -> TcpTreeNameMsgPayload {
        TcpTreeNameMsgPayload { tree_name: S(tree_name) }
    }
    fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl TcpMsgPayload for TcpTreeNameMsgPayload {}
impl fmt::Display for TcpTreeNameMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Tree name for border cell {}", self.tree_name);
        write!(f, "{}", s)
    }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum TcpMessageError {
    #[fail(display = "TcpMessageError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "TcpMessageError::Gvm {}: No GVM for this message type {}", func_name, msg_type)]
//    Gvm { func_name: &'static str, msg_type: TcpMsgType },
//    #[fail(display = "TcpMessageError::InvalidTcpMsgType {}: Invalid message type {} from packet assembler", func_name, msg_type)]
//    InvalidTcpMsgType { func_name: &'static str, msg_type: TcpMsgType },
//    #[fail(display = "TcpMessageError::Message {}: Message error from {}", func_name, handler)]
//    Message { func_name: &'static str, handler: &'static str },
//    #[fail(display = "TcpMessageError::NoGmv {}: No GVM in StackTreeMsg", func_name)]
//    NoGvm { func_name: &'static str },
//    #[fail(display = "TcpMessageError::Payload {}: Wrong payload for type {}", func_name, msg_type)]
//    Payload { func_name: &'static str, msg_type: TcpMsgType },
    #[fail(display = "TcpMessageError::Process {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
//    #[fail(display = "TcpMessageError::TreeID {}: No TreeID ", func_name, msg_type: TcpMsgType)]
//    TreeID { func_name: &'static str, msg_type: TcpMsgType },
//    #[fail(display = "TcpMessageError::TreeMapEntry {}: No tree named {} in map", func_name, tree_name)]
//    TreeMapEntry { func_name: &'static str, tree_name: String }
}
