#![deny(unused_must_use)]
#![recursion_limit="1024"]
#[macro_use]
extern crate error_chain;
extern crate eval;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate rand; 
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate uuid;

mod blueprint;
mod cellagent;
mod config;
mod container;
mod datacenter;
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
mod routing_table;
mod routing_table_entry;
mod service;
mod tenant;
mod traph;
mod traph_element;
mod tree;
mod uptree_spec;
mod utility;
mod vm;

use std::io::{stdin, stdout, Read, Write};
use std::collections::HashSet;
use std::fs::File;
use std::sync::mpsc::channel;
use std::collections::HashMap;

use blueprint::Blueprint;
use config::{BASE_TREE_NAME, CONTROL_TREE_NAME, NCELLS, NPORTS, NLINKS, OUTPUT_FILE_NAME, CellNo, Edge, PortNo};
use ecargs::{ECArgs};
use gvm_equation::{GvmEquation, GvmEqn, GvmVariable, GvmVariableType};
use message_types::{OutsideFromNoc, OutsideToNoc, NocFromOutside, NocToOutside};
use nalcell::CellConfig;
use noc::Noc;
use uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use utility::write_err;

fn main() {
	if let Err(e) = run() { println!("Main Error");
        write_err("*** main", e);
         ::std::process::exit(1);
	}
	println!("\nMain exit");
}
fn is2e(i: usize, j: usize) -> Edge { Edge { v: (CellNo(i),CellNo(j)) } }
use blueprint::BlueprintError;
fn run() -> Result<(), Error> {
	println!("Multicell Routing: Output to file {} (set in config.rs)", OUTPUT_FILE_NAME);
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
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![is2e(0,1),is2e(1,2),is2e(1,6),is2e(3,4),is2e(5,6),is2e(6,7),is2e(7,8),is2e(8,9),is2e(0,5),is2e(2,3),is2e(2,7),is2e(3,8),is2e(4,9)];
	let mut exceptions = HashMap::new();
	exceptions.insert(CellNo(5), PortNo{v:4});
	exceptions.insert(CellNo(2), PortNo{v:8});
	let mut border = HashMap::new();
	border.insert(CellNo(2), vec![PortNo{v:2}]);
	border.insert(CellNo(7), vec![PortNo{v:2}]);
	let blueprint = Blueprint::new(ncells, nports, edges, exceptions, border)?;
	println!("{}", blueprint);
//	deployment_demo()?; 	// Demonstrate features of deployment spec
	let (outside_to_noc, noc_from_outside): (OutsideToNoc, NocFromOutside) = channel();
	let (noc_to_outside, outside_from_noc): (NocToOutside, OutsideFromNoc) = channel();
	let noc = Noc::new(noc_to_outside)?;
	let _ = noc.initialize(&blueprint, noc_from_outside)?;
	loop {
		stdout().write(b"Enter the name of a file containing a manifest\n")?;
		let mut filename = String::new();
		let _ = stdin().read_line(&mut filename)?;
		let mut f = File::open(filename.trim())?;
		let mut manifest = String::new();
		let _ = f.read_to_string(&mut manifest)?;
		outside_to_noc.send(manifest)?;
	}
}
fn deployment_demo() -> Result<(), Error> {
	let mut eqns = HashSet::new();
	eqns.insert(GvmEqn::Recv("true"));
	eqns.insert(GvmEqn::Send("true"));
	eqns.insert(GvmEqn::Xtnd("hops<7"));
	eqns.insert(GvmEqn::Save("false"));
	let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
	let up_tree1 = UpTreeSpec::new("test1", vec![0, 0, 0, 2, 2])?;
	let up_tree2 = UpTreeSpec::new("test2", vec![1, 1, 0, 1])?;
	let ref allowed_tree1 = AllowedTree::new("foo");
	let ref allowed_tree2 = AllowedTree::new("bar");
	let c1 = ContainerSpec::new("c1", "D1", vec!["param1"], vec![allowed_tree1, allowed_tree2])?;
	let c2 = ContainerSpec::new("c2", "D1", vec!["param1","param2"], vec![allowed_tree1])?;
	let c3 = ContainerSpec::new("c3", "D3", vec!["param3"], vec![allowed_tree1])?;
	let c4 = ContainerSpec::new("c4", "D2", vec![], vec![allowed_tree1, allowed_tree2])?;
	let c5 = ContainerSpec::new("c5", "D2", vec![], vec![])?;
	let c6 = ContainerSpec::new("c6", "D3", vec!["param4"], vec![allowed_tree1])?;
	let vm_spec1 = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
		vec![allowed_tree1, allowed_tree2], vec![&c1, &c2, &c4, &c5, &c5], vec![&up_tree1, &up_tree2])?;
	let up_tree3 = UpTreeSpec::new("test3", vec![0, 0])?;
	let up_tree4 = UpTreeSpec::new("test4", vec![1, 1, 0])?;
	let vm_spec2 = VmSpec::new("vm2", "RedHat",  CellConfig::Large,
		vec![allowed_tree1], vec![&c5, &c3, &c6], vec![&up_tree3, &up_tree4])?;
	let up_tree_def = Manifest::new("mytest", CellConfig::Large, "cell_tree", vec![allowed_tree1, allowed_tree2], vec![&vm_spec1, &vm_spec2], vec![&up_tree3], gvm_eqn)?;
	println!("{}", up_tree_def);
	Ok(())
}
// Errors
use failure::{Error, Fail};
/*
error_chain! {
	foreign_links {
		Io(::std::io::Error);
		SendNoc(::message_types::OutsideNocError);
		Serialize(::serde_json::Error);
	}
	links {
		Blueprint(::blueprint::Error, ::blueprint::ErrorKind);
		Noc(::noc::Error, ::noc::ErrorKind);
		UpTree(::uptree_spec::Error, ::uptree_spec::ErrorKind);
	}
}
*/