use std::{env};

use actix_web::{web, App, HttpServer, Responder};

use ec_trace_analyzer::{geometry, hello, index};

fn main() {
    let server_url = env::var("SERVER_URL").expect("Environment variable SERVER_URL not found");
    println!("Server at {}", server_url);
    let geo_data = geometry::data();
    let hello_data = hello::data();
    HttpServer::new(move || {
        App::new()
            .route("/",web::get().to(index::index))
            
            .register_data(geo_data.clone())
            .service(geometry::get())
            .service(geometry::post_border())
            .service(geometry::post_interior())
            
            .register_data(hello_data.clone())
            .service(hello::get())
            .service(hello::post())
    })
        .bind(server_url)
        .unwrap()
        .run()
        .unwrap();
}