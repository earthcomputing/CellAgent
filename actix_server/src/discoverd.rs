use std::{
    collections::{
        HashMap,
    },
    sync::{Mutex, MutexGuard}
};

use actix_web::{web, Error, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hello::{AppCell, AppCells, LinkType, Neighbor};

type Size = usize;

fn show_black_tree(appcells: web::Data<AppCells>) -> Result<HttpResponse, actix_web::Error> {
    let appcells = appcells.get_ref();
    let string = serde_json::to_string(appcells)?;
    Ok(HttpResponse::Ok().body(string))
}
fn process_discoverd(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("DiscoverDMsg: bad trace record");
    let body_opt: Result<Body, String> = match serde_json::from_value(trace_body.clone()) {
        Ok(b) => Ok(b),
        Err(e) => Err(format!("process_discoverd error {:?}", e))
    };
    let body = body_opt.unwrap();
    if body.msg.payload.discover_type == "First" {
        let this_cell_name = body.cell_id.name;
        let recv_port = body.port_no;
        let tree_name = body.msg.payload.port_tree_id.name;
        let mut cells = appcells
            .get_ref()
            .appcells.lock().unwrap();
        let other_cell = {
            update_tree(&mut cells, &this_cell_name, &tree_name, recv_port, LinkType::Child)
                .get(&recv_port).expect("DiscoverDMsg: missing neighbor")
        }.clone();
        let other_cell_name = other_cell.cell_name();
        let other_cell_port = other_cell.port;
        update_tree(&mut cells, other_cell_name, &tree_name, other_cell_port, LinkType::Parent);
    }
    Ok(HttpResponse::Ok().body("process_discoverd".to_owned()))
}
fn update_tree<'a>(cells: &'a mut MutexGuard<HashMap<String, AppCell>>, cell_name: &String,
               tree_name: &String, port: usize, link_type: LinkType) -> &'a HashMap<usize, Neighbor> {
    let appcell = cells
        .entry(cell_name.clone())
        .or_insert(Default::default());
    appcell
        .trees_mut()
        .entry(tree_name.clone())
        .or_insert(Default::default())
        .tree_mut()
        .insert(port, link_type);
    appcell.neighbors()
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
    discover_type: String, // First or Subsequent
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
        .data(web::Data::new(AppCells::default()))
        .route("", web::get().to(show_black_tree))
}
pub fn post() -> Scope {
    web::scope("/ca_process_discoverd_msg")
        .data(web::Data::new(AppCells::default()))
        .route("", web::post().to(process_discoverd))
}
#[derive(Debug, Default, Serialize)]
pub struct AppBlackTree {
    neighbors: Mutex<HashMap<String,String>>
}
pub fn data() -> web::Data<AppBlackTree> {
    web::Data::new(AppBlackTree::default())
}
