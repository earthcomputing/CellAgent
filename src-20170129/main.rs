mod utility;
mod config;
mod name;
mod tenant;
use std::env;
use utility::get_first_arg;
use name::{CellID};
use tenant::{Tenant};

fn main() {
	println!("Multicell Routing");
	let args: Vec<String> = env::args().collect();
	let n_physical_ports = match get_first_arg(args) {
		Some(n) => n,
		None    => 8
	};
	println!("Main: {} ports for this cell", n_physical_ports);
	
	let cell_id = CellID::new("foo").expect("Main: Can't create cellID");
	let cell_id2 = CellID::new("bar").expect("Main: Can't create cellID");
	let cell_id_clone = cell_id.clone();
	let x = cell_id;
	println!("Main: cell_id {:?} {:?} {:?} {:?}", cell_id, cell_id == cell_id_clone, 
		x == cell_id, cell_id == cell_id2);
	let y = cell_id.add_component("bar");
	println!("Main: {:?}", y);
	
	let mut base_tenant = Tenant::new("Base", 100, None).expect("Main: Could not create tenant");
	println!("Main: {:?}", base_tenant);
	let mut sub_tenant = base_tenant.create_subtenant("A", 75).expect("Main: Could not create subtenant");
	println!("Main: {:?}", sub_tenant);
	let sub_sub_tenant = sub_tenant.create_subtenant("B", 25).expect("Main: Could not create subsubtenant");
	println!("Main: {:?}", sub_sub_tenant);
	let sub_tenant = base_tenant.create_subtenant("B",20).expect("Main: Could not create second subtenant");
	let children = base_tenant.get_children();
	for child in children {
		println!("Main: child of ({:?}) is ({:?})", base_tenant, child);
	}
}