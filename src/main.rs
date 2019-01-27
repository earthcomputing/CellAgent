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
mod name_type;
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
          collections::{HashMap, HashSet},
          fs::{File, OpenOptions},
          sync::mpsc::channel};

use crate::blueprint::Blueprint;
use crate::config::{AUTO_BREAK, NCELLS, NPORTS, NLINKS, OUTPUT_FILE_NAME, QUENCH,
             get_edges, CellNo, PortNo};
use crate::datacenter::Datacenter;
use crate::ecargs::{ECArgs};
use crate::gvm_equation::{GvmEqn};
use crate::message_types::{OutsideFromNoc, OutsideToNoc, NocFromOutside, NocToOutside};
use crate::nalcell::CellConfig;
use crate::noc::Noc;
use crate::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use crate::utility::{print_vec, S, TraceHeader};

fn main() -> Result<(), Error> {
    let _f = "main";
    println!("Multicell Routing: Output to file {} (set in config.rs)", OUTPUT_FILE_NAME);
    println!("{:?} Quenching of Discover messages", QUENCH);
    /* Can't get records from main() to show up in trace file
        let (rows, cols, geometry) = config::get_geometry();
        {
            // For reasons I can't understand, the trace record doesn't show up when generated from main.
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "trace_schema" };
            let trace = json!({ "schema_version": SCHEMA_VERSION, "ncells": NCELLS, "rows": rows, "cols": cols });
            let _ = dal::add_to_trace(trace_header, TraceType::Trace, trace_params,&trace, _f);
        }
    */
    let _ = OpenOptions::new().write(true).truncate(true).open(OUTPUT_FILE_NAME);
    /* Doesn't work when debugging in Eclipse
    let args: Vec<String> = env::args().collect();
    println!("Main: args {:?}",args);
    let ecargs = match ECArgs::get_args(args) {
        Ok(e) => e,
        Err(err) => panic!("Argument Error: {}",err)
    };
*/
    let ecargs = match ECArgs::new(NCELLS, NPORTS, *NLINKS) {
        Ok(a) => a,
        Err(err) => panic!("Argument Error: {}", err)
    };
    let (ncells, nports) = ecargs.get_args();
    println!("\nMain: {} ports for each of {} cells", *nports, *ncells);
    let edges = get_edges();
    let mut exceptions = HashMap::new();
    exceptions.insert(CellNo(5), PortNo(7));
    exceptions.insert(CellNo(2), PortNo(6));
    let mut border = HashMap::new();
    border.insert(CellNo(2), vec![PortNo(2)]);
    border.insert(CellNo(7), vec![PortNo(2)]);
    let blueprint = Blueprint::new(ncells, nports, edges, &exceptions, &border).context(MainError::Chain { func_name: "run", comment: S("") })?;
    println!("{}", blueprint);
    if false { deployment_demo()?; }    // Demonstrate features of deployment spec
    let (outside_to_noc, noc_from_outside): (OutsideToNoc, NocFromOutside) = channel();
    let (noc_to_outside, _outside_from_noc): (NocToOutside, OutsideFromNoc) = channel();
    let mut noc = Noc::new(noc_to_outside)?;
    let (mut dc, _) = noc.initialize(&blueprint, noc_from_outside).context(MainError::Chain { func_name: "run", comment: S("") })?;
    if AUTO_BREAK != 0 {
        println!("---> Automatically break link");
        break_link(&mut dc)?;
    }
    loop {
        stdout().write(b"\nType:
            d to print datacenter
            c to print cells
            l to print links
            p to print forwarding table
            m to deploy an application
            x to exit program\n").context(MainError::Chain { func_name: "run", comment: S("") })?;
        let mut print_opt = String::new();
        stdin().read_line(&mut print_opt).context(MainError::Chain { func_name: "run", comment: S("") })?;
        if print_opt.len() > 1 {
            match print_opt.trim() {
                "d" => {
                    println!("{}", dc);
                    Ok(())
                },
                "c" => show_ca(&dc),
                "l" => break_link(&mut dc),
                "p" => show_pe(&dc),
                "m" => deploy(&outside_to_noc.clone()),
                "x" => std::process::exit(0),
                _   => {
                    println!("Invalid input {}", print_opt);
                    Ok(())
                }
            }?;
        }
    }
}
fn show_ca(dc: &Datacenter) -> Result<(), Error> {
    let cells = dc.get_cells();
    print_vec(&dc.get_cell_ids());
    let _ = stdout().write(b"Enter cell to display cell\n")?;
    let cell_no = read_int()?;
    cells.get(cell_no)
        .map_or_else(|| println!("{} is not a valid input", cell_no),
                     |cell| {
                         println!("{}", cell);
                     });
    Ok(())
}
fn show_pe(dc: &Datacenter) -> Result<(), Error> {
    let cells = dc.get_cells();
    print_vec(&dc.get_cell_ids());
    let _ = stdout().write(b"Enter cell to display forwarding table\n")?;
    let cell_no = read_int()?;
    cells.get(cell_no)
        .map_or_else(|| println!("{} is not a valid input", cell_no),
                     |cell| {
                         println!("{}", cell.get_packet_engine());
                     });
    Ok(())
}
fn break_link(dc: &mut Datacenter) -> Result<(), Error> {
    let link_no = if AUTO_BREAK != 0 {
        // TODO: Wait until discover is done before automatically breaking link, should be removed
        crate::utility::sleep(4);
        println!("---> Automatically break link {}", AUTO_BREAK);
        AUTO_BREAK
    } else {
        let link_ids = dc.get_link_ids();
        print_vec(&link_ids);
        let _ = stdout().write(b"Enter link to break or null\n")?;
        read_int()?
    };
    let links = dc.get_links_mut();
    links.get_mut(link_no)
        .map_or_else(|| -> Result<(), Error> { println!("{} is not a valid input", link_no); Ok(()) },
                     |link: &mut crate::link::Link| -> Result<(), Error> { link.break_link()?; Ok(()) }
        )?;
    Ok(())
}
fn read_int() -> Result<usize, Error> {
    let _f = "read_int";
    let mut char = String::new();
    stdin().read_line(&mut char)?;
    match char.trim().parse::<usize>() {
        Ok(num) => Ok(num),
        Err(_) => {
            println!("{} is not an integer.  Try again.", char.trim());
            read_int()
        }
    }
}
fn deploy(outside_to_noc: &OutsideToNoc) -> Result<(), Error> {
    stdout().write(b"Enter the name of a file containing a manifest\n").context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut filename = String::new();
    stdin().read_line(&mut filename).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut f = File::open(filename.trim()).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut manifest = String::new();
    f.read_to_string(&mut manifest).context(MainError::Chain { func_name: "run", comment: S("") })?;
    outside_to_noc.send(manifest)?;
    Ok(())
}
fn deployment_demo() -> Result<(), Error> {
    let mut eqns = HashSet::new();
    eqns.insert(GvmEqn::Recv("true"));
    eqns.insert(GvmEqn::Send("true"));
    eqns.insert(GvmEqn::Xtnd("hops<7"));
    eqns.insert(GvmEqn::Save("false"));
//  let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
    let up_tree1 = UpTreeSpec::new("test1", vec![0, 0, 0, 2, 2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree2 = UpTreeSpec::new("test2", vec![1, 1, 0, 1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let allowed_tree1 = &AllowedTree::new("foo");
    let allowed_tree2 = &AllowedTree::new("bar");
    let c1 = ContainerSpec::new("c1", "D1", vec!["param1"], &[allowed_tree1, allowed_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c2 = ContainerSpec::new("c2", "D1", vec!["param1","param2"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c3 = ContainerSpec::new("c3", "D3", vec!["param3"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c4 = ContainerSpec::new("c4", "D2", vec![], &[allowed_tree1, allowed_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c5 = ContainerSpec::new("c5", "D2", vec![], &[]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c6 = ContainerSpec::new("c6", "D3", vec!["param4"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let vm_spec1 = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                               &vec![allowed_tree1, allowed_tree2], vec![&c1, &c2, &c4, &c5, &c5], vec![&up_tree1, &up_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree3 = UpTreeSpec::new("test3", vec![0, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree4 = UpTreeSpec::new("test4", vec![1, 1, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let vm_spec2 = VmSpec::new("vm2", "RedHat",  CellConfig::Large,
                               &vec![allowed_tree1], vec![&c5, &c3, &c6], vec![&up_tree3, &up_tree4]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree_def = Manifest::new("mytest", CellConfig::Large,
                                    &AllowedTree::new("cell_tree"),
                                    &[allowed_tree1, allowed_tree2],
                                    vec![&vm_spec1, &vm_spec2], vec![&up_tree3]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    println!("{}", up_tree_def);
    Ok(())
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "MainError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "MainError::Console {} {} is not a valid input {}", func_name, input, comment)]
    Console { func_name: &'static str, input: String, comment: String },
    #[fail(display = "MainError::Kafka {} Kafka producer undefined", func_name)]
    Kafka { func_name: &'static str}
}
