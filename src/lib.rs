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

use crate::config::{CellNo, CellQty, Edge, PortNo, PortQty, is2e};
use crate::datacenter::Datacenter;
use crate::utility::{TraceHeader};

trait Test {
    fn test(&mut self);
}


macro_rules! test_result {
    ($name:ident, $testable:expr) => {
        #[test]
        fn $name() {
            $testable.test();
        }
    }
}


struct DatacenterGraph {
    expected_num_cells: CellQty,
    expected_edges: Vec<Edge>,
    dc: Datacenter,
}

impl DatacenterGraph {
    fn new_three_cells() -> DatacenterGraph {
        let num_cells = CellQty(3);
        let edges = vec![is2e(0,1), is2e(0,2), is2e(1,2)];
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(3)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                num_cells,
                &edges,
                PortQty(3),
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterGraph {
            expected_num_cells: num_cells,
            expected_edges: edges,
            dc: dc,
        }
    }
    fn new_four_cells() -> DatacenterGraph {
        let num_cells = CellQty(4);
        let edges = vec![is2e(0,1), is2e(0,2), is2e(1,2), is2e(0,3), is2e(1,3)];//, is2e(2,3)]
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(3)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                num_cells,
                &edges,
                PortQty(3),
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterGraph {
            expected_num_cells: num_cells,
            expected_edges: edges,
            dc: dc,
        }
    }
    fn new_ten_cells() -> DatacenterGraph {
        let num_cells = CellQty(10);
        let edges =
            vec![
                is2e(0,1), is2e(1,2), is2e(2,3), is2e(3,4),
                is2e(5,6), is2e(6,7), is2e(7,8), is2e(8,9),
                is2e(0,5), is2e(1,6), is2e(2,7), is2e(3,8), is2e(4,9)
            ];
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(5)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                CellQty(10),
                &edges,
                PortQty(5),
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterGraph {
            expected_num_cells: num_cells,
            expected_edges: edges,
            dc: dc,
        }
    }
    // blueprint-baran-distributed.gv
    // 97 edges
    fn new_fortyseven_cells() -> DatacenterGraph {
        let num_cells = CellQty(47);
        let edges =
            vec![
                is2e( 0, 1), is2e( 0, 4), is2e( 1, 2), is2e( 1, 5), is2e( 1, 6), is2e( 2, 3), is2e( 2, 6), is2e( 2, 7), is2e( 3, 8),
                is2e( 4, 5), is2e( 4, 9), is2e( 5, 6), is2e( 5,10), is2e( 5,11), is2e( 6, 7), is2e( 6,12), is2e( 7, 8), is2e( 7,13),
                is2e( 8,14), is2e( 9,10), is2e( 9,15), is2e(10,11), is2e(10,16), is2e(11,12), is2e(11,16), is2e(11,18), is2e(12,13),
                is2e(12,18), is2e(13,14), is2e(13,19), is2e(14,19), is2e(14,20), is2e(15,16), is2e(15,17), is2e(15,26), is2e(16,17),
                is2e(17,18), is2e(17,21), is2e(17,26), is2e(18,19), is2e(18,22), is2e(18,23), is2e(19,20), is2e(19,23), is2e(19,24),
                is2e(20,25), is2e(21,22), is2e(21,27), is2e(21,28), is2e(22,28), is2e(22,29), is2e(23,24), is2e(23,29), is2e(24,25),
                is2e(24,30), is2e(25,30), is2e(21,26), is2e(26,27), is2e(26,31), is2e(27,28), is2e(27,32), is2e(28,29), is2e(28,32),
                is2e(28,33), is2e(29,30), is2e(29,34), is2e(30,34), is2e(30,38), is2e(27,31), is2e(31,35), is2e(32,33), is2e(32,35),
                is2e(32,36), is2e(33,34), is2e(33,36), is2e(33,37), is2e(34,37), is2e(35,36), is2e(35,39), is2e(35,40), is2e(36,37),
                is2e(36,41), is2e(37,38), is2e(37,42), is2e(37,43), is2e(38,43), is2e(31,39), is2e(39,40), is2e(40,41), is2e(40,45),
                is2e(41,42), is2e(41,46), is2e(42,43), is2e(42,46), is2e(39,44), is2e(44,45), is2e(45,46)
            ];
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(8)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                num_cells,
                &edges,
                PortQty(8),
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterGraph {
            expected_num_cells: num_cells,
            expected_edges: edges,
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

test_result!(test_graph_three_cells, DatacenterGraph::new_three_cells());
test_result!(test_graph_four_cells, DatacenterGraph::new_four_cells());
test_result!(test_graph_ten_cells, DatacenterGraph::new_ten_cells());
test_result!(test_graph_fortyseven_cells, DatacenterGraph::new_fortyseven_cells());


struct DatacenterPorts {
    default_num_phys_ports_per_cell: PortQty,
    cell_port_exceptions: HashMap<CellNo, PortQty>,
    dc: Datacenter,
}

impl DatacenterPorts {
    fn new_with_exceptions() -> DatacenterPorts {
        let default_num_phys_ports_per_cell = PortQty(8);
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(5), PortQty(7));
        cell_port_exceptions.insert(CellNo(2), PortQty(6));
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(7)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                CellQty(10),
                &vec![is2e(0,1), is2e(1,2), is2e(2,3), is2e(3,4),
                      is2e(5,6), is2e(6,7), is2e(7,8), is2e(8,9),
                      is2e(0,5), is2e(1,6), is2e(2,7), is2e(3,8), is2e(4,9)],
                default_num_phys_ports_per_cell,
                &cell_port_exceptions,
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterPorts {
            default_num_phys_ports_per_cell: default_num_phys_ports_per_cell,
            cell_port_exceptions: cell_port_exceptions,
            dc: dc,
        }
    }
}

impl Test for DatacenterPorts {
    fn test(&mut self) {
        for cell in self.dc.get_cells() {
            match self.cell_port_exceptions.get(&cell.get_no()) {
                Some(num_phys_ports) => {
                    assert_eq!(cell.get_num_ports(), PortQty(**num_phys_ports+1));
                }
                None => {
                    assert_eq!(cell.get_num_ports(), PortQty(*self.default_num_phys_ports_per_cell+1));
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

test_result!(test_ports, DatacenterPorts::new_with_exceptions());


struct DatacenterBorder {
    expected_border_cell_ports: HashMap<CellNo, Vec<PortNo>>,
    dc: Datacenter,
}

impl DatacenterBorder {
    fn new_partial_border() -> DatacenterBorder {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(2), vec![PortNo(3)]);
        border_cell_ports.insert(CellNo(7), vec![PortNo(1)]);
        let (dc, outside_to_noc) =
            match Datacenter::construct(
                CellQty(10),
                &vec![is2e(0,1), is2e(1,2), is2e(2,3), is2e(3,4),
                      is2e(5,6), is2e(6,7), is2e(7,8), is2e(8,9),
                      is2e(0,5), is2e(1,6), is2e(2,7), is2e(3,8), is2e(4,9)],
                PortQty(4),
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
	DatacenterBorder {
            expected_border_cell_ports: border_cell_ports,
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

test_result!(test_border, DatacenterBorder::new_partial_border());


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
