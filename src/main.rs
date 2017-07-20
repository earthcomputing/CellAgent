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
mod utility;
mod vm;

use std::{thread, time};
use std::sync::mpsc::channel;

use config::{NCELLS,NPORTS,NLINKS};
use datacenter::{Datacenter};
use ecargs::{ECArgs};
use message_types::{OutsideToPort, OutsideFromPort, PortToOutside, PortFromOutside};
use noc::Noc;

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
	let (mut dc, join_handles) = build_datacenter(nports, ncells)?;
	control(&mut dc)?;
	for handle in join_handles {
		let _ = handle.join();
	};
	Ok(())
}
fn control(dc: &mut Datacenter) -> Result<()> {
	let (outside_to_port, port_from_outside): (OutsideToPort, OutsideFromPort) = channel();
	let (port_to_outside, outside_from_port): (PortToOutside, PortFromOutside) = channel();
	dc.connect_to_noc(port_to_outside, port_from_outside)?;
	let noc = Noc::new();
	noc.initialize(outside_to_port, outside_from_port)?;
	Ok(())
}
fn build_datacenter(nports: u8, ncells: usize) -> Result<(Datacenter, Vec<thread::JoinHandle<()>>)> {
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![(0,1),(1,2),(1,6),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(2,3),(2,7),(3,8),(4,9)];
	let mut dc = Datacenter::new();
	let join_handles = dc.initialize(ncells, nports, edges)?;
	let nap = time::Duration::from_millis(1000);
	thread::sleep(nap);
	println!("{}", dc);
	println!("All links broken");
	Ok((dc, join_handles))
}
fn write_err(e: Error) -> Result<()> { 
	use ::std::io::Write;
	let stderr = &mut ::std::io::stderr();
	let _ = writeln!(stderr, "Main: {}", e);
	for e in e.iter().skip(1) {
		let _ = writeln!(stderr, "Caused by: {}", e);
	}
	if let Some(backtrace) = e.backtrace() {
		let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
	}
	//::std::process::exit(1);
	Err(e)	
}
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		Send(::message_types::OutsidePortError);
	}
	links {
		DatacenterError(datacenter::Error, datacenter::ErrorKind);
		Message(::message::Error, ::message::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		Noc(::noc::Error, ::noc::ErrorKind);
		Packet(::packet::Error, ::packet::ErrorKind);
	}
	errors {
		Control
	}
}
