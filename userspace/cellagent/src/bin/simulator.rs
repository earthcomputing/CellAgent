#[macro_use] extern crate failure;

use std::{convert::TryFrom,
          io::{stdin, stdout, Read, Write},
          fs::{File,},
          collections::{HashSet},
};

use ec_fabrix::blueprint::{Blueprint};
use ec_fabrix::config::{CONFIG};
use ec_fabrix::datacenter::{Datacenter, ApplicationToNoc};
use ec_fabrix::gvm_equation::{GvmEqn};
use ec_fabrix::link::Link;
use ec_fabrix::rack::{EdgeConnection, CellInteriorConnection};
use ec_fabrix::uptree_spec::{AllowedTree, ContainerSpec, Manifest, UpTreeSpec, VmSpec};
use ec_fabrix::utility::{CellConfig, CellNo, PortNo, S, print_hash_map, sleep};

fn main() -> Result<(), Error> {
    let _f = "main";
    let output_file_name = format!("{}{}.json", CONFIG.output_dir_name, CONFIG.output_file_name);
    println!("\nMulticell trace and debug output to file {}", output_file_name);
    println!("{} Quenching of Discover messages", CONFIG.quench);
    println!("Delaying HelloMsg {} seconds", CONFIG.race_sleep);
    if CONFIG.breadth_first {
        println!("Breadth first Discover");
    } else {
        println!("Stochastic Discover");
    }
    println!("Wait until {} trees seen or {} hello messages before sending discover message", CONFIG.min_trees, CONFIG.min_hello);
    if CONFIG.replay {
        println!("\nReplay from trace");
    }
    println!("\nMain: {} ports for each of {} cells", CONFIG.num_ports_per_cell , CONFIG.num_cells);
    let mut dc =
        match Datacenter::construct(Blueprint::new(CONFIG.num_cells,
                                                   &CONFIG.edge_list,
                                                   CONFIG.num_ports_per_cell,
                                                   &CONFIG.cell_port_exceptions,
                                                   &CONFIG.border_cell_ports,)?) {
            Ok(dc) => dc,
            Err(err) => panic!("Datacenter construction failure: {}", err)
        };
    if false { deployment_demo()?; }    // Demonstrate features of deployment spec
    if CONFIG.auto_break.is_some() { break_link(&mut dc)?; }
    loop {
        stdout().write(b"\nType:
            d to print datacenter
            c to print cells
            l to print links
            p to print forwarding table
            x to exit program\n\n").context(MainError::Chain { func_name: "run", comment: S("") })?;
        let mut print_opt = String::new();
        stdin().read_line(&mut print_opt).context(MainError::Chain { func_name: _f, comment: S("") })?;
        if print_opt.len() > 1 {
            match print_opt.trim() {
                "d" => {
                    println!("{}", dc.get_rack());
                    Ok(())
                },
                "c" => show_ca(&dc),
                "l" => break_link(&mut dc),
                "p" => show_pe(&dc),
                "x" => std::process::exit(0),
                _   => {
                    println!("Invalid input {}", print_opt);
                    Ok(())
                }
            }?;
        }
    }
}
fn show_ca(dc: &Datacenter) -> Result<(), Error> {
    let rack = dc.get_rack();
    let cells = rack.get_cells();
    print_hash_map(&rack.get_cell_ids());
    let _ = stdout().write(b"Enter cell to display cell\n")?;
    let cell_no = read_int()?;
    cells.get(&CellNo(cell_no))
        .map_or_else(|| println!("{} is not a valid input", cell_no),
                     |cell| {
                         println!("{}", cell);
                     });
    Ok(())
}
fn show_pe(dc: &Datacenter) -> Result<(), Error> {
    let rack = dc.get_rack();
    let cells = rack.get_cells();
    print_hash_map(&rack.get_cell_ids());
    let _ = stdout().write(b"Enter cell to display forwarding table\n")?;
    let cell_no = read_int()?;
    cells.get(&CellNo(cell_no))
        .map_or_else(|| println!("{} is not a valid input", cell_no),
                     |cell| {
                         println!("{}", cell.get_cell_agent().get_cmodel().get_packet_engine());
                     });
    Ok(())
}
fn break_link(dc: &mut Datacenter) -> Result<(), Error> {
    let rack = dc.get_rack_mut();
    // Changed this (for the time being, at least) to use an edge_connection (with ports) instead of an edge.  We may want to look up and break all connections for an edge
    let edge_connection: EdgeConnection = match CONFIG.auto_break {
        Some(edge_connection) => {
            // TODO: Wait until discover is done before automatically breaking link, should be removed
            println!("---> Sleeping to let discover finish before automatically breaking link");
            sleep(6);
            println!("---> Automatically break link {}", edge_connection);
            edge_connection
        },
        None => {
            let link_ids = rack.get_link_ids();
            print_hash_map(&link_ids);
            let _ = stdout().write(b"Enter first cell number of link to break\n")?;
            let left_cell = usize::try_from(read_int()?)?;
            let _ = stdout().write(b"Enter first port number of link to break\n")?;
            let left_port = u8::try_from(read_int()?)?;
            let _ = stdout().write(b"Enter second cell number of link to break\n")?;
            let rite_cell = usize::try_from(read_int()?)?;
            let _ = stdout().write(b"Enter second port number of link to break\n")?;
            let rite_port = u8::try_from(read_int()?)?;
            EdgeConnection::new(
                CellInteriorConnection::new(CellNo(left_cell), PortNo(left_port)),
                CellInteriorConnection::new(CellNo(rite_cell), PortNo(rite_port))
            )
        },
    };
    let links = rack.get_links_mut();
    links.get_mut(&edge_connection)
        .map_or_else(|| -> Result<(), Error> { println!("{} is not a valid input", edge_connection); Ok(()) },
                     |link: &mut Link| -> Result<(), Error> { link.break_link()?; Ok(()) }
        )?;
    Ok(())
}
fn read_int() -> Result<usize, Error> {
    let _f = "read_int";
    let mut char = String::new();
    stdin().read_line(&mut char)?;
    match char.trim().parse::<usize>() {
        Ok(num) => Ok(num),
        Err(_) => {
            println!("{} is not an integer.  Try again.", char.trim());
            read_int()
        }
    }
}
fn _deploy(application_to_noc: &ApplicationToNoc) -> Result<(), Error> {
    let _f = "deploy";
    stdout().write(b"Enter the name of a file containing a manifest\n").context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut filename = String::new();
    stdin().read_line(&mut filename).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut f = File::open(filename.trim()).context(MainError::Chain { func_name: "run", comment: S("") })?;
    let mut manifest = String::new();
    f.read_to_string(&mut manifest).context(MainError::Chain { func_name: "run", comment: S("") })?;
    application_to_noc.send(manifest)?;
    Ok(())
}
fn deployment_demo() -> Result<(), Error> {
    let mut eqns = HashSet::new();
    eqns.insert(GvmEqn::Recv("true"));
    eqns.insert(GvmEqn::Send("true"));
    eqns.insert(GvmEqn::Xtnd("hops<7"));
    eqns.insert(GvmEqn::Save("false"));
//  let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
    let up_tree1 = UpTreeSpec::new("test1", vec![0, 0, 0, 2, 2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree2 = UpTreeSpec::new("test2", vec![1, 1, 0, 1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let allowed_tree1 = AllowedTree::new("foo");
    let allowed_tree2 = AllowedTree::new("bar");
    let c1 = ContainerSpec::new("c1", "D1", vec!["param1"], &[allowed_tree1.clone(), allowed_tree2.clone()]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c2 = ContainerSpec::new("c2", "D1", vec!["param1","param2"], &[allowed_tree1.clone()]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c3 = ContainerSpec::new("c3", "D3", vec!["param3"], &[allowed_tree1.clone()]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c4 = ContainerSpec::new("c4", "D2", vec![], &[allowed_tree1.clone(), allowed_tree2.clone()]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c5 = ContainerSpec::new("c5", "D2", vec![], &[]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let c6 = ContainerSpec::new("c6", "D3", vec!["param4"], &[allowed_tree1.clone()]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let vm_spec1 = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
                               &vec![allowed_tree1.clone(), allowed_tree2.clone()], vec![&c1, &c2, &c4, &c5, &c5], vec![&up_tree1, &up_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree3 = UpTreeSpec::new("test3", vec![0, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree4 = UpTreeSpec::new("test4", vec![1, 1, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let vm_spec2 = VmSpec::new("vm2", "RedHat",  CellConfig::Large,
                               &vec![allowed_tree1.clone()], vec![&c5, &c3, &c6], vec![&up_tree3, &up_tree4]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    let up_tree_def = Manifest::new("mytest", CellConfig::Large,
                                    &AllowedTree::new("cell_tree"),
                                    &[allowed_tree1, allowed_tree2],
                                    vec![&vm_spec1, &vm_spec2], vec![&up_tree3]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
    println!("{}", up_tree_def);
    Ok(())
}
// Errors
use failure::{Error, ResultExt};

#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "MainError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "MainError::Console {} {} is not a valid input {}", func_name, input, comment)]
    Console { func_name: &'static str, input: String, comment: String },
    #[fail(display = "MainError::Kafka {} Kafka producer undefined", func_name)]
    Kafka { func_name: &'static str}
}
