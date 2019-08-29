use std::{
    collections::{
        HashMap,
        hash_map::Entry
    },
    fmt,
    sync::{Arc, Mutex}
};

use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::trace_record::TraceRecord;
use crate::geometry::AppGeometry;

type Size = usize;

pub fn hello(appcells: web::Data<AppCells>, record: web::Json<Value>)
                     -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("HelloMsg: bad trace record");
    let body: Body = match serde_json::from_value(trace_body.clone()) {
        Ok(b) => b,
        Err(e) => { println!("Hello: {:?}", e); return Err(e.into()); }
    };
    let this_cell_id = body.cell_id.name;
    let sending_cell_id = body.msg.payload.cell_id.name;
    let my_port_no = body.port_no;
    let other_port_no = body.msg.payload.port_no;
    let neighbor = Neighbor::new(sending_cell_id, other_port_no);
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
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CellID {
    name: String
}
#[derive(Debug, Default, Serialize)]
pub struct AppCells {
    neighbors: Mutex<HashMap<String,Neighbors>>
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
struct Neighbors {
    neighbors: HashMap<Size, Neighbor>
}
impl Neighbors {
    fn add(&mut self, port: Size, neighbor: Neighbor) {
        self.neighbors.insert(port, neighbor);
    }
}
impl fmt::Display for Neighbors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Neighbors\n")?;
        for neighbor in self.neighbors.values() { write!(f, "{}\n", neighbor)?; }
        write!(f, "")
    }
}
#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
struct Neighbor {
    name: String,
    port: Size
}
impl Neighbor {
    fn new(name: String, port: Size) -> Neighbor { Neighbor { name, port }}
}
impl fmt::Display for Neighbor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Port {} connected to cell {}", self.port, self.name)
    }
}
