use config::{MAX_PORTS, SEPARATOR};

pub fn get_first_arg(a: Vec<String>) -> Option<i32> {
	if a.len() != 2 {
		None
	} else {
		match a[1].parse::<i32>() {
			Ok(x) => Some(x),
			Err(_) => None
		}
	}
}
pub fn chars_to_string(chars: &[char]) -> String {
	let mut s = String::new();
	for c in chars.iter() {
		if *c == ' ' { break; }
		s = s + &c.to_string();
	}
	s
}
pub fn int_to_mask(i: u8) -> Result<u16, UtilityError> {
    if i > 15 {
        Err(UtilityError::Port(PortError::new(i)))
    } else {
        let mask: u16 = (1 as u16).rotate_left(i as u32);
        Ok(mask)
    }
}
pub fn mask_from_port_nos(port_nos: Vec<u8>) -> Result<u16, UtilityError> {
	let mut mask: u16 = 0;
	for port_no in port_nos.iter() {
		mask = mask | try!(int_to_mask(*port_no));
	}
	Ok(mask)
}
// Errors
use std::fmt;
use std::error::Error;
#[derive(Debug)]
pub enum UtilityError {
	Port(PortError),
}
impl Error for UtilityError {
	fn description(&self) -> &str {
		match *self {
			UtilityError::Port(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			UtilityError::Port(ref err) => Some(err),
		}
	}
}
impl fmt::Display for UtilityError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			UtilityError::Port(_) => write!(f, "Cell Agent Name Error caused by"),
		}
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: u8) -> PortError {
		PortError { msg: format!("Port number {} is larger than the maximum of {}", port_no, MAX_PORTS) }
	}
}
impl Error for PortError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<PortError> for UtilityError {
	fn from(err: PortError) -> UtilityError { UtilityError::Port(err) }
}
