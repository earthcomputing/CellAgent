use actix_web::{HttpResponse, Responder};

pub fn index() -> impl Responder {
    HttpResponse::Ok().body("EARTH Computing Trace Visualizer")
}