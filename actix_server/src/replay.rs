use std;
use std::{
          fmt,
          collections::HashMap,
          env::args,
          fs::{File, create_dir, remove_dir_all},
          io::{self, prelude::*, BufReader},
          path::Path,
          ops::{Deref}};

use actix_web::{web, Error, HttpServer, Responder, HttpResponse, Scope};
use serde::{Deserialize, Serialize};

use ec_fabrix::dal;

use crate::hello::{AppCells};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileNameParams { filename: String }
fn replay_from_file(params: web::Form<FileNameParams>) -> Result<impl Responder, Error> {
    let filename = &params.filename;
    let replay_file = File::open(filename)?;
    let reader = BufReader::new(replay_file);
    for line in reader.lines() {
        println!("{}", line?);
    }
    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .body(format!("Replay from file {}", filename)))
}
pub fn post() -> Scope {
    web::scope("/replay")
        .data(web::Data::new(AppCells::default()))
        .route("", web::post().to(replay_from_file))
}
