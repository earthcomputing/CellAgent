use std::{
    collections::{
        HashMap,
    },
    sync::{Mutex}
};

use actix_web::{web, Error, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hello::AppCells;

type Size = usize;

fn show_black_tree() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().body("black_tree".to_owned()))
}
fn process_discoverd(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("DiscoverDMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    if body.msg.payload.discoverd_type == "First" {
        let this_cell_name = body.cell_id.name;
        let recv_port = body.port_no;
        let tree_name = body.msg.payload.port_tree_id.name;
    }
    Ok(HttpResponse::Ok().body("process_discoverd".to_owned()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Body {
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
    cell_id:CellID,         // Sending cell
    discoverd_type: String, // First or Subsequent
    port_no: usize,         // Sending cell's port
    port_tree_id: TreeID
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
struct CellID {
    name: String
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
struct TreeID {
    name: String
}

pub fn get() -> Scope {
    web::scope("/black_tree")
        .data(web::Data::new(AppBlackTree::default()))
        .route("", web::get().to(show_black_tree))
}
pub fn post() -> Scope {
    web::scope("/ca_process_discoverd_msg")
        .data(web::Data::new(AppBlackTree::default()))
        .route("", web::post().to(process_discoverd))
}
#[derive(Debug, Default, Serialize)]
pub struct AppBlackTree {
    neighbors: Mutex<HashMap<String,String>>
}
pub fn data() -> web::Data<AppBlackTree> {
    web::Data::new(AppBlackTree::default())
}
