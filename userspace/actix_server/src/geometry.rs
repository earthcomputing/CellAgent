/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{cmp::max,
          collections::HashMap,
          sync::{Mutex}
};

use actix_web::{web, Error, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type RowCol = HashMap<String, Location>;

pub fn cell_geometry(path: &str, is_border: bool, state: web::Data<AppGeometry>, record: web::Json<Value>)
                 -> Result<impl Responder, Error> {
    let trace_body = record.get("body").expect("HelloMsg: bad trace record");
    let body: Body = serde_json::from_value(trace_body.clone())?;
    cell_geometry_body(path, is_border, state, body)
}
pub fn cell_geometry_body(path: &str, is_border: bool, state: web::Data<AppGeometry>, body: Body)
        -> Result<impl Responder, Error>{
    let name = body.cell_id.name;
    let location = body.location;
    let app_geometry = state.get_ref();
    app_geometry.geometry.lock().unwrap().add(CellID { name: name.clone() },
                                              Location { row: location[0], col: location[1], is_border });
    Ok(HttpResponse::Ok().body(format!("{} adding {}\n{:?}\n", path, name, app_geometry)))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    cell_id: CellID,
    location: [usize; 2]
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellID {
    name: String
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppGeometry {
    pub geometry: Mutex<Geometry>
}
impl AppGeometry {
    pub fn clear(&mut self) {
        self.geometry = Mutex::new(Geometry::default());
    }
}
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Location { row: usize, col: usize, is_border: bool }

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Geometry {
    pub is_border: bool,
    pub maxrow: usize,
    pub maxcol: usize,
    pub rowcol: RowCol
}
impl Geometry {
    pub fn add(&mut self, cell_id: CellID, rowcol: Location) {
        self.is_border = rowcol.is_border;
        self.maxrow = max(self.maxrow, rowcol.row);
        self.maxcol = max(self.maxrow, rowcol.col);
        self.rowcol.insert(cell_id.name, rowcol);
    }
}
async fn border_cell_start(state: web::Data<AppGeometry>, record: web::Json<Value>)
                     ->impl Responder {
    let path = "border_cell_start";
    cell_geometry(path, true, state, record)
}
async fn interior_cell_start(state: web::Data<AppGeometry>, record: web::Json<Value>)
                       -> impl Responder {
    let path = "interior_cell_start";
    cell_geometry(path, false, state, record)
}
async fn show_geometry(state: web::Data<AppGeometry>) -> Result<HttpResponse, actix_web::Error> {
    let geo = state.get_ref();
    let string = serde_json::to_string(geo)?;
    Ok(HttpResponse::Ok().body(string))
}

pub fn get() -> Scope {
    web::scope("/geometry")
        .route("", web::get().to(show_geometry))
}
pub fn post_border() -> Scope {
    web::scope("/border_cell_start")
        .route("", web::post().to(border_cell_start))
}
pub fn post_interior() -> Scope {
    web::scope("/interior_cell_start")
        .route("", web::post().to(interior_cell_start))
}
pub fn data() -> web::Data<AppGeometry> {
    web::Data::new(AppGeometry::default())
}
