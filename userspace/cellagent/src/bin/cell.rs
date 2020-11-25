#[macro_use] extern crate failure;

use std::{collections::{HashSet},
          fmt,
          fs::{File, OpenOptions},
	  iter::FromIterator,
          process::{Command, Stdio},
          sync::{Arc}
};

use ec_fabrix::config::{CONFIG, PortQty};
use ec_fabrix::ecnl::{ECNL_Session};
use ec_fabrix::nalcell::{NalCell};
use ec_fabrix::utility::{CellConfig, PortNo};

fn main() -> Result<(), Error> {
    let _f = "main";
    println!("Multicell Routing: Output to file {} (set in config.rs)", CONFIG.output_file_name);
    println!("{:?} Quenching of Discover messages", CONFIG.quench);
    let _ = OpenOptions::new()
        .write(true)
        .truncate(true)
	.open(&CONFIG.output_file_name);
    let cell_name = "Carol"; /* if needed, can read cell name from config file */
    let mut wc_cmd_outer;
    let num_phys_ports_str = {
        let lspci_cmd = Command::new("lspci")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .expect("lspci failed in identifying ethernet ports");
        use std::process::*;
        use std::io::*;
        use std::os::unix::io::{AsRawFd, FromRawFd};
        unsafe {  // AHK: I don't think this block needs unsave
            let grep_cmd = Command::new("grep")
                .arg("Ethernet")
                .stdin(Stdio::from_raw_fd(lspci_cmd.stdout.unwrap().as_raw_fd()))
                .stdout(Stdio::piped())
                .spawn()
                .expect("grep failed in identifying ethernet ports");
            let wc_cmd = Command::new("wc")
                .arg("-l")
                .stdin(Stdio::from_raw_fd(grep_cmd.stdout.unwrap().as_raw_fd()))
                .stdout(Stdio::piped())
                .output()
                .expect("wc failed in identifying ethernet ports");
            wc_cmd_outer = wc_cmd;
        }
        String::from_utf8_lossy(&wc_cmd_outer.stdout)
    };
    println!("num_phys_ports: {}", num_phys_ports_str);
    let num_phys_ports : PortQty = PortQty(num_phys_ports_str.trim().parse().unwrap());
    let ecnl = Arc::new(ECNL_Session::new());
    let num_ecnl_ports = ecnl.clone().num_ecnl_ports();
    println!("Num ecnl ports: {:?} ", num_ecnl_ports);
    let border_port_list : Vec<PortNo> = (*num_ecnl_ports+1..*num_phys_ports+1)
        .map(|i| PortNo(i))
	.collect();
    let (mut nal_cell, ca_join_handle) = NalCell::new(cell_name,
                                                      num_phys_ports,
                                                      &HashSet::from_iter(border_port_list),
                                                      CellConfig::Large,
                                                      Some(ecnl.clone()),
    )?;
    nal_cell.link_ecnl_channels(ecnl)?;
    match ca_join_handle.join() {
        Ok(()) => Ok(()),
        Err(e) => Err(MainError::Chain { func_name: _f, comment: format!("{:?}", e) }.into())
    }
}

// fn deployment_demo() -> Result<(), Error> {
//     let mut eqns = HashSet::new();
//     eqns.insert(GvmEqn::Recv("true"));
//     eqns.insert(GvmEqn::Send("true"));
//     eqns.insert(GvmEqn::Xtnd("hops<7"));
//     eqns.insert(GvmEqn::Save("false"));
// //  let ref gvm_eqn = GvmEquation::new(eqns, vec![GvmVariable::new(GvmVariableType::PathLength, "hops")]);
//     let up_tree1 = UpTreeSpec::new("test1", vec![0, 0, 0, 2, 2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let up_tree2 = UpTreeSpec::new("test2", vec![1, 1, 0, 1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let allowed_tree1 = &AllowedTree::new("foo");
//     let allowed_tree2 = &AllowedTree::new("bar");
//     let c1 = ContainerSpec::new("c1", "D1", vec!["param1"], &[allowed_tree1, allowed_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let c2 = ContainerSpec::new("c2", "D1", vec!["param1","param2"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let c3 = ContainerSpec::new("c3", "D3", vec!["param3"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let c4 = ContainerSpec::new("c4", "D2", vec![], &[allowed_tree1, allowed_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let c5 = ContainerSpec::new("c5", "D2", vec![], &[]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let c6 = ContainerSpec::new("c6", "D3", vec!["param4"], &[allowed_tree1]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let vm_spec1 = VmSpec::new("vm1", "Ubuntu", CellConfig::Large,
//                                &vec![allowed_tree1, allowed_tree2], vec![&c1, &c2, &c4, &c5, &c5], vec![&up_tree1, &up_tree2]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let up_tree3 = UpTreeSpec::new("test3", vec![0, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let up_tree4 = UpTreeSpec::new("test4", vec![1, 1, 0]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let vm_spec2 = VmSpec::new("vm2", "RedHat",  CellConfig::Large,
//                                &vec![allowed_tree1], vec![&c5, &c3, &c6], vec![&up_tree3, &up_tree4]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     let up_tree_def = Manifest::new("mytest", CellConfig::Large,
//                                     &AllowedTree::new("cell_tree"),
//                                     &[allowed_tree1, allowed_tree2],
//                                     vec![&vm_spec1, &vm_spec2], vec![&up_tree3]).context(MainError::Chain { func_name: "deployment_demo", comment: S("")})?;
//     println!("{}", up_tree_def);
//     Ok(())
// }
// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "MainError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "MainError::Console {} {} is not a valid input {}", func_name, input, comment)]
    Console { func_name: &'static str, input: String, comment: String },
    #[fail(display = "MainError::Kafka {} Kafka producer undefined", func_name)]
    Kafka { func_name: &'static str}
}
