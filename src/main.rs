#![deny(unused_must_use)]
#![recursion_limit="1024"]
#[macro_use]
extern crate error_chain;
extern crate eval;
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
use std::fs::File;
use std::sync::mpsc::channel;
use std::collections::HashMap;

use blueprint::Blueprint;
use config::{NCELLS, NPORTS, NLINKS, OUTPUT_FILE_NAME, PHYSICAL_UP_TREE_NAME, CellNo, CellType, Edge, PortNo};
use ecargs::{ECArgs};
use message_types::{OutsideToNoc, NocFromOutside};
use noc::Noc;
use uptree_spec::{DeploymentSpec, UpTreeSpec, VmSpec};

fn main() {
	if let Err(ref e) = run() {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Error: {}", e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);

		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
		::std::process::exit(1);
	}
	println!("Main exit");
}

fn run() -> Result<()> {
	println!("Multicell Routing: Output to file {} set in config.rs", OUTPUT_FILE_NAME);
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
	println!("Main: {} ports for each of {} cells", *nports, *ncells);
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![is2e(0,1),is2e(1,2),is2e(1,6),is2e(3,4),is2e(5,6),is2e(6,7),is2e(7,8),is2e(8,9),is2e(0,5),is2e(2,3),is2e(2,7),is2e(3,8),is2e(4,9)];
	let exceptions = HashMap::new();
	let mut border = HashMap::new();
	border.insert(CellNo(2), vec![PortNo{v:2}]);
	border.insert(CellNo(7), vec![PortNo{v:2}]);
	let blueprint = Blueprint::new(CellType::Physical, ncells, nports, edges, exceptions, border)?;
	println!("{}", blueprint);
	let up_tree1 = UpTreeSpec::new("test1", vec![0, 0, 0, 2, 2])?;
	let up_tree2 = UpTreeSpec::new("test2", vec![1, 1, 0, 1])?;
	let vm_spec1 = VmSpec::new("vm1", vec!["foo", "bar"], vec!["c1", "c2", "c3", "c2", "c4"], vec![up_tree1, up_tree2])?;
	let up_tree3 = UpTreeSpec::new("test3", vec![0, 0])?;
	let up_tree4 = UpTreeSpec::new("test4", vec![1, 1, 0])?;
	let vm_spec2 = VmSpec::new("vm2", vec!["foo"], vec!["c5", "c3", "c6"], vec![up_tree3.clone(), up_tree4])?;
	let up_tree_def = DeploymentSpec::new("tree1", vec!["foo", "bar"], vec![vm_spec1, vm_spec2], vec![up_tree3])?;
	println!("{}", up_tree_def);
	return Ok(());
	let (outside_to_noc, noc_from_outside): (OutsideToNoc, NocFromOutside) = channel();
	let noc = Noc::new(PHYSICAL_UP_TREE_NAME, CellType::Physical)?;
	let _ = noc.initialize(blueprint, noc_from_outside)?;
	loop {
		stdout().write(b"Enter filename specifying an up tree\n")?;
		let mut filename = String::new();
		let _ = stdin().read_line(&mut filename)?;
		let mut f = File::open(filename.trim())?;
		let mut uptree_spec = String::new();
		let _ = f.read_to_string(&mut uptree_spec)?;
		outside_to_noc.send(uptree_spec)?;
	}
}
fn is2e(i: usize, j: usize) -> Edge { Edge { v: (CellNo(i),CellNo(j)) } }
// Errors
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
