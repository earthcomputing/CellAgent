use std::fmt;
use config::{MAX_PORTS, CellNo, LinkNo, PortNo};

#[derive(Debug)]
pub struct ECArgs {
	nports: PortNo,
	ncells: CellNo,
	nlinks: LinkNo,
}
impl ECArgs {
	pub fn new(ncells: CellNo, nports: PortNo, nlinks: CellNo) -> Result<ECArgs> {
		if nports.v < (MAX_PORTS.v - 1) {
			Ok(ECArgs { nports: nports as PortNo, ncells: ncells, nlinks: LinkNo{v:nlinks} })
		} else {
			Err(ErrorKind::NumberPorts(nports).into())
		}	
	}
//	pub fn get_nports(&self) -> u8 { return self.nports }
//	pub fn get_ncells(&self) -> usize { return self.ncells }
//	pub fn get_nlinks(&self) -> usize { return self.nlinks }
	pub fn get_args(&self) -> (CellNo, PortNo) { (self.ncells, self.nports) } 
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
			self.ncells.v, self.nports.v, self.nlinks.v.v);
		s
	}
}
impl fmt::Display for ECArgs { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
error_chain! {
	errors { 
		NumberPorts(n: PortNo) {
			display("You asked for {} ports, but only {} are allowed", n.v, MAX_PORTS.v)
			
		}
	}
}
