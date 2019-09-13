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

fn show_black_tree() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().body("black_tree".to_owned()))
}
fn process_discoverd() -> Result<impl Responder, Error> {
    Ok(HttpResponse::Ok().body("process_discoverd".to_owned()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Body {
    cell_id: CellID, // Reciving cell
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
