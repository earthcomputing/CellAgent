mod geometry;

use std::{fmt};
use std::sync::{Arc, Mutex};

use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use failure::{Error};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use geometry::{Geometry, Id, Location};

#[get("/")]
fn index() -> impl Responder {
    HttpResponse::Ok().body("EARTH Computing Trace Visualizer")
}
#[post("border_cell_start")]
fn border_cell_start(mut state: web::Data<AppGeometry>, record: web::Json<Value>)
        ->impl Responder {
    let path = "border_cell_start";
    println!("Update border cell");
    cell_geometry(path, state, record)
}
#[post("interior_cell_start")]
fn interior_cell_start(mut state: web::Data<AppGeometry>, record: web::Json<Value>)
        ->impl Responder {
    let path = "interior_cell_start";
    println!("Update interior cell");
    cell_geometry(path, state, record)
}
#[get("geometry")]
fn show_geometry(state: web::Data<AppGeometry>) -> Result<HttpResponse, actix_web::Error> {
    let geo = state.get_ref();
    let string = serde_json::to_string(geo)?;
    Ok(HttpResponse::Ok().body(string))
}
#[get("discoverd")]
fn show_discover_d() -> impl Responder {
    HttpResponse::Ok().body("Showing DiscoverD messages")
}
#[derive(Debug, Clone, Serialize)]
struct TraceRecord {
    record: Value
}
#[derive(Debug, Clone, Serialize)]
struct AppGeometry {
    geometry: Arc<Mutex<Geometry>>
}
impl Responder for AppGeometry {
    type Error = actix_web::Error;
    type Future = Result<HttpResponse, actix_web::Error>;
    
    fn respond_to(self, _req: &HttpRequest) -> Self::Future {
        let body = serde_json::to_string(&self)?;
        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(body))
    }
}
fn main() {
    let app_geometry = AppGeometry { geometry: Arc::new(Mutex::new(Geometry::default())) };
    HttpServer::new(move || {
        let state = web::Data::new(app_geometry.clone());
        App::new()
            .register_data(state)
            .service(index)
            .service(show_geometry)
            .service(show_discover_d)
            .service(border_cell_start)
            .service(interior_cell_start)
    })
        .bind("127.0.0.1:8088")
        .unwrap()
        .run()
        .unwrap();
}
fn cell_geometry(path: &str, state: web::Data<AppGeometry>, record: web::Json<Value>)
    -> Result<impl Responder, Error> {
    #[derive(Debug, Copy, Clone, Deserialize)]
    struct GeoStruct {
        cell_number: usize,
        location: [usize; 2]
    }
    let app_geometry = state.get_ref();
    if let Some(body) = record.get("body") {
        let geo = serde_json::from_value::<GeoStruct>(body.clone())?;
        app_geometry.geometry.lock().unwrap().add(Id::new(geo.cell_number),
                                              Location::new(geo.location));
    }
    Ok(HttpResponse::Ok().body(format!("{} adding {}\n{:?}\n", path, record, app_geometry)))
}
fn _err_msg(path: &str, record: &Value) -> String {
    format!("{}: Bad trace record {}", path, record)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CellData { id: usize, row: usize, col: usize }
impl fmt::Display for CellData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id {} Row {} Col {}", self.id, self.row, self.col)
    }
}
