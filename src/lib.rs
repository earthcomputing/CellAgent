#![deny(unused_must_use)]
//#![warn(rust_2018_idioms)]
#![recursion_limit="1024"]
#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
mod blueprint;
mod cellagent;
mod cmodel;
mod config;
mod container;
mod dal;
mod datacenter;
mod dumpstack;
mod ecargs;
mod errors;
mod gvm_equation;
mod link;
mod message;
mod message_types;
mod nalcell;
mod name;
mod noc;
mod packet;
mod packet_engine;
mod port;
mod port_tree;
mod routing_table;
mod routing_table_entry;
mod service;
mod tenant;
mod traph;
mod traph_element;
mod tree;
mod uptree_spec;
mod utility;
mod uuid_ec;
mod vm;

use std::{io::{stdin, stdout, Read, Write},
          sync::mpsc::channel};

use crate::config::{NCELLS, CellNo,
                    Edge, get_edges};
use crate::datacenter::Datacenter;
use crate::utility::{TraceHeader};

trait Test {
    fn test(&mut self);
}


struct DatacenterGraph {
    expected_num_cells: CellNo,
    expected_edges: Vec<Edge>,
    dc: Datacenter,
}

impl DatacenterGraph {
    fn new_sample() -> DatacenterGraph {
        let (dc, outside_to_noc) = match Datacenter::construct_sample() {
            Ok(pair) => pair,
            Err(err) => panic!("Datacenter construction failure: {}", err)
        };
	DatacenterGraph {
            expected_num_cells: NCELLS,
            expected_edges: get_edges(),
            dc: dc,
        }
    }
}

impl Test for DatacenterGraph {
    fn test(&mut self) {
        assert_eq!(self.dc.get_cells().len(), *self.expected_num_cells);
        assert_eq!(self.dc.get_cell_ids().len(), *self.expected_num_cells);
        assert_eq!(self.dc.get_links_mut().len(), self.expected_edges.len());
        for link in self.dc.get_links_mut() {
            // Could check that each link is in expected_edges (or vice versa)
        }
    }
}

impl Drop for DatacenterGraph {
    fn drop(&mut self) {
        // teardown goes here
    }
}

#[test]
fn test_sample_graph() {
    DatacenterGraph::new_sample().test();
}


// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "MainError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "MainError::Console {} {} is not a valid input {}", func_name, input, comment)]
    Console { func_name: &'static str, input: String, comment: String },
    #[fail(display = "MainError::Kafka {} Kafka producer undefined", func_name)]
    Kafka { func_name: &'static str}
}
