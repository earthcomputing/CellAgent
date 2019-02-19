use std::fmt;

use failure::{Error};

use crate::config::{MAX_NUM_PHYS_PORTS, CellNo, CellQty, LinkNo, PortQty};

#[derive(Debug)]
pub struct ECArgs {
    nports: PortQty,
    ncells: CellQty,
    nlinks: LinkNo,
}
impl ECArgs {
    pub fn new(ncells: CellQty, nports: PortQty, nlinks: CellNo) -> Result<ECArgs, Error> {
        if *nports <= (*MAX_NUM_PHYS_PORTS - 1) {
            Ok(ECArgs { nports: nports as PortQty, ncells, nlinks: LinkNo(CellNo(*nlinks)) })
        } else {
            Err(EcargsError::NumberPorts { nports, func_name: "new", max_num_phys_ports: MAX_NUM_PHYS_PORTS }.into())
        }
    }
//  pub fn get_nports(&self) -> u8 { return self.nports }
//  pub fn get_ncells(&self) -> usize { return self.ncells }
//  pub fn get_nlinks(&self) -> usize { return self.nlinks }
    pub fn get_args(&self) -> (CellQty, PortQty) { (self.ncells, self.nports) }
/*
    pub fn args(args: Vec<String>)-> Result<ECArgs,ECArgsError> {
        if args.len() != 3 { Err(ECArgsError::NumberArgs(NumberArgsError::new(args.len()-1,2))) }
        else {
            let nports = match args[1].parse::<i32>() {
                Ok(n) => Some(n),
                Err(_) => None
            };
            let ncells = match args[2].parse::<i32>() {
                Ok(n) => Some(n),
                Err(_) => None
            };
            let nlinks = match args[3].parse::<i32>() {
                Ok(n) => Some(n),
                Err(_) => None
            };
            if nports.is_none() {
                Err(ECArgsError::ArgType(ArgTypeError::new(&args[1], 1, "i32")))
            } else if ncells.is_none() {
                Err(ECArgsError::ArgType(ArgTypeError::new(&args[2], 2, "i32")))
            } else if nlinks.is_none() {
                Err(ECArgsError::ArgType(ArgTypeError::new(&args[3], 3, "i32")))
            } else {
                Ok(ECArgs { nports: nports.unwrap() as u8,
                            ncells: ncells.unwrap() as usize, nlinks: nlinks.unwrap() as usize })
            }
        }
    }
*/
    pub fn to_string(&self) -> String {
        let s = format!("{} cells, {} ports per cell, {} links",
            *self.ncells, *self.nports, **self.nlinks);
        s
    }
}
impl fmt::Display for ECArgs { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
#[derive(Debug, Fail)]
pub enum EcargsError {
    #[fail(display = "EcargsError::NumberPorts {}:  You asked for {:?} ports, but only {:?} are allowed", func_name, nports, max_num_phys_ports)]
    NumberPorts { func_name: &'static str, nports: PortQty, max_num_phys_ports: PortQty}
}
