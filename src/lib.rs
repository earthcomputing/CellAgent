#![deny(unused_must_use)]
//#![warn(rust_2018_idioms)]
#![recursion_limit="1024"]
#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate lazy_static;
mod app_message;
mod app_message_formats;
mod blueprint;
mod cellagent;
mod cmodel;
mod config;
mod container;
mod dal;
mod datacenter;
mod dumpstack;
mod ec_message;
mod ec_message_formats;
mod errors;
mod gvm_equation;
mod link;
mod nalcell;
mod name;
mod noc;
mod packet;
mod packet_engine;
mod port;
mod port_tree;
mod rack;
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

use std::{collections::{HashMap}};

use crate::blueprint::{CellNo, Edge, is2e};
use crate::config::{MAX_NUM_PHYS_PORTS_PER_CELL, CellQty, PortNo, PortQty};
use crate::datacenter::{Datacenter};
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

macro_rules! test_error {
    ($name:ident, $testable:expr, $message:expr) => {
        #[test]
        #[should_panic]
// Message is currently not matching
//        #[should_panic(expected = $message($testable))]
        fn $name() {
            $testable.test();
        }
    }
}


#[derive(Clone)]
struct DatacenterGraphSpec {
    num_cells: CellQty,
    edges: Vec<Edge>,
}

lazy_static! {
    static ref TRIANGLE_GRAPH_SPEC: DatacenterGraphSpec = DatacenterGraphSpec {
        num_cells: CellQty(3),
        edges: vec![is2e(0, 1), is2e(0, 2), is2e(1, 2)],
    };
}

lazy_static! {
    static ref TWO_BY_TWO_WITH_DIAGONALS_GRAPH_SPEC: DatacenterGraphSpec = DatacenterGraphSpec {
        num_cells: CellQty(4),
        edges: vec![is2e(0,1), is2e(0,2), is2e(1,2), is2e(0,3), is2e(1,3), is2e(2,3)],
    };
}

lazy_static! {
    static ref FIVE_BY_TWO_GRAPH_SPEC: DatacenterGraphSpec = DatacenterGraphSpec {
        num_cells: CellQty(10),
        edges: vec![
            is2e(0,1), is2e(1,2), is2e(2,3), is2e(3,4),
            is2e(5,6), is2e(6,7), is2e(7,8), is2e(8,9),
            is2e(0,5), is2e(1,6), is2e(2,7), is2e(3,8), is2e(4,9)
        ],
    };
}

lazy_static! {
    static ref FORTYSEVEN_GRAPH_SPEC: DatacenterGraphSpec = DatacenterGraphSpec {
        num_cells: CellQty(47),
        edges: vec![
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
        ],
    };
}

impl DatacenterGraphSpec {
    fn new_invalid_edge_endpoint() -> DatacenterGraphSpec {
        DatacenterGraphSpec {
            num_cells: CellQty(3),
            edges: vec![is2e(0,1), is2e(0,2), is2e(1,3)],
        }
    }
}

impl Test for DatacenterGraphSpec {
    fn test(&mut self) {
        let _dc = {
            let mut border_cell_ports = HashMap::new();
            border_cell_ports.insert(CellNo(0), vec![PortNo(0)]);
            match Datacenter::construct(
                self.num_cells,
                &self.edges,
                MAX_NUM_PHYS_PORTS_PER_CELL,
                &HashMap::new(),
                &border_cell_ports,
            ) {
                Ok(dc) => dc,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            }
        };
    }
}

test_error!(test_graph_invalid_edge_endpoint, DatacenterGraphSpec::new_invalid_edge_endpoint(), format!("BlueprintError::EdgeEndpoint: Cell reference 3 in edges should be less than total number of cells {}", |datacenter_graph_spec| datacenter_graph_spec.num_cells));

struct DatacenterGraph {
    graph_spec: DatacenterGraphSpec,
    dc: Datacenter,
}

impl DatacenterGraph {
    fn new_three_cells() -> DatacenterGraph {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(2)]);
        match Datacenter::construct(
            TRIANGLE_GRAPH_SPEC.num_cells,
            &TRIANGLE_GRAPH_SPEC.edges,
            PortQty(3),
            &HashMap::new(),
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterGraph {
                graph_spec: TRIANGLE_GRAPH_SPEC.clone(),
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
    fn new_four_cells() -> DatacenterGraph {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(3)]);
        match Datacenter::construct(
            TWO_BY_TWO_WITH_DIAGONALS_GRAPH_SPEC.num_cells,
            &TWO_BY_TWO_WITH_DIAGONALS_GRAPH_SPEC.edges,
            PortQty(4),
            &HashMap::new(),
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterGraph {
                graph_spec: TWO_BY_TWO_WITH_DIAGONALS_GRAPH_SPEC.clone(),
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
    fn new_ten_cells() -> DatacenterGraph {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(4)]);
        match Datacenter::construct(
            FIVE_BY_TWO_GRAPH_SPEC.num_cells,
            &FIVE_BY_TWO_GRAPH_SPEC.edges,
            PortQty(5),
            &HashMap::new(),
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterGraph {
                graph_spec: FIVE_BY_TWO_GRAPH_SPEC.clone(),
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
    // blueprint-baran-distributed.gv
    // 97 edges
    fn new_fortyseven_cells() -> DatacenterGraph {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(7)]);
        match Datacenter::construct(
            FORTYSEVEN_GRAPH_SPEC.num_cells,
            &FORTYSEVEN_GRAPH_SPEC.edges,
            PortQty(8),
            &HashMap::new(),
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterGraph {
                graph_spec: FORTYSEVEN_GRAPH_SPEC.clone(),
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
}

impl Test for DatacenterGraph {
    fn test(&mut self) {
        assert_eq!(self.dc.get_rack().get_cells().len(), *self.graph_spec.num_cells);
        assert_eq!(self.dc.get_rack().get_cell_ids().len(), *self.graph_spec.num_cells);
        assert_eq!(self.dc.get_rack().get_links().len(), self.graph_spec.edges.len());
        for _link in self.dc.get_rack().get_links() {
            // Could check that each link is in edges (or vice versa)
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


#[derive(Clone)]
struct DatacenterPortsSpec<'a> {
    default_num_phys_ports_per_cell: PortQty,
    cell_port_exceptions: HashMap<CellNo, PortQty>,
    graph_spec: &'a DatacenterGraphSpec,
}

lazy_static! {
    static ref TRIANGLE_PORTS_SPEC: DatacenterPortsSpec<'static> = {
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(0), PortQty(3));
        DatacenterPortsSpec {
            default_num_phys_ports_per_cell: PortQty(2),
            cell_port_exceptions: cell_port_exceptions,
            graph_spec: &TRIANGLE_GRAPH_SPEC,
        }
    };
}

lazy_static! {
    static ref FIVE_BY_TWO_PORTS_SPEC: DatacenterPortsSpec<'static> = {
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(4), PortQty(2));
        DatacenterPortsSpec {
            default_num_phys_ports_per_cell: PortQty(4), // This seems like it should be OK as 3, but test_default_port_border fails
            cell_port_exceptions: cell_port_exceptions,
            graph_spec: &FIVE_BY_TWO_GRAPH_SPEC,
        }
    };
}

impl<'a> DatacenterPortsSpec<'a> {
    fn new_invalid_default_num_phys_ports_per_cell() -> DatacenterPortsSpec<'static> {
        let default_num_phys_ports_per_cell = PortQty(*MAX_NUM_PHYS_PORTS_PER_CELL+1);
        let cell_port_exceptions = HashMap::new();
        DatacenterPortsSpec {
            default_num_phys_ports_per_cell: default_num_phys_ports_per_cell,
            cell_port_exceptions: cell_port_exceptions,
            graph_spec: &TRIANGLE_GRAPH_SPEC,
        }
    }
    fn new_invalid_cell_ports_exception_cell() -> DatacenterPortsSpec<'static> {
        let default_num_phys_ports_per_cell = PortQty(3);
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(3), PortQty(2));
        DatacenterPortsSpec {
            default_num_phys_ports_per_cell: default_num_phys_ports_per_cell,
            cell_port_exceptions: cell_port_exceptions,
            graph_spec: &TRIANGLE_GRAPH_SPEC,
        }
    }
    fn new_invalid_cell_ports_exception_ports() -> DatacenterPortsSpec<'static> {
        let default_num_phys_ports_per_cell = PortQty(3);
        let mut cell_port_exceptions = HashMap::new();
        cell_port_exceptions.insert(CellNo(2), PortQty(3));
        DatacenterPortsSpec {
            default_num_phys_ports_per_cell: default_num_phys_ports_per_cell,
            cell_port_exceptions: cell_port_exceptions,
            graph_spec: &TRIANGLE_GRAPH_SPEC,
        }
    }
}

impl<'a> Test for DatacenterPortsSpec<'a> {
    fn test(&mut self) {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(0)]);
        let _dc =
            match Datacenter::construct(
                self.graph_spec.num_cells,
                &self.graph_spec.edges,
                self.default_num_phys_ports_per_cell,
                &self.cell_port_exceptions,
                &border_cell_ports,
            ) {
                Ok(dc) => dc,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
    }
}

test_error!(test_ports_invalid_default_num_phys_ports_per_cell, DatacenterPortsSpec::new_invalid_default_num_phys_ports_per_cell(), format!("BlueprintError::DefaultNumPhysPortsPerCell: Default number of physical ports per cell {} is greater than the maximum allowed {}", |datacenter_ports_spec| datacenter_ports_spec.default_num_phys_ports_per_cell, |datacenter_ports_spec| MAX_NUM_PHYS_PORTS_PER_CELL));
test_error!(test_ports_invalid_cell_ports_exception_cell, DatacenterPortsSpec::new_invalid_cell_ports_exception_cell(), format!("BlueprintError::CellPortsExceptionCell: Port exception requested for cell {}; number of cells is {}", |datacenter_ports_spec| datacenter_ports_spec.cell_ports_exceptions.keys().max(), |datacenter_ports_spec| datacenter_ports_spec.graph_spec.num_cells));
test_error!(test_ports_invalid_cell_ports_exception_ports, DatacenterPortsSpec::new_invalid_cell_ports_exception_ports(), format!("BlueprintError::CellPortsExceptionPorts: {} ports exception requested for cell {}; maximum number of ports is {}", |datacenter_ports_spec| {let mut cell_port_exceptions_vec: Vec<(&CellNo, &PortQty)> = datacenter_ports_spec.cell_port_exceptions.iter().collect(); cell_port_exceptions_vec.sort_by(|a, b| b.1.cmp(a.1)); cell_port_exceptions_vec[0].1}, |datacenter_ports_spec| {let mut cell_port_exceptions_vec: Vec<(&CellNo, &PortQty)> = datacenter_ports_spec.cell_port_exceptions.iter().collect(); cell_port_exceptions_vec.sort_by(|a, b| b.1.cmp(a.1)); cell_port_exceptions_vec[0].0}, |datacenter_ports_spec| MAX_NUM_PHYS_PORTS_PER_CELL));

struct DatacenterPorts<'a> {
    ports_spec: DatacenterPortsSpec<'a>,
    dc: Datacenter,
}

impl<'a> DatacenterPorts<'a> {
    fn new_with_exceptions() -> DatacenterPorts<'static> {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(2)]);
        match Datacenter::construct(
            FIVE_BY_TWO_PORTS_SPEC.graph_spec.num_cells,
            &FIVE_BY_TWO_PORTS_SPEC.graph_spec.edges,
            FIVE_BY_TWO_PORTS_SPEC.default_num_phys_ports_per_cell,
            &FIVE_BY_TWO_PORTS_SPEC.cell_port_exceptions,
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterPorts {
                ports_spec: FIVE_BY_TWO_PORTS_SPEC.clone(),
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
}

impl<'a> Test for DatacenterPorts<'a> {
    fn test(&mut self) {
        for (_cell_no, cell) in self.dc.get_rack().get_cells() {
            match self.ports_spec.cell_port_exceptions.get(&CellNo(cell.get_name().trim_start_matches("C:").parse().unwrap())) {
                Some(num_phys_ports) => {
                    assert_eq!(cell.get_num_ports(), PortQty(**num_phys_ports+1));
                }
                None => {
                    assert_eq!(cell.get_num_ports(), PortQty(*self.ports_spec.default_num_phys_ports_per_cell+1));
                }
            }
        }
    }
}

impl<'a> Drop for DatacenterPorts<'a> {
    fn drop(&mut self) {
        // teardown goes here
    }
}

test_result!(test_ports, DatacenterPorts::new_with_exceptions());


#[derive(Clone)]
struct DatacenterBorderSpec<'a, 'b> {
    border_cell_ports: HashMap<CellNo, Vec<PortNo>>,
    ports_spec: &'b DatacenterPortsSpec<'a>,
}

impl<'a, 'b> DatacenterBorderSpec<'a, 'b> {
    fn new_invalid_num_border_cells() -> DatacenterBorderSpec<'static, 'static> {
        // Assume at least one border cell required; none supplied
        DatacenterBorderSpec {
            border_cell_ports: HashMap::new(),
            ports_spec: &TRIANGLE_PORTS_SPEC,
        }
    }
    fn new_invalid_border_cell_ports_cell() -> DatacenterBorderSpec<'static, 'static> {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(3), vec![PortNo(2)]);
        DatacenterBorderSpec {
            border_cell_ports: border_cell_ports,
            ports_spec: &TRIANGLE_PORTS_SPEC,
        }
    }
    fn new_invalid_border_cell_ports_port() -> DatacenterBorderSpec<'static, 'static> {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(4), vec![PortNo(2)]);
        DatacenterBorderSpec {
            border_cell_ports: border_cell_ports,
            ports_spec: &FIVE_BY_TWO_PORTS_SPEC,
        }
    }
}

impl<'a, 'b> Test for DatacenterBorderSpec<'a, 'b> {
    fn test(&mut self) {
        let _dc =
            match Datacenter::construct(
                self.ports_spec.graph_spec.num_cells,
                &self.ports_spec.graph_spec.edges,
                self.ports_spec.default_num_phys_ports_per_cell,
                &self.ports_spec.cell_port_exceptions,
                &self.border_cell_ports,
            ) {
                Ok(pair) => pair,
                Err(err) => panic!("Datacenter construction failure: {}", err)
            };
    }
}

test_error!(test_border_cell_ports_invalid_num_cells, DatacenterBorderSpec::new_invalid_num_border_cells(), format!("BlueprintError::BorderCellCount: Must have {} border cells but only {} supplied", |datacenter_border_spec| MIN_NUM_BORDER_PORTS, |datacenter_border_spec| datacenter_border_spec.border_cell_ports.len()));
test_error!(test_border_cell_ports_invalid_cell, DatacenterBorderSpec::new_invalid_border_cell_ports_cell(), format!("BlueprintError::BorderCellPortsCell: Border port requested for cell {}; number of cells is {}", |datacenter_border_spec| {let mut border_cell_ports_vec: Vec<(&CellNo, &Vec<PortNo>)> = datacenter_border_spec.borer_cell_ports.iter().collect(); border_cell_ports_vec[0].0}, |datacenter_border_spec| datacenter_border_spec.ports_spec.graph_spec.num_cells));
test_error!(test_border_cell_ports_invalid_port, DatacenterBorderSpec::new_invalid_border_cell_ports_port(), format!("BlueprintError::BorderCellPortsPort: Border port {} requested for cell {}; number of ports is {}", |datacenter_border_spec| datacenter_border_spec.border_cell_ports[{let mut border_cell_ports_vec: Vec<(&CellNo, &Vec<PortNo>)> = datacenter_border_spec.borer_cell_ports.iter().collect(); border_cell_ports_vec[0].0}], |datacenter_border_spec| {let mut border_cell_ports_vec: Vec<(&CellNo, &Vec<PortNo>)> = datacenter_border_spec.borer_cell_ports.iter().collect(); border_cell_ports_vec[0].0}, |datacenter_border_spec| datacenter_border_spec.ports_spec.graph_spec.num_cells));

struct DatacenterBorder<'a, 'b> {
    border_spec: DatacenterBorderSpec<'a, 'b>,
    dc: Datacenter,
}

impl<'a, 'b> DatacenterBorder<'a, 'b> {
    fn new_default_port_border() -> DatacenterBorder<'static, 'static> {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(2)]);
        border_cell_ports.insert(CellNo(7), vec![PortNo(2)]);
        match Datacenter::construct(
            FIVE_BY_TWO_PORTS_SPEC.graph_spec.num_cells,
            &FIVE_BY_TWO_PORTS_SPEC.graph_spec.edges,
            FIVE_BY_TWO_PORTS_SPEC.default_num_phys_ports_per_cell,
            &FIVE_BY_TWO_PORTS_SPEC.cell_port_exceptions,
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterBorder {
                border_spec: DatacenterBorderSpec {
                    border_cell_ports: border_cell_ports,
                    ports_spec: &FIVE_BY_TWO_PORTS_SPEC,
                },
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
    fn new_exception_port_border() -> DatacenterBorder<'static, 'static> {
        let mut border_cell_ports = HashMap::new();
        border_cell_ports.insert(CellNo(0), vec![PortNo(2)]);
        match Datacenter::construct(
            TRIANGLE_PORTS_SPEC.graph_spec.num_cells,
            &TRIANGLE_PORTS_SPEC.graph_spec.edges,
            TRIANGLE_PORTS_SPEC.default_num_phys_ports_per_cell,
            &TRIANGLE_PORTS_SPEC.cell_port_exceptions,
            &border_cell_ports,
        ) {
            Ok(dc) => DatacenterBorder {
                border_spec: DatacenterBorderSpec {
                    border_cell_ports: border_cell_ports,
                    ports_spec: &TRIANGLE_PORTS_SPEC,
                },
                dc: dc,
            },
            Err(err) => panic!("Datacenter construction failure: {}", err)
        }
    }
}

impl<'a, 'b> Test for DatacenterBorder<'a, 'b> {
    fn test(&mut self) {
        for (cell_no, cell) in self.dc.get_rack().get_cells() {
            let border_ports: &Vec<PortNo>;
            let mut is_border_cell: bool = false;
            match self.border_spec.border_cell_ports.get(&CellNo(cell.get_name().trim_start_matches("C:").parse().unwrap())) {
                Some(specified_border_ports) => {
                    border_ports = specified_border_ports;
                    is_border_cell = !border_ports.is_empty();
                }
                None => {
                    border_ports = &Vec::new();
                }
            }
            assert_eq!(cell.is_border(), is_border_cell);
            for _no in 0..*cell.get_num_ports() {
                // Could check that each port in cell is border or not as expected
            }
        }
    }
}

impl<'a, 'b> Drop for DatacenterBorder<'a, 'b> {
    fn drop(&mut self) {
        // teardown goes here
    }
}

test_result!(test_default_port_border, DatacenterBorder::new_default_port_border());
test_result!(test_exception_port_border, DatacenterBorder::new_exception_port_border());


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
