#[macro_use] extern crate actix_web;
mod geometry;

use std::{fmt, fmt::Write};
use std::sync::{Arc, Mutex};

use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Serialize, Deserialize};

use geometry::{Geometry, Id, Location};
use std::error::Error;

#[get("/")]
fn index() -> impl Responder {
    HttpResponse::Ok().body("EARTH Computing Trace Visualizer")
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
#[post("add")]
fn add(cell_data: web::Json<CellData>, mut state: web::Data<AppGeometry>) -> impl Responder {
    println!("adding {:?}", cell_data);
    let mut geometry = state.get_ref();
    geometry.geometry.lock().unwrap().add(Id::new(cell_data.id), Location::new(cell_data.row, cell_data.col));
    HttpResponse::Ok().body(format!("adding {}\n{:?}\n", cell_data, geometry.geometry))
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
    let mut geometry = Geometry::default();
    geometry.add(Id::new(0), Location::new(0,0));
    geometry.add(Id::new(1), Location::new(1,1));
    geometry.add(Id::new(2), Location::new(0,2));
    let app_geometry = AppGeometry { geometry: Arc::new(Mutex::new(geometry)) };
    HttpServer::new(move || {
        let state = web::Data::new(app_geometry.clone());
        App::new()
            .register_data(state)
            .data(web::JsonConfig::default().limit(4096))
            .service(index)
            .service(show_geometry)
            .service(show_discover_d)
            .service(add)
    })
        .bind("127.0.0.1:8088")
        .unwrap()
        .run()
        .unwrap();
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CellData { id: usize, row: usize, col: usize }
impl fmt::Display for CellData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id {} Row {} Col {}", self.id, self.row, self.col)
    }
}
