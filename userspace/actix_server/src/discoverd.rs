/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{
    collections::{
        HashMap,
    },
    sync::{MutexGuard}
};

use actix_web::{web, Error, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hello::{AppCell, AppCells, LinkType, Neighbor};

type Size = usize;

async fn show_black_tree(appcells: web::Data<AppCells>) -> Result<HttpResponse, actix_web::Error> {
    let appcells = appcells.get_ref();
    let string = serde_json::to_string(appcells)?;
    Ok(HttpResponse::Ok().body(string))
}
async fn process_discoverd(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("DiscoverDMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    process_discoverd_body(appcells, body)
}
pub fn process_discoverd_body(appcells: web::Data<AppCells>, body: Body)
        -> Result<impl Responder, Error> {
    if body.msg.payload.discover_type == "Parent" {
        let this_cell_name = body.cell_id.name;
        let recv_port = body.port_no;
        let tree_name = body.msg.payload.port_tree_id.name;
        let mut cells = appcells
            .get_ref()
            .appcells.lock().unwrap();
        let other_cell =
            update_tree(&mut cells, &this_cell_name, &tree_name, recv_port, LinkType::Child)
                .get(&recv_port).expect("DiscoverDMsg: missing neighbor")
                .clone(); // Avoid borrow error on next update_tree call
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
        .black_trees_mut()
        .entry(tree_name.clone())
        .or_insert(Default::default())
        .tree_mut()
        .insert(port, link_type);
    appcell.neighbors()
}

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
        .route("", web::get().to(show_black_tree))
}
pub fn post() -> Scope {
    web::scope("/ca_process_discoverd_msg")
        .route("", web::post().to(process_discoverd))
}
