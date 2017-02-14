mod cellagent;
mod config;
mod datacenter;
mod ecargs;
mod link;
mod nalcell;
mod name;
mod noc;
mod packet;
mod port;
mod tenant;
mod utility;
mod vm;
use std::error::Error;
use config::{NCELLS,NPORTS,NLINKS};
use datacenter::{Datacenter,DatacenterError};
use ecargs::{ECArgs};
use nalcell::{NalCell};
use name::{CellID};
use tenant::{Tenant};
use vm::VirtualMachine;

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
	if nports > 0 {
		match build_datacenter(nports, ncells) {
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
}
fn build_datacenter(nports: u8, ncells: usize) -> Result<(),Box<Error>>{
	let edges = vec![(0,1),(1,2),(2,3),(3,4),(5,6),(6,7),(7,8),(8,9),(0,5),(1,6),(2,7),(3,8),(4,9)];
	let mut cells = Vec::new();
	let mut dc = try!(Datacenter::new(&mut cells, ncells, nports, edges));
	//try!(dc.build(ncells, nports, edges));
	println!("{}",dc);
	Ok(())
}
// Test mutability
#[derive(Debug)]
pub struct Test<'a> { pub x: &'a str }
impl<'a> Test<'a> { pub fn new() -> Test<'a> { Test { x: "foo" } } }
fn test_mut() {
	let mut vm = Test::new();
	let vm_mut = &mut vm;
	f1(vm_mut);
	f2(vm_mut);	
}
fn f1<'a>(dc: &'a mut Test    ) { println!("f1: {:?}", dc); } // Compiles
//fn f1<'a>(dc: &'a mut Test<'a>) { println!("f1: {:?}", dc); } // Doesn't
fn f2(dc: &mut Test) { println!("f2: {:?}", dc); }
// Other tests
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