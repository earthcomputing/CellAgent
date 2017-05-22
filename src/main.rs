extern crate crossbeam;
extern crate serde;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
mod cellagent;
mod config;
mod datacenter;
mod ecargs;
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
use std::error::Error;
use std::{thread, time};
use config::{NCELLS,NPORTS,NLINKS};
use datacenter::{Datacenter};
use ecargs::{ECArgs};
use name::{CellID};
use tenant::{Tenant};

fn main() {
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
	let (ncells,nports) = ecargs.get_args();
	println!("Main: {} ports for each of {} cells", nports, ncells);
	crossbeam::scope( |scope| { 
		if nports > 0 {
			match build_datacenter(scope, nports, ncells) {
				Ok(_) => println!("Normal Exit"),
				Err(e) => match e.cause() {
					Some(c) => println!("Abnormal Exit: {} {}", e, c),
					None => println!("Abnormal Exit: {}", e)
				}
			}
		} else { 
			match tests() {
				Ok(_) => println!("Normal Exit"),
				Err(e) => match e.cause() {
					Some(c) => println!("Abnormal Exit: {} {}",e, c),
					None => println!("Abnormal Exit: {}",e)
				}
			}
		}
	//test_mut(); // Trying to figure out an issue with mutability
	});
	println!("Main exit");
}
#[deny(unused_must_use)]
fn build_datacenter<'a>(scope: &crossbeam::Scope, nports: u8, ncells: usize) -> Result<Datacenter<'a>,Box<Error>>{
	//let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let edges = vec![(0,1),(1,2),(1,6),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(2,3),(2,7),(3,8),(4,9)];
	let dc = Datacenter::new(scope, ncells, nports, edges)?;
	let nap = time::Duration::from_millis(1000);
	thread::sleep(nap);
	Ok(dc)
}
// Other tests
fn tests() -> Result<(),Box<Error>> {
	try!(CellID::new(42));
	let mut base_tenant = try!(Tenant::new("Base", 100, None));
	println!("Main: {:?}", base_tenant);
	let mut sub_tenant = try!(base_tenant.create_subtenant("A", 75));
	println!("Main: {:?}", sub_tenant);
	let sub_sub_tenant = try!(sub_tenant.create_subtenant("B", 25));
	println!("Main: {:?}", sub_sub_tenant);
	try!(base_tenant.create_subtenant("B",20));
	let children = base_tenant.get_children();
	for child in children {
		println!("Main: child of ({:?}) is ({:?})", base_tenant, child);
	}
	Ok(())
}