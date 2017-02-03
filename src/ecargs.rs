use std::fmt;
use config::MAX_PORTS;

pub struct ECArgs {
	pub nports: u8,
	pub ncells: usize,
}
impl ECArgs {
	pub fn get_args(args: Vec<String>)-> Result<ECArgs,ECArgsError> {
		if args.len() != 3 { Err(ECArgsError::NumberArgs(NumberArgsError::new(args.len()-1,2))) }
		else {
			let nports = match args[1].parse::<i32>() {
				Ok(n) => Some(n),
				Err(err) => None
			};
			let ncells = match args[2].parse::<i32>() {
				Ok(n) => Some(n),
				Err(_) => None
			};
			if nports.is_some() & ncells.is_some() { 
				let n = nports.unwrap() as usize;
				if n < MAX_PORTS - 1 {
					Ok(ECArgs { nports: nports.unwrap() as u8, ncells: ncells.unwrap() as usize })
				} else {
					Err(ECArgsError::NumberPorts(NumberPortsError::new(n as usize)))
				}
			} else if nports.is_none() {
				Err(ECArgsError::ArgType(ArgTypeError::new(&args[1], 1, "i32")))
			} else {
				Err(ECArgsError::ArgType(ArgTypeError::new(&args[2], 2, "i32")))
			}
		}
	}
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
	pub fn new(n: usize, needed: usize) -> NumberArgsError {
		NumberArgsError { msg: format!("You entered {} args, but {} are required", n, needed) }
	}
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
	pub fn new(v: &str, n: usize, needed: &str) -> ArgTypeError {
		ArgTypeError { msg: format!("You entered {} for arg {}, but a {} is required", v, n, needed) }
	}
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
