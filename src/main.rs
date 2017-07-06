#![deny(unused_must_use)]
#![recursion_limit = "1024"]
extern crate crossbeam;
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
mod tenant;
mod traph;
mod utility;
mod vm;
use std::{thread, time};
use crossbeam::Scope;
use config::{NCELLS,NPORTS,NLINKS};
use container::Service;
use datacenter::{Datacenter};
use ecargs::{ECArgs};
use message::{Message, SetupVMsMsg};
use packet::Packetizer;

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
	crossbeam::scope( |scope| -> Result<()> { 
		match build_datacenter(scope, nports, ncells) {
			Ok(dc) => control(scope, dc),
			Err(e) => write_err(e)
		}
	})?;
	Ok(())
}
fn control(scope: &Scope, dc: Datacenter) -> Result<()> {
	//let noc = Noc::new(dc);
	//noc.initialize();
	Ok(())
}
fn setup_vms(outside_to_port: message_types::OutsideToPort) -> Result<()> {
	let msg = SetupVMsMsg::new("NocMaster", vec![vec![Service::NocMaster]])?;
	let other_index = 0;
	let direction = msg.get_header().get_direction();
	let bytes = Packetizer::serialize(&msg)?;
	let packets = Packetizer::packetize(bytes, direction, other_index)?;
	for packet in packets.iter() {
		//outside_to_port.send(**packet)?;
	}
	Ok(())
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
fn build_datacenter(scope: &crossbeam::Scope, nports: u8, ncells: usize) -> Result<Datacenter>{
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![(0,1),(1,2),(1,6),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(2,3),(2,7),(3,8),(4,9)];
	let dc = Datacenter::new(scope, ncells, nports, edges)?;
	let nap = time::Duration::from_millis(1000);
	thread::sleep(nap);
	println!("{}", dc);
	Ok(dc)
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
		Packet(::packet::Error, ::packet::ErrorKind);
	}
	errors {
		Control
	}
}
