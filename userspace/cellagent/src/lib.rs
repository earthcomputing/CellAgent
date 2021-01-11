#![warn(bare_trait_objects)]
#![deny(unused_must_use)]
//#![allow(dead_code)]
//#![allow(unused_variables)]
//#![allow(unused_imports)]
#![warn(rust_2018_idioms)]
#![recursion_limit="1024"]
#[macro_use] extern crate crossbeam;
#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

pub mod app_message;
pub mod app_message_formats;
pub mod blueprint;
pub mod cellagent;
pub mod cmodel;
pub mod config;
pub mod container;
pub mod dal;
#[cfg(any(feature = "simulator"))]
pub mod datacenter;
pub mod dumpstack;
#[cfg(feature = "cell")]
pub mod ecnl;
pub mod errors;
pub mod gvm_equation;
#[cfg(any(feature = "simulator"))]
pub mod link;
pub mod ec_message;
pub mod ec_message_formats;
#[cfg(feature = "cell")]
pub mod ecnl_port;
#[cfg(any(feature = "simulator", feature = "cell"))]
pub mod nalcell;
pub mod name;
pub mod noc;
pub mod packet;
pub mod packet_engine;
pub mod port;
pub mod port_tree;
#[cfg(any(feature = "simulator"))]
pub mod rack;
pub mod replay;
pub mod routing_table;
pub mod routing_table_entry;
pub mod service;
pub mod simulated_port;
pub mod snake;
pub mod tenant;
pub mod traph;
pub mod traph_element;
pub mod tree;
pub mod uptree_spec;
pub mod utility;
pub mod uuid_ec;
pub mod vm;

