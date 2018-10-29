mod utility;
mod config;
mod name;
mod tenant;
use std::env;
use std::error::Error;
use utility::get_first_arg;
use name::{CellID};
use tenant::{Tenant};

fn main() {
	match cell_setup() {
		Ok(_) => println!("Normal termination"),
		Err(e) => println!("Abnormal termination {}", e)
	}
}
// A function defined so I can use try! to handle errors
fn cell_setup() -> Result<(),Box<Error>> {
	println!("Multicell Routing");
	let args: Vec<String> = env::args().collect();
	let n_physical_ports = match get_first_arg(args) {
		Some(n) => n,
		None    => 8
	};
	println!("Main: {} ports for this cell", n_physical_ports);
	let cell_id = try!(CellID::new("foo"));
	let cell_id2 = try!(CellID::new("bar")); 
	let cell_id_clone = cell_id.clone();
	let x = cell_id;
	println!("Main: cell_id {:?} {:?} {:?} {:?}", cell_id, cell_id == cell_id_clone, 
		x == cell_id, cell_id == cell_id2);
	let y = cell_id.add_component("bar");
	println!("Main: {:?}", try!(y));
	let mut base_tenant = try!(Tenant::new("Base", 100, None));
	println!("Main: {:?}", base_tenant);
	let mut sub_tenant = try!(base_tenant.create_subtenant("A", 75));
	println!("Main: {:?}", sub_tenant);
	let sub_sub_tenant = try!(sub_tenant.create_subtenant("B", 25));
	println!("Main: {:?}", sub_sub_tenant);
	try!(base_tenant.create_subtenant("B",20));
	let children = try!(base_tenant.get_children());
	for child in children {
		println!("Main: child of ({:?}) is ({:?})", base_tenant, child);
	}
	Ok(())
}