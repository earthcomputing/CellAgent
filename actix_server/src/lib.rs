#![warn(bare_trait_objects)]
#![deny(unused_must_use)]
//#![allow(dead_code)]
//#![allow(unused_variables)]
//#![allow(unused_imports)]
//#![warn(rust_2018_idioms)]

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

pub mod geometry;