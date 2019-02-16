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
          collections::{HashMap},
          sync::mpsc::channel};

use crate::config::{NCELLS, NPORTS, CellNo, CellQty,
                    Edge, PortNo, PortQty, get_edges};
use crate::datacenter::Datacenter;
use crate::utility::{TraceHeader};

trait Test {
    fn test(&mut self);
}


struct DatacenterGraph {
    expected_num_cells: CellQty,
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


struct DatacenterPorts {
    default_num_ports_per_cell: PortQty,
    cell_port_exceptions: HashMap<CellNo, PortQty>,
    dc: Datacenter,
}

impl DatacenterPorts {
    fn new_sample() -> DatacenterPorts {
        let (dc, outside_to_noc) = match Datacenter::construct_sample() {
            Ok(pair) => pair,
            Err(err) => panic!("Datacenter construction failure: {}", err)
        };
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(5), PortQty(7));
        cell_port_exceptions.insert(CellNo(2), PortQty(6));
	DatacenterPorts {
            default_num_ports_per_cell: NPORTS,
            cell_port_exceptions: cell_port_exceptions,
            dc: dc,
        }
    }
}

impl Test for DatacenterPorts {
    fn test(&mut self) {
        for cell in self.dc.get_cells() {
            match self.cell_port_exceptions.get(&cell.get_no()) {
                Some(num_ports) => {
                    assert_eq!(cell.get_num_ports(), PortQty(**num_ports+1));
                }
                None => {
                    assert_eq!(cell.get_num_ports(), PortQty(*self.default_num_ports_per_cell+1));
                }
            }
        }
    }
}

impl Drop for DatacenterPorts {
    fn drop(&mut self) {
        // teardown goes here
    }
}

#[test]
fn test_sample_ports() {
    DatacenterPorts::new_sample().test();
}


struct DatacenterBorder {
    expected_border_cell_ports: HashMap<CellNo, Vec<PortNo>>,
    dc: Datacenter,
}

impl DatacenterBorder {
    fn new_sample() -> DatacenterBorder {
        let (dc, outside_to_noc) = match Datacenter::construct_sample() {
            Ok(pair) => pair,
            Err(err) => panic!("Datacenter construction failure: {}", err)
        };
        let mut expected_border_cell_ports = HashMap::new();
        expected_border_cell_ports.insert(CellNo(2), vec![PortNo(2)]);
        expected_border_cell_ports.insert(CellNo(7), vec![PortNo(2)]);
	DatacenterBorder {
            expected_border_cell_ports: expected_border_cell_ports,
            dc: dc,
        }
    }
}

impl Test for DatacenterBorder {
    fn test(&mut self) {
        for cell in self.dc.get_cells() {
            let border_ports: &Vec<PortNo>;
            let mut is_border_cell: bool = false;
            match self.expected_border_cell_ports.get(&cell.get_no()) {
                Some(port_nums) => {
                    border_ports = port_nums;
                    is_border_cell = !port_nums.is_empty();
                }
                None => {
                    border_ports = &Vec::new();
                }
            }
            assert_eq!(cell.is_border(), is_border_cell);
            for no in 0..*cell.get_num_ports() {
                // Could check that each port in cell is border or not as expected
            }
        }
    }
}

impl Drop for DatacenterBorder {
    fn drop(&mut self) {
        // teardown goes here
    }
}

#[test]
fn test_sample_border() {
    DatacenterBorder::new_sample().test();
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
