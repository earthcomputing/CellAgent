use std::{
    collections::{
        HashMap,
    },
    sync::{Mutex}
};

use actix_web::{web, Error, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Size = usize;

pub fn hello(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("HelloMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    let this_cell_id = body.cell_id.name;
    let sending_cell_id = CellID { name: body.msg.payload.cell_id.name };
    let my_port_no = body.port_no;
    let other_port_no = body.msg.payload.port_no;
    let neighbor = Neighbor { cell_id: sending_cell_id, port: other_port_no};
    let mut cells = appcells
        .get_ref()
        .neighbors.lock().unwrap();
    let neighbors = cells
        .entry(this_cell_id)
        .or_insert(Default::default());
    neighbors.neighbors.insert(my_port_no, neighbor);
    Ok(HttpResponse::Ok().body(format!("Adding hello")))
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
#[derive(Debug, Default, Serialize)]
pub struct AppCells {
    neighbors: Mutex<HashMap<String,Neighbors>>
}
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
struct CellID {
    name: String
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
struct Neighbors {
    neighbors: HashMap<Size, Neighbor>
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
struct Neighbor {
    cell_id: CellID,
    port: Size
}
