use std;
use std::{
          fs::{File},
          io::{prelude::*, BufReader},
};

use actix_web::{http, web, Error, Responder, HttpResponse, Scope};
use serde::{Deserialize, Serialize};

use crate::{discoverd, geometry, hello, stacktreed};
use crate::geometry::{AppGeometry, RowCol};
use crate::hello::{AppCells};
use crate::trace_record::TraceRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileNameParams { filename: String }

fn replay_from_file(appcells: web::Data<AppCells>, appgeometry: web::Data<AppGeometry>,
                    form_data: web::Form<FileNameParams>)
                    -> Result<impl Responder, Error> {
    reset(appcells.clone(), appgeometry.clone());
    let filename = &form_data.filename;
    let replay_file = File::open(filename)?;
    let reader = BufReader::new(replay_file);
    for line in reader.lines() {
        let mut foo = line?;
        foo.pop();
        let trace_record: TraceRecord = serde_json::from_str(&foo)?;
        let header = trace_record.header();
        let body = trace_record.body().clone();
        match header.format() {
            "ca_process_discoverd_msg" => {
                let body: discoverd::Body = serde_json::from_value(body)?;
                discoverd::process_discoverd_body(appcells.clone(), body)?;
            },
            "border_cell_start" => {
                let body: geometry::Body = serde_json::from_value(body)?;
                let path = "border_cell_start";
                let is_border = true;
                geometry::cell_geometry_body(path, is_border, appgeometry.clone(), body)?;
            },
            "interior_cell_start" => {
                let body: geometry::Body = serde_json::from_value(body)?;
                let path = "interior_cell_start";
                let is_border = false;
                geometry::cell_geometry_body(path, is_border, appgeometry.clone(), body)?;
            },
            "ca_process_stack_treed_msg" => {
                let body: stacktreed::Body = serde_json::from_value(body)?;
                stacktreed::process_stack_treed_body(appcells.clone(), body)?;
            },
            "ca_process_hello_msg" => {
                let body: hello::Body = serde_json::from_value(body)?;
                hello::process_hello_body(appcells.clone(), body)?;
            }
            _ => ()
        }
    }
    Ok(HttpResponse::Found()
        .header(http::header::LOCATION, "/")
        .content_type("text/plain")
        .body(format!("Replay from file {}", filename)))
}
pub fn reset(appcells: web::Data<AppCells>, geometry: web::Data<AppGeometry>) {
    let mut cells = appcells
        .get_ref()
        .appcells.lock().unwrap();
    cells.clear();
    let mut geometry = geometry
        .get_ref()
        .geometry.lock().unwrap();
    geometry.maxcol = 0;
    geometry.maxrow = 0;
    geometry.rowcol = RowCol::default();
}
pub fn post() -> Scope {
    web::scope("/replay")
        .data(web::Data::new(AppCells::default()))
        .route("", web::post().to(replay_from_file))
}
