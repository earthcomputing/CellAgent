use std::fmt;
use config::MAX_PORTS;

#[derive(Debug)]
pub struct ECArgs {
	nports: u8,
	ncells: usize,
	nlinks: usize,
}
impl ECArgs {
	pub fn new(ncells: usize, nports: u8, nlinks: usize) -> Result<ECArgs,ECArgsError> {
		if nports < (MAX_PORTS - 1) as u8{
			Ok(ECArgs { nports: nports as u8, ncells: ncells, nlinks: nlinks })
		} else {
			Err(ECArgsError::NumberPorts(NumberPortsError::new(nports as usize)))
		}	
	}
//	pub fn get_nports(&self) -> u8 { return self.nports }
//	pub fn get_ncells(&self) -> usize { return self.ncells }
//	pub fn get_nlinks(&self) -> usize { return self.nlinks }
	pub fn get_args(&self) -> (usize,u8) { (self.ncells, self.nports) } 
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
			self.ncells, self.nports, self.nlinks);
		s
	}
}
impl fmt::Display for ECArgs { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum ECArgsError {
	NumberArgs(NumberArgsError),
	ArgType(ArgTypeError),
	NumberPorts(NumberPortsError)
}
impl Error for ECArgsError {
	fn description(&self) -> &str {
		match *self {
			ECArgsError::NumberArgs(ref err) => err.description(),
			ECArgsError::ArgType(ref err) => err.description(),
			ECArgsError::NumberPorts(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			ECArgsError::NumberArgs(_) => None,
			ECArgsError::ArgType(_) => None,
			ECArgsError::NumberPorts(_) => None,
		}
	}
}
impl fmt::Display for ECArgsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			ECArgsError::NumberArgs(ref err) => write!(f, "EC Args Error: {}", err),
			ECArgsError::ArgType(ref err) => write!(f, "EC Args Error: {}", err),
			ECArgsError::NumberPorts(ref err) => write!(f, "EC Args Error: {}", err),
		}
	}
}
#[derive(Debug)]
pub struct NumberArgsError { msg: String }
impl NumberArgsError { 
//	pub fn new(n: usize, needed: usize) -> NumberArgsError {
//		NumberArgsError { msg: format!("You entered {} args, but {} are required", n, needed) }
//	}
}
impl Error for NumberArgsError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NumberArgsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NumberArgsError> for ECArgsError {
	fn from(err: NumberArgsError) -> ECArgsError { ECArgsError::NumberArgs(err) }
}
#[derive(Debug)]
pub struct ArgTypeError { msg: String }
impl ArgTypeError { 
//	pub fn new(v: &str, n: usize, needed: &str) -> ArgTypeError {
//		ArgTypeError { msg: format!("You entered {} for arg {}, but {} is required", v, n, needed) }
//	}
}
impl Error for ArgTypeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for ArgTypeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<ArgTypeError> for ECArgsError {
	fn from(err: ArgTypeError) -> ECArgsError { ECArgsError::ArgType(err) }
}
#[derive(Debug)]
pub struct NumberPortsError { msg: String }
impl NumberPortsError { 
	pub fn new(n: usize) -> NumberPortsError {
		NumberPortsError { msg: format!("You asked for {} ports, but only {} are allowed", n, MAX_PORTS-1) }
	}
}
impl Error for NumberPortsError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for NumberPortsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<NumberPortsError> for ECArgsError {
	fn from(err: NumberPortsError) -> ECArgsError { ECArgsError::NumberPorts(err) }
}
