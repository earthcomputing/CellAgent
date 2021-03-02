use std::{fmt,
          ops::{Deref},
          sync::atomic::{AtomicU64, Ordering},
};

use serde_json;

use crate::cellagent::CellAgent;
use crate::gvm_equation::{GvmEquation};
use crate::name::{OriginatorID};
use crate::noc::Noc;
use crate::uptree_spec::{AllowedTree, Manifest};
use crate::utility::{ByteArray, S};

// This is currently at the cell level, but could be placed at the up-tree level.
#[derive(Debug, Copy, Clone, Default, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SenderMsgSeqNo(pub u64);
impl Deref for SenderMsgSeqNo { type Target = u64; fn deref(&self) -> &Self::Target { &self.0 } }
static MESSAGE_COUNT: AtomicU64 = AtomicU64::new(0);
pub fn get_next_count() -> SenderMsgSeqNo { SenderMsgSeqNo(MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst)) }

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppMsgType { // Make sure these match the struct names
    AppInterapplicationMsg,
    AppDeleteTreeMsg,
    AppManifestMsg,
    AppQueryMsg,
    AppStackTreeMsg,
    AppTreeNameMsg,
}
impl AppMsgType {
    pub fn app_msg_from_bytes(bytes: &ByteArray) -> Result<Box<dyn AppMessage>, Error> {
        let _f = "app_msg_from_bytes";
        let serialized = bytes.stringify()?;
        let msg = serde_json::from_str(&serialized)?;
        Ok(msg)
    }
}
impl fmt::Display for AppMsgType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            AppMsgType::AppInterapplicationMsg => "AppInterapplication",
            AppMsgType::AppDeleteTreeMsg       => "AppDeleteTree",
            AppMsgType::AppManifestMsg         => "AppManifest",
            AppMsgType::AppQueryMsg            => "AppQuery",
            AppMsgType::AppStackTreeMsg        => "AppStackTree",
            AppMsgType::AppTreeNameMsg         => "AppTreeName",
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
#[typetag::serde(tag = "app_msg_type")]
pub trait AppMessage: fmt::Display {
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
    fn is_ait(&self) -> bool { self.get_header().is_ait() }
    fn is_snake(&self) -> bool { self.get_header().is_snake() }
    fn get_target_tree_name(&self) -> &AllowedTree { self.get_header().get_target_tree_name() }
    fn value(&self) -> serde_json::Value;
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.get_header().get_sender_msg_seq_no() } // Should prepend self.get_header().get_sender_id()
    fn get_sender_name(&self) -> &str { &self.get_header().get_sender_name() }
    fn get_direction(&self) -> AppMsgDirection { self.get_header().get_direction() }
    fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.get_header().get_allowed_trees() }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error>;
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error>;
}
#[typetag::serde(tag = "app_msg_payload_type")]
pub trait AppMsgPayload: fmt::Display {}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppMsgHeader {
    sender_msg_seq_no: SenderMsgSeqNo, // Debugging only?
    target_tree: AllowedTree,
    sender_name: String,
    msg_type: AppMsgType,
    is_ait: bool,
    is_snake: bool,
    direction: AppMsgDirection,
    allowed_trees: Vec<AllowedTree>, // Trees named in message body
}
impl AppMsgHeader {
    pub fn new(sender_name: &str, target_tree: &AllowedTree, is_ait: bool, is_snake: bool,
               msg_type: AppMsgType, direction: AppMsgDirection, allowed_trees: &Vec<AllowedTree>)
            -> AppMsgHeader {
        let msg_count = get_next_count();
        AppMsgHeader { sender_msg_seq_no: msg_count, target_tree: target_tree.clone(),
            sender_name: S(sender_name), msg_type, is_ait, is_snake, direction, allowed_trees: allowed_trees.clone() }
    }
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
    fn get_target_tree_name(&self) -> &AllowedTree { &self.target_tree }
    fn get_sender_name(&self) -> &str { &self.sender_name }
    fn _get_msg_type(&self) -> AppMsgType { self.msg_type }
    fn is_ait(&self) -> bool { self.is_ait }
    fn is_snake(&self) -> bool { self.is_snake }
    fn get_direction(&self) -> AppMsgDirection { self.direction }
    fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
}
impl fmt::Display for AppMsgHeader { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        let s = format!("Message on tree {} seq no {} sender '{}' type {} {} is_ait {}", self.target_tree,
                        *self.sender_msg_seq_no, self.sender_name, self.msg_type,
                        self.direction, self.is_ait);
        write!(f, "{}", s) 
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppInterapplicationMsg {
    header: AppMsgHeader,
    payload: AppInterapplicationMsgPayload
}
impl AppInterapplicationMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool, target_tree: &AllowedTree, direction: AppMsgDirection,
               allowed_trees: &Vec<AllowedTree>, body: &str) -> AppInterapplicationMsg {
        let msg_type = AppMsgType::AppInterapplicationMsg;
        let header = AppMsgHeader::new(sender_name, target_tree, is_ait, is_snake,
                                       msg_type, direction, allowed_trees);
        let payload = AppInterapplicationMsgPayload::new(body);
        AppInterapplicationMsg { header, payload }
    }
}
#[typetag::serde]
impl AppMessage for AppInterapplicationMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        let _f = "process_ca";
        cell_agent.app_interapplication(self, sender_id).context(AppMessageError::Chain { func_name: _f, comment: S("") })?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_interapplication(self, noc_to_port)?;
        Ok(())
    }
}
impl fmt::Display for AppInterapplicationMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: {}", self.header, self.payload);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppInterapplicationMsgPayload {
    body: ByteArray,
}
impl AppInterapplicationMsgPayload {
    fn new(body: &str) -> AppInterapplicationMsgPayload {
        AppInterapplicationMsgPayload { body: ByteArray::new(body) }
    }
    pub fn get_body(&self) -> &ByteArray { &self.body }
}
#[typetag::serde]
impl AppMsgPayload for AppInterapplicationMsgPayload {}
impl fmt::Display for AppInterapplicationMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = self.body.stringify().expect("Error converting bytes to string in AppInterapplicationMsg fmt::Display");
        write!(f, "{}", body)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDeleteTreeMsg {
    header: AppMsgHeader,
    payload: AppDeleteTreeMsgPayload
}
impl AppDeleteTreeMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool, target_tree: &AllowedTree, direction: AppMsgDirection)
            -> AppDeleteTreeMsg {
        let allowed_trees = &vec![target_tree.clone()];
        let msg_type = AppMsgType::AppDeleteTreeMsg;
        let header = AppMsgHeader::new(sender_name, target_tree, is_ait, is_snake,
                                       msg_type, direction, allowed_trees);
        let payload = AppDeleteTreeMsgPayload::new();
        AppDeleteTreeMsg { header, payload }
    }
    pub fn get_delete_tree_name(&self) -> &AllowedTree { self.header.get_target_tree_name() }
}
#[typetag::serde]
impl AppMessage for AppDeleteTreeMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        cell_agent.app_delete_tree(self, sender_id)?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_delete_tree(self, noc_to_port)?;
        Ok(())
    }
}
impl fmt::Display for AppDeleteTreeMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{}: tree {}", self.get_header(), self.get_payload());
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct AppDeleteTreeMsgPayload {}
impl AppDeleteTreeMsgPayload {
    fn new() -> AppDeleteTreeMsgPayload {
        AppDeleteTreeMsgPayload {}
    }
}
#[typetag::serde]
impl AppMsgPayload for AppDeleteTreeMsgPayload {}
impl fmt::Display for AppDeleteTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "")
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifestMsg {
    header: AppMsgHeader,
    payload: AppManifestMsgPayload
}
impl AppManifestMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool, deploy_tree_name: &AllowedTree, manifest: &Manifest,
               allowed_trees: &Vec<AllowedTree>) -> AppManifestMsg {
        // Note that direction is leafward so cell agent will get the message
        let msg_type = AppMsgType::AppManifestMsg;
        let header = AppMsgHeader::new(sender_name, deploy_tree_name,
                                           is_ait, is_snake, msg_type,
                                           AppMsgDirection::Leafward, allowed_trees);
        let payload = AppManifestMsgPayload::new(manifest);
        AppManifestMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppManifestMsgPayload { &self.payload }
    pub fn get_deploy_tree_name(&self) -> &AllowedTree { self.header.get_target_tree_name() }
}
#[typetag::serde]
impl AppMessage for AppManifestMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        cell_agent.app_manifest(self, sender_id)?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_manifest(self, noc_to_port)?;
        Ok(())
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
    manifest: Manifest
}
impl AppManifestMsgPayload {
    fn new(manifest: &Manifest) -> AppManifestMsgPayload {
        AppManifestMsgPayload { manifest: manifest.clone() }
    }
    pub fn get_manifest(&self) -> &Manifest { &self.manifest }
}
#[typetag::serde]
impl AppMsgPayload for AppManifestMsgPayload {}
impl fmt::Display for AppManifestMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Manifest: {}", self.manifest);
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppQueryMsg {
    header: AppMsgHeader,
    payload: AppQueryMsgPayload
}
impl AppQueryMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool, query_tree_name: &AllowedTree, query: &str,
               direction: AppMsgDirection, query_trees: &Vec<AllowedTree>) -> AppQueryMsg {
        let msg_type = AppMsgType::AppQueryMsg;
        let header = AppMsgHeader::new(sender_name, query_tree_name,
                                       is_ait, is_snake, msg_type, direction,
                                       query_trees);
        let payload = AppQueryMsgPayload::new(query_trees, query);
        AppQueryMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppQueryMsgPayload { &self.payload }
    pub fn get_query(&self) -> &str { self.payload.get_query() }
}
#[typetag::serde]
impl AppMessage for AppQueryMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        cell_agent.app_query(self, sender_id)?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_query(self, noc_to_port)?;
        Ok(())
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
    query: String,
}
impl AppQueryMsgPayload {
    fn new(_query_tree_name: &Vec<AllowedTree>, query: &str) -> AppQueryMsgPayload {
        AppQueryMsgPayload { query: S(query) }
    }
    pub fn get_query(&self) -> &str { &self.query }
}
#[typetag::serde]
impl AppMsgPayload for AppQueryMsgPayload {}
impl fmt::Display for AppQueryMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Query: {}", self.query)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStackTreeMsg {
    header: AppMsgHeader,
    payload: AppStackTreeMsgPayload
}
impl AppStackTreeMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool,
                new_tree_name: &AllowedTree, parent_tree_name: &AllowedTree,
               direction: AppMsgDirection, gvm_eqn: &GvmEquation) -> AppStackTreeMsg {
        let msg_type = AppMsgType::AppStackTreeMsg;
        let header = AppMsgHeader::new(sender_name, parent_tree_name,
                                       is_ait, is_snake, msg_type,
                                       direction, &Vec::new());
        let payload = AppStackTreeMsgPayload::new(new_tree_name, gvm_eqn);
        AppStackTreeMsg { header, payload}
    }
    pub fn get_payload(&self) -> &AppStackTreeMsgPayload { &self.payload }
    pub fn get_new_tree_name(&self) -> &AllowedTree { &self.payload.get_new_tree_name() }
    pub fn get_parent_tree_name(&self) -> &AllowedTree { &self.header.get_target_tree_name() }
    pub fn get_gvm(&self) -> &GvmEquation { &self.payload.get_gvm_eqn() }
}
#[typetag::serde]
impl AppMessage for AppStackTreeMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.header.msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        cell_agent.app_stack_tree(self, sender_id)?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_stack_tree(self, noc_to_port)?;
        Ok(())
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
    gvm_eqn: GvmEquation
}
impl AppStackTreeMsgPayload {
    fn new(new_tree_name: &AllowedTree, gvm_eqn: &GvmEquation) -> AppStackTreeMsgPayload {
        AppStackTreeMsgPayload { new_tree_name: new_tree_name.clone(), gvm_eqn: gvm_eqn.clone() }
    }
    fn get_new_tree_name(&self) -> &AllowedTree { &self.new_tree_name }
    fn get_gvm_eqn(&self) -> &GvmEquation { &self.gvm_eqn }
}
#[typetag::serde]
impl AppMsgPayload for AppStackTreeMsgPayload {}
impl fmt::Display for AppStackTreeMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "New tree {} with GVM {}", self.new_tree_name, self.gvm_eqn)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTreeNameMsg {
    header: AppMsgHeader,
    payload: AppTreeNameMsgPayload
}
impl AppTreeNameMsg {
    pub fn new(sender_name: &str, is_ait: bool, is_snake: bool,
            target_tree_name: &AllowedTree, tree_name: &AllowedTree)
                -> AppTreeNameMsg {
        // Note that direction is rootward so cell agent will get the message
        let allowed_trees = &vec![];
        let msg_type = AppMsgType::AppTreeNameMsg;
        let header = AppMsgHeader::new(sender_name, target_tree_name,
                                       is_ait, is_snake, msg_type,
                                       AppMsgDirection::Rootward, allowed_trees);
        let payload = AppTreeNameMsgPayload::new(tree_name);
        AppTreeNameMsg { header, payload }
    }
    pub fn get_payload(&self) -> &AppTreeNameMsgPayload { &self.payload }
    pub fn get_tree_name(&self) -> &AllowedTree { self.payload.get_tree_name() }
}
#[typetag::serde]
impl AppMessage for AppTreeNameMsg {
    fn get_header(&self) -> &AppMsgHeader { &self.header }
    fn get_payload(&self) -> &dyn AppMsgPayload { &self.payload }
    fn get_msg_type(&self) -> AppMsgType { self.get_header().msg_type }
    fn value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("I don't know how to handle errors in msg.value()")
    }
    fn process_ca(&self, cell_agent: &mut CellAgent, sender_id: OriginatorID) -> Result<(), Error> {
        cell_agent.app_tree_name(self, sender_id)?;
        Ok(())
    }
    fn process_noc(&self, noc: &mut Noc, noc_to_port: &NocToPort) -> Result<(), Error> {
        noc.app_process_tree_name(self, noc_to_port)?;
        Ok(())
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
    tree_name: AllowedTree,
}
impl AppTreeNameMsgPayload {
    fn new(tree_name: &AllowedTree) -> AppTreeNameMsgPayload {
        AppTreeNameMsgPayload { tree_name: tree_name.clone() }
    }
    fn get_tree_name(&self) -> &AllowedTree { &self.tree_name }
}
#[typetag::serde]
impl AppMsgPayload for AppTreeNameMsgPayload {}
impl fmt::Display for AppTreeNameMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Tree name {}", self.tree_name);
        write!(f, "{}", s)
    }
}

// Errors
use failure::{Error, ResultExt};
use crate::app_message_formats::NocToPort;

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
