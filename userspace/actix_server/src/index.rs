/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
use std::{
    fs
};

use actix_web::{web, HttpResponse, Responder};

pub async fn index(html_file_data: web::Data<String>) -> impl Responder {
    HttpResponse::Ok().body(html(html_file_data))}

fn html(html_file_data: web::Data<String>) -> String {
    let html_file_name = html_file_data.get_ref();
     fs::read_to_string(html_file_name)
        .expect(&format!("Cannot read html file {}", html_file_name))
}
