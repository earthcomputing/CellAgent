mod geometry;

use std::{fmt, env};

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use geometry::{cell_geometry, AppGeometry};

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
fn main() {
    let server_url = env::var("SERVER_URL").expect("Environment variable SERVER_URL not found");
    println!("Server at {}", server_url);
    let app_geometry = AppGeometry::default();
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
        .bind(server_url)
        .unwrap()
        .run()
        .unwrap();
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
