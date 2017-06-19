#![deny(unused_must_use)]
#![recursion_limit = "1024"]
extern crate crossbeam;
#[macro_use]
extern crate error_chain;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
mod cellagent;
mod config;
mod datacenter;
mod ecargs;
mod errors;
mod link;
mod message;
mod nalcell;
mod name;
mod noc;
mod packet;
mod packet_engine;
mod port;
mod routing_table;
mod routing_table_entry;
mod tenant;
mod traph;
mod utility;
mod vm;
use std::{thread, time};
use config::{NCELLS,NPORTS,NLINKS};
use datacenter::{Datacenter};
use ecargs::{ECArgs};

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
	println!("Multicell Routing");
/* Doesn't work when debugging in Eclipse
	let args: Vec<String> = env::args().collect();
	println!("Main: args {:?}",args);
	let ecargs = match ECArgs::get_args(args) {
		Ok(e) => e,
		Err(err) => panic!("Argument Error: {}",err)
	}; 
*/
	let ecargs = match ECArgs::new(NCELLS,NPORTS,NLINKS) {
		Ok(a) => a,
		Err(err) => panic!("Argument Error: {}", err)
	};
	//let cell_id = CellID::new("foo bar").chain_err(|| "testing bad name")?;
	let (ncells,nports) = ecargs.get_args();
	println!("Main: {} ports for each of {} cells", nports, ncells);
	crossbeam::scope( |scope| { 
		if let Err(ref e) = build_datacenter(scope, nports, ncells) {
			use ::std::io::Write;
			let stderr = &mut ::std::io::stderr();
			let _ = writeln!(stderr, "Error: {}", e);
			for e in e.iter().skip(1) {
				let _ = writeln!(stderr, "Caused by: {}", e);
			}
			if let Some(backtrace) = e.backtrace() {
				let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
			}
			//::std::process::exit(1);
		}
	});
	Ok(())
}
fn build_datacenter<'a>(scope: &crossbeam::Scope, nports: u8, ncells: usize) -> Result<Datacenter<'a>>{
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![(0,1),(1,2),(1,6),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(2,3),(2,7),(3,8),(4,9)];
	let dc = Datacenter::new(scope, ncells, nports, edges)?;
	let nap = time::Duration::from_millis(1000);
	thread::sleep(nap);
	println!("{}", dc);
	Ok(dc)
}
error_chain! {
	links {
		DatacenterError(datacenter::Error, datacenter::ErrorKind);
	}
}
