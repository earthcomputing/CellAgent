use std::{env,
          fs,
};

use actix_web::{web, App, HttpServer, Responder, HttpResponse};

use ec_trace_analyzer::{discoverd, geometry, hello, index, replay, stacktreed};

fn main() {
    let server_url = env::var("SERVER_URL").expect("Environment variable SERVER_URL not found");
    println!("Server at {}", server_url);
    let html_file_name = env::var("HTML").expect("Environment variable HTML not found");
    println!("html_file_name {}", html_file_name);
    let index_data = web::Data::new(html_file_name);
    let geo_data = geometry::data();
    let hello_data = hello::data();
    HttpServer::new(move || {
        App::new()
            .route("/visualizer.css", web::get().to(get_css))
            .route("/visualize.js", web::get().to(get_visualizer))
            
            .register_data(index_data.clone())
            .route("/",web::get().to(index::index))
            
            .route("/reset", web::post().to(replay::reset))
            
            .register_data(geo_data.clone())
            .service(geometry::get())
            .service(geometry::post_border())
            .service(geometry::post_interior())
            
            .register_data(hello_data.clone())
            .service(hello::get())
            .service(hello::post())
        
            .service(discoverd::get())
            .service(discoverd::post())
        
            .service(stacktreed::get())
            .service(stacktreed::post())
        
            .service(replay::post())
    })
        .keep_alive(100)
        .bind(server_url)
        .unwrap()
        .run()
        .unwrap();
}
fn get_css() -> impl Responder {
    let css = fs::read_to_string("./html/visualizer.css").expect("Cannot read CSS file");
    HttpResponse::Ok()
        .content_type("text/css")
        .body(css)
}
fn get_visualizer() -> impl Responder {
    let viz = fs::read_to_string("./html/visualize.js").expect("Cannot read JavaScript file");
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(viz)
}