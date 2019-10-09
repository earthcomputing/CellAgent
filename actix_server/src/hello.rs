use std::{
    collections::{
        HashMap,
    },
    sync::{Mutex}
};

use actix_web::{web, Error, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Size = usize;

fn show_topology(topology: web::Data<AppCells>) -> Result<HttpResponse, actix_web::Error> {
    let topo = topology.get_ref();
    let string = serde_json::to_string(topo)?;
    Ok(HttpResponse::Ok().body(string))
}

pub fn process_hello(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("HelloMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    process_hello_body(appcells, body)
}
pub fn process_hello_body(appcells: web::Data<AppCells>, body: Body)
        -> Result<impl Responder, Error> {
    let this_cell_id = body.cell_id.name;
    let sending_cell_id = CellID { name: body.msg.payload.cell_id.name };
    let my_port_no = body.port_no;
    let other_port_no = body.msg.payload.port_no;
    let neighbor = Neighbor { cell_name: sending_cell_id.name, port: other_port_no};
    let mut cells = appcells
        .get_ref()
        .appcells.lock().unwrap();
    let appcell = cells
        .entry(this_cell_id)
        .or_insert(Default::default());
    let neighbors = appcell.neighbors_mut();
    neighbors.neighbors.insert(my_port_no, neighbor);
    Ok(HttpResponse::Ok().body(format!("Adding hello")))
}
// Message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    cell_id: CellID, // Receiving cell
    port_no: Size,   // Receive port
    msg: EcMsg
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EcMsg {
    payload: Payload
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload {
    cell_id:CellID,  // Sending cell
    port_no: usize   // Sending cell's port
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
struct CellID {
    name: String
}
// Server data
#[derive(Debug, Default, Serialize)]
pub struct AppCells {
    pub appcells: Mutex<HashMap<String,AppCell>>
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
pub struct AppCell {
    pub neighbors: Neighbors,
    pub black_trees: Trees,
    pub stacked_trees: Trees,
}
impl AppCell {
    pub fn neighbors(&self) -> &HashMap<Size, Neighbor> { &self.neighbors.neighbors }
    pub fn neighbors_mut(&mut self) -> &mut Neighbors { &mut self.neighbors }
    pub fn black_trees(&self) -> &HashMap<String, Tree> { &self.black_trees.trees }
    pub fn black_trees_mut(&mut self) -> &mut HashMap<String, Tree> { &mut self.black_trees.trees }
    pub fn stacked_trees(&self) -> &HashMap<String, Tree> { &self.stacked_trees.trees }
    pub fn stacked_trees_mut(&mut self) -> &mut HashMap<String, Tree> { &mut self.stacked_trees.trees }
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
pub struct Neighbors {
    pub neighbors: HashMap<Size, Neighbor>
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    pub cell_name: String,
    pub port: Size
}
impl Neighbor {
    pub fn cell_name(&self) -> &String { &self.cell_name }
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
pub struct Trees {
    trees: HashMap<String, Tree>
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    tree: HashMap<usize, LinkType>
}
impl Tree {
    pub fn tree_mut(&mut self) -> &mut HashMap<usize, LinkType> { &mut self.tree }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinkType {
    Child, Parent, Pruned
}
impl Default for LinkType {
    fn default() -> LinkType { LinkType::Pruned }
}

pub fn get() -> Scope {
    web::scope("/topology")
        .data(web::Data::new(AppCells::default()))
        .route("", web::get().to(show_topology))
}
pub fn post() -> Scope {
    web::scope("/ca_process_hello_msg")
        .data(web::Data::new(AppCells::default()))
        .route("", web::post().to(process_hello))
}
pub fn data() -> web::Data<AppCells> {
    web::Data::new(AppCells::default())
}