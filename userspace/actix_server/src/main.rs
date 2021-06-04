/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{env,
          fs,
};

use actix_web::{web, App, HttpServer, Responder, HttpResponse};

use ec_trace_analyzer::{discoverd, geometry, hello, index, replay, stacktreed};

#[actix_rt::main]
async fn main() {
    let server_url = env::var("SERVER_URL").expect("Environment variable SERVER_URL not found");
    println!("Server at {}", server_url);
    let html_file_name = env::var("HTML").expect("Environment variable HTML not found");
    println!("html_file_name {}", html_file_name);
    let index_data = web::Data::new(html_file_name); // Location of index.html
    let geo_data = geometry::data();
    let hello_data = hello::data();
    HttpServer::new(move || {
        App::new()
            .route("/visualizer.css", web::get().to(get_css))
            .route("/visualize.js", web::get().to(get_visualizer))
            
            .app_data(index_data.clone())
            .route("/",web::get().to(index::index))
            
            .route("/reset", web::post().to(replay::reset))
            
            .app_data(geo_data.clone())
            .service(geometry::get())
            .service(geometry::post_border())
            .service(geometry::post_interior())
            
            .app_data(hello_data.clone())
            .service(hello::get())
            .service(hello::post())
        
            .service(discoverd::get())
            .service(discoverd::post())
        
            .service(stacktreed::get())
            .service(stacktreed::post())
        
            .service(replay::post())
        })
        .keep_alive(100usize)
        .bind(server_url)
        .unwrap()
        .run()
        .await
        .unwrap();
}
async fn get_css() -> impl Responder {
    let css = fs::read_to_string("./html/visualizer.css").expect("Cannot read CSS file");
    HttpResponse::Ok()
        .content_type("text/css")
        .body(css)
}
async fn get_visualizer() -> impl Responder {
    let viz = fs::read_to_string("./html/visualize.js").expect("Cannot read JavaScript file");
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(viz)
}
