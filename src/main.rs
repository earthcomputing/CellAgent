mod cellagent;
mod config;
mod ecargs;
mod nalcell;
mod name;
mod port;
mod tenant;
mod utility;
mod vm;
use std::env;
use std::cmp::min;
use std::error::Error;
use config::MAX_PORTS;
use ecargs::{ECArgs};
use name::{CellID};
use tenant::{Tenant};

fn main() {
	println!("Multicell Routing");
	let args: Vec<String> = env::args().collect();
	let ecargs = match ECArgs::get_args(args) {
		Ok(e) => e,
		Err(err) => panic!("Argument Error: {}",err)
	}; 
	let nports = ecargs.nports;
	let ncells = ecargs.ncells;
	println!("Main: {} ports for each of {} cells", nports, ncells);
	if nports > 0 {
	} else { 
		match tests() {
			Ok(_) => println!("Normal Exit"),
			Err(e) => match e.cause() {
				Some(c) => println!("Abnormal Exit: {} {}",e, c),
				None => println!("Abnormal Exit: {}",e)
			} 
		}
	}
}
fn tests() -> Result<(),Box<Error>> {
	let cell_id = try!(CellID::new("foo"));
	let cell_id2 = try!(CellID::new("bar")); 
	let cell_id_clone = cell_id.clone();
	let x = cell_id;
	println!("Main: cell_id {:?} {:?} {:?} {:?}", cell_id, cell_id == cell_id_clone, 
		x == cell_id, cell_id == cell_id2);
	let y = try!(cell_id.add_component("bar"));
	println!("Main: {:?}", y);
	
	let mut base_tenant = try!(Tenant::new("Base", 100, None));
	println!("Main: {:?}", base_tenant);
	let mut sub_tenant = try!(base_tenant.create_subtenant("A", 75));
	println!("Main: {:?}", sub_tenant);
	let sub_sub_tenant = try!(sub_tenant.create_subtenant("B", 25));
	println!("Main: {:?}", sub_sub_tenant);
	let sub_tenant = try!(base_tenant.create_subtenant("B",20));
	let children = base_tenant.get_children();
	for child in children {
		println!("Main: child of ({:?}) is ({:?})", base_tenant, child);
	}
	Ok(())
}