use std::{fmt,
          ops::{Deref},
          sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering},
};

use serde_json;

use crate::config::{ByteArray};
use crate::gvm_equation::{GvmEquation};
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
pub enum AppMsgType {
    Interapplication,
    DeleteTree,
    Manifest,
    Query,
    StackTree,
    TreeName,
}
impl fmt::Display for AppMsgType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            AppMsgType::Interapplication  => "Interapplication",
            AppMsgType::DeleteTree   => "DeleteTree",
            AppMsgType::Manifest     => "Manifest",
            AppMsgType::Query        => "Query",
            AppMsgType::StackTree    => "StackTree",
            AppMsgType::TreeName     => "TreeName",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum AppMsgDirection {
    Rootward,
    Leafward
}
impl fmt::Display for AppMsgDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            AppMsgDirection::Rootward => "Rootward",
            AppMsgDirection::Leafward => "Leafward"
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct TypePlusAppMsg {
    msg_type: AppMsgType,
    serialized_msg: String
}
impl TypePlusAppMsg {
    pub fn _new(msg_type: AppMsgType, serialized_msg: String) -> TypePlusAppMsg {
        TypePlusAppMsg { msg_type, serialized_msg }
    }
    fn _et_type(&self) -> AppMsgType { self.msg_type }
    fn _get_serialized_msg(&self) -> &str { &self.serialized_msg }
}
impl fmt::Display for TypePlusAppMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.msg_type, self.serialized_msg)
    }
}
pub trait AppMessage {
    fn get_header(&self) -> &AppMsgHeader;
    fn get_payload(&self) -> &dyn AppMsgPayload;
    fn get_msg_type(&self) -> AppMsgType;
    fn is_rootward(&self) -> bool {
        match self.get_header().get_direction() {
            AppMsgDirection::Rootward => true,
            AppMsgDirection::Leafward => false
        }
    }
    fn is_leafward(&self) -> bool { !self.is_rootward() }
    fn is_ait(&self) -> bool { self.get_header().get_ait() }
    fn value(&self) -> serde_json::Value;
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.get_header().get_sender_msg_seq_no() } // Should prepend self.get_header().get_sender_id()
}
pub trait AppMsgPayload: fmt::Display {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMsgHeader {
    sender_msg_seq_no: SenderMsgSeqNo, // Debugging only?
    is_ait: bool,
    sender_id: SenderID, // Used to find set of AllowedTrees
    msg_type: AppMsgType,
    direction: AppMsgDirection,
    allowed_tree_names: Vec<AllowedTree>,
}
impl AppMsgHeader {
    pub fn new(sender_id: SenderID, is_ait: bool, msg_type: AppMsgType, direction: AppMsgDirection) -> AppMsgHeader {
        let msg_count = get_next_count();
        AppMsgHeader { sender_id: sender_id.clone(), is_ait, msg_type, direction, sender_msg_seq_no: msg_count, allowed_tree_names: Vec::<AllowedTree>::new() }
    }
    pub fn _get_msg_type(&self) -> AppMsgType { self.msg_type }
    pub fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
    pub fn get_ait(&self) -> bool { self.is_ait }
    pub fn get_direction(&self) -> AppMsgDirection { self.direction }
    pub fn _get_sender_id(&self) -> &SenderID { &self.sender_id }
    pub fn _get_allowed_tree_names(&self) -> &Vec<AllowedTree> { &self.allowed_tree_names }
    pub fn set_allowed_tree_names(&mut self, allowed_tree_names: Vec<AllowedTree>) { self.allowed_tree_names = allowed_tree_names; } // Should this be set in new()?
    //pub fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction; }
}
impl fmt::Display for AppMsgHeader { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        let s = format!("Message {} {} '{}'", *self.sender_msg_seq_no, self.msg_type, self.direction);
        write!(f, "{}", s) 
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInterapplicationMsg {
    header: AppMsgHeader,
    payload: AppInterapplicationMsgPayload
}
impl AppInterapplicationMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, allowed_tree_name: &AllowedTree, direction: AppMsgDirection, body: &str) -> AppInterapplicationMsg {
        let header = AppMsgHeader::new(sender_id, is_ait, AppMsgType::Interapplication, direction);
        let payload = AppInterapplicationMsgPayload::new(&allowed_tree_name, body);
        AppInterapplicationMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppInterapplicationMsgPayload { &self.payload }
    pub fn get_allowed_tree_name(&self) -> &AllowedTree { self.payload.get_allowed_tree_name() }
}
impl AppMessage for AppInterapplicationMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppInterapplicationMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppInterapplicationMsgPayload {
    allowed_tree_name: AllowedTree,
    body: ByteArray,
}
impl AppInterapplicationMsgPayload {
    fn new(allowed_tree_name: &AllowedTree, body: &str) -> AppInterapplicationMsgPayload {
        AppInterapplicationMsgPayload { allowed_tree_name: allowed_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_allowed_tree_name(&self) -> &AllowedTree { &self.allowed_tree_name }
}
impl AppMsgPayload for AppInterapplicationMsgPayload {}
impl fmt::Display for AppInterapplicationMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(body) = ::std::str::from_utf8(&self.body) {
            let s = format!("Interapplication message {}", body);
            write!(f, "{}", s)
        } else {
            write!(f, "Error converting application message body from bytes to string")
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDeleteTreeMsg {
    header: AppMsgHeader,
    payload: AppDeleteTreeMsgPayload
}
impl AppDeleteTreeMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, delete_tree_name: &AllowedTree, direction: AppMsgDirection, body: &str) -> AppDeleteTreeMsg {
        let header = AppMsgHeader::new(sender_id, is_ait, AppMsgType::DeleteTree, direction);
        let payload = AppDeleteTreeMsgPayload::new(&delete_tree_name, body);
        AppDeleteTreeMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppDeleteTreeMsgPayload { &self.payload }
    pub fn get_delete_tree_name(&self) -> &AllowedTree { self.payload.get_delete_tree_name() }
}
impl AppMessage for AppDeleteTreeMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppDeleteTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppDeleteTreeMsgPayload {
    delete_tree_name: AllowedTree,
    body: ByteArray,
}
impl AppDeleteTreeMsgPayload {
    fn new(delete_tree_name: &AllowedTree, body: &str) -> AppDeleteTreeMsgPayload {
        AppDeleteTreeMsgPayload { delete_tree_name: delete_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_delete_tree_name(&self) -> &AllowedTree { &self.delete_tree_name }
}
impl AppMsgPayload for AppDeleteTreeMsgPayload {}
impl fmt::Display for AppDeleteTreeMsgPayload {
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
pub struct AppManifestMsg {
    header: AppMsgHeader,
    payload: AppManifestMsgPayload
}
impl AppManifestMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, deploy_tree_name: &AllowedTree, allowed_tree_names: &Vec<AllowedTree>, manifest: &Manifest) -> AppManifestMsg {
        // Note that direction is leafward so cell agent will get the message
        let mut header = AppMsgHeader::new(sender_id, is_ait, AppMsgType::Manifest, AppMsgDirection::Leafward);
        header.set_allowed_tree_names(allowed_tree_names.clone());
        let payload = AppManifestMsgPayload::new(&deploy_tree_name, &manifest);
        AppManifestMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppManifestMsgPayload { &self.payload }
    pub fn get_deploy_tree_name(&self) -> &AllowedTree { self.payload.get_deploy_tree_name() }
}
impl AppMessage for AppManifestMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppManifestMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppManifestMsgPayload {
    deploy_tree_name: AllowedTree,
    tree_name: AllowedTree,
    manifest: Manifest 
}
impl AppManifestMsgPayload {
    fn new(deploy_tree_name: &AllowedTree, manifest: &Manifest) -> AppManifestMsgPayload {
        let tree_name = manifest.get_deployment_tree();
        AppManifestMsgPayload { deploy_tree_name: deploy_tree_name.clone(), tree_name: tree_name.clone(),
            manifest: manifest.clone() }
    }
    pub fn get_manifest(&self) -> &Manifest { &self.manifest }
    pub fn get_deploy_tree_name(&self) -> &AllowedTree { &self.deploy_tree_name }
}
impl AppMsgPayload for AppManifestMsgPayload {}
impl fmt::Display for AppManifestMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Manifest: {}", self.get_manifest());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppQueryMsg {
    header: AppMsgHeader,
    payload: AppQueryMsgPayload
}
impl AppQueryMsg {
    pub fn new(sender_id: SenderID, is_ait: bool, query_tree_name: &AllowedTree, direction: AppMsgDirection, body: &str) -> AppQueryMsg {
        let header = AppMsgHeader::new(sender_id, is_ait, AppMsgType::Query, direction);
        let payload = AppQueryMsgPayload::new(&query_tree_name, body);
        AppQueryMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppQueryMsgPayload { &self.payload }
    pub fn get_query_tree_name(&self) -> &AllowedTree { self.payload.get_query_tree_name() }
}
impl AppMessage for AppQueryMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppQueryMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppQueryMsgPayload {
    query_tree_name: AllowedTree,
    body: ByteArray,
}
impl AppQueryMsgPayload {
    fn new(query_tree_name: &AllowedTree, body: &str) -> AppQueryMsgPayload {
        AppQueryMsgPayload { query_tree_name: query_tree_name.clone(), body: ByteArray(S(body).into_bytes()) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
    pub fn get_query_tree_name(&self) -> &AllowedTree { &self.query_tree_name }
}
impl AppMsgPayload for AppQueryMsgPayload {}
impl fmt::Display for AppQueryMsgPayload {
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
pub struct AppStackTreeMsg {
    header: AppMsgHeader,
    payload: AppStackTreeMsgPayload
}
impl AppStackTreeMsg {
    pub fn new(sender_id: SenderID, new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree,
               direction: AppMsgDirection, gvm_eqn: &GvmEquation) -> AppStackTreeMsg {
        let header = AppMsgHeader::new( sender_id, true,AppMsgType::StackTree, direction);
        let payload = AppStackTreeMsgPayload::new(new_tree_name, parent_tree_name, gvm_eqn);
        AppStackTreeMsg { header, payload}
    }
    pub fn get_payload(&self) -> &AppStackTreeMsgPayload { &self.payload }
}
impl AppMessage for AppStackTreeMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.header.msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppStackTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppStackTreeMsgPayload {
    new_tree_name: AllowedTree,
    parent_tree_name: AllowedTree,
    gvm_eqn: GvmEquation
}
impl AppStackTreeMsgPayload {
    fn new(new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree, gvm_eqn: &GvmEquation) -> AppStackTreeMsgPayload {
        AppStackTreeMsgPayload { new_tree_name: new_tree_name.clone(), parent_tree_name: parent_tree_name.clone(),
            gvm_eqn: gvm_eqn.clone() }
    }
    pub fn get_new_tree_name(&self) -> &AllowedTree { &self.new_tree_name }
    pub fn get_parent_tree_name(&self) -> &AllowedTree { &self.parent_tree_name }
    pub fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
}
impl AppMsgPayload for AppStackTreeMsgPayload {}
impl fmt::Display for AppStackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree {} stacked on tree {} {}", self.new_tree_name, self.parent_tree_name, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTreeNameMsg {
    header: AppMsgHeader,
    payload: AppTreeNameMsgPayload
}
impl AppTreeNameMsg {
    pub fn new(sender_id: SenderID, tree_name: &str) -> AppTreeNameMsg {
        // Note that direction is rootward so cell agent will get the message
        let header = AppMsgHeader::new(sender_id, false,AppMsgType::TreeName, AppMsgDirection::Rootward);
        let payload = AppTreeNameMsgPayload::new(tree_name);
        AppTreeNameMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppTreeNameMsgPayload { &self.payload }
    pub fn get_tree_name(&self) -> &String { self.payload.get_tree_name() }
}
impl AppMessage for AppTreeNameMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
}
impl fmt::Display for AppTreeNameMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppTreeNameMsgPayload {
    tree_name: String,
}
impl AppTreeNameMsgPayload {
    fn new(tree_name: &str) -> AppTreeNameMsgPayload {
        AppTreeNameMsgPayload { tree_name: S(tree_name) }
    }
    fn get_tree_name(&self) -> &String { &self.tree_name }
}
impl AppMsgPayload for AppTreeNameMsgPayload {}
impl fmt::Display for AppTreeNameMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Tree name for border cell {}", self.tree_name);
        write!(f, "{}", s)
    }
}

// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum AppMessageError {
    #[fail(display = "AppMessageError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "AppMessageError::Gvm {}: No GVM for this message type {}", func_name, msg_type)]
//    Gvm { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "AppMessageError::InvalidAppMsgType {}: Invalid message type {} from packet assembler", func_name, msg_type)]
//    InvalidAppMsgType { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "AppMessageError::Message {}: Message error from {}", func_name, handler)]
//    Message { func_name: &'static str, handler: &'static str },
//    #[fail(display = "AppMessageError::NoGmv {}: No GVM in StackTreeMsg", func_name)]
//    NoGvm { func_name: &'static str },
//    #[fail(display = "AppMessageError::Payload {}: Wrong payload for type {}", func_name, msg_type)]
//    Payload { func_name: &'static str, msg_type: AppMsgType },
    #[fail(display = "AppMessageError::Process {}: Wrong message process function called", func_name)]
    Process { func_name: &'static str },
//    #[fail(display = "AppMessageError::TreeID {}: No TreeID ", func_name, msg_type: AppMsgType)]
//    TreeID { func_name: &'static str, msg_type: AppMsgType },
//    #[fail(display = "AppMessageError::TreeMapEntry {}: No tree named {} in map", func_name, tree_name)]
//    TreeMapEntry { func_name: &'static str, tree_name: String }
}
