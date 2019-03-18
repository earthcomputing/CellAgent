#![deny(unused_must_use)]
//#![warn(rust_2018_idioms)]
#![recursion_limit="1024"]
#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

mod app_message;
mod app_message_formats;
mod blueprint;
mod config;
mod container;
mod dal;
mod dumpstack;
mod errors;
mod gvm_equation;
mod name;
mod noc;
mod service;
mod tenant;
mod uptree_spec;
mod utility;
mod uuid_ec;

use std::{io::{stdin, stdout, Read, Write},
          collections::{HashMap, HashSet},
          fs::{File, OpenOptions},
          sync::mpsc::channel};

use crate::app_message_formats::{ApplicationFromNoc, ApplicationToNoc, NocFromApplication, NocToApplication};
use crate::blueprint::{Blueprint, CellNo, is2e};
use crate::config::{OUTPUT_FILE_NAME, QUENCH,
                    CellQty, CellConfig, PortNo, PortQty};
use crate::gvm_equation::{GvmEqn};
use crate::noc::Noc;
use crate::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use crate::utility::{print_vec, S, TraceHeader};

fn main() -> Result<(), Error> {
    let _f = "main";
    println!("Multicell Routing: Output to file {} (set in config.rs)", OUTPUT_FILE_NAME);
    println!("{:?} Quenching of Discover messages", QUENCH);
    let _ = OpenOptions::new().write(true).truncate(true).open(OUTPUT_FILE_NAME);
    let cell_port_exceptions = HashMap::new();
    let mut border_cell_ports = HashMap::new();
    border_cell_ports.insert(CellNo(0), vec![PortNo(2)]);
    let blueprint = Blueprint::new(CellQty(3), &vec![is2e(0,1), is2e(1,2), is2e(0,2)], PortQty(3), &cell_port_exceptions, &border_cell_ports).context(MainError::Chain { func_name: "run", comment: S("") })?;
    println!("{}", blueprint);
    if false { deployment_demo()?; }    // Demonstrate features of deployment spec
    let (application_to_noc, noc_from_application): (ApplicationToNoc, NocFromApplication) = channel();
    let (noc_to_application, _application_from_noc): (NocToApplication, ApplicationFromNoc) = channel();
    let mut noc = Noc::new(noc_to_application)?;
    let (port_to_noc, port_from_noc) = noc.initialize(&blueprint, noc_from_application).context(MainError::Chain { func_name: "run", comment: S("") })?;
    deploy(&application_to_noc.clone())?; /* Deploy Echo */
    /* Send Ping request */
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
fn deploy(application_to_noc: &ApplicationToNoc) -> Result<(), Error> {
    stdout().write(b"Enter the name of a file containing a manifest\n").context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut filename = String::new();
    stdin().read_line(&mut filename).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut f = File::open(filename.trim()).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut manifest = String::new();
    f.read_to_string(&mut manifest).context(MainError::Chain { func_name: "run", comment: S("") })?;
    application_to_noc.send(manifest)?;
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
