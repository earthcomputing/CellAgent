use config::{MAX_PORTS, MaskValue, PortNo};
/*
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
*/
pub const BASE_TENANT_MASK: Mask = Mask { mask: 255 };   // All ports
pub const DEFAULT_USER_MASK: Mask = Mask { mask: 254 };  // All ports except port 0
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Mask { mask: MaskValue }
#[deny(unused_must_use)]
impl Mask {
	pub fn new(i: PortNumber) -> Mask {
	    let mask = (1 as u16).rotate_left(i.get_port_no() as u32);
        Mask { mask: mask } 
	}
	pub fn new0() -> Mask { Mask { mask: 1 } }
	pub fn empty() -> Mask { Mask { mask: 0 } }
	pub fn get_as_value(&self) -> MaskValue { self.mask }
	pub fn or(&self, mask: Mask) -> Mask {
		Mask { mask: self.mask | mask.mask }
	}
	pub fn and(&self, mask: Mask) -> Mask {
		Mask { mask: self.mask & mask.mask }
	}
	pub fn not(&self) -> Mask {
		Mask { mask: !self.mask }
	}
	pub fn all_but_port(&self, port_number: PortNumber) -> Mask {
		let port_mask = Mask::new(port_number);
		self.and(port_mask.not())
	}
	pub fn make(port_numbers: Vec<PortNumber>) -> Mask {
		let mut mask = Mask::empty();
		// Using map() is more complicated because of try!
		for port_number in port_numbers.iter() {
			let port_mask = Mask::new(*port_number);
			mask = mask.or(port_mask);
		}
		mask
	}
	pub fn get_port_nos(&self) -> Vec<PortNo> {
		let mut port_nos = Vec::new();
		for i in 0..MAX_PORTS {
			let port_number = match PortNumber::new(i, MAX_PORTS) {
				Ok(n) => n,
				Err(_) => panic!("Mask port_nos_from_mask cannont generate an error")
			};
			let test = Mask::new(port_number);
			if test.mask & self.mask != 0 { port_nos.push(i as PortNo) }
		}
		port_nos
	}
}
impl fmt::Display for Mask {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, " {:016.b}", self.mask) 
	}
}
// Errors
use std::fmt;
use std::error::Error;
#[derive(Debug)]
pub enum UtilityError {
	Port(PortError),
	Unimplemented(UnimplementedError),
}
impl Error for UtilityError {
	fn description(&self) -> &str {
		match *self {
			UtilityError::Port(ref err) => err.description(),
			UtilityError::Unimplemented(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			UtilityError::Port(ref err) => Some(err),
			UtilityError::Unimplemented(ref err) => Some(err),
		}
	}
}
impl fmt::Display for UtilityError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			UtilityError::Port(_) => write!(f, "Cell Agent Name Error caused by"),
			UtilityError::Unimplemented(_) => write!(f, "Cell Agent Unimplemented Feature Error caused by"),
		}
	}
}
#[derive(Debug)]
pub struct MaskError { msg: String }
impl MaskError { 
//	pub fn new(cell_id: CellID) -> MaskError {
//		MaskError { msg: format!("Cell {} has no tenant mask", cell_id) }
//	}
}
impl Error for MaskError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for MaskError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
#[derive(Debug)]
pub struct PortError { msg: String }
impl PortError { 
	pub fn new(port_no: PortNo) -> PortError {
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
#[derive(Debug)]
pub struct UnimplementedError { msg: String }
impl UnimplementedError { 
//	pub fn new(feature: &str) -> UnimplementedError {
//		UnimplementedError { msg: format!("{} is not implemented", feature) }
//	}
}
impl Error for UnimplementedError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for UnimplementedError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg) 
	}
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct PortNumber { pub port_no: PortNo }
#[deny(unused_must_use)]
impl PortNumber {
	pub fn new(no: PortNo, no_ports: PortNo) -> Result<PortNumber, PortNumberError> {
		if no > no_ports {
			Err(PortNumberError::new(no, no_ports))
		} else {
			Ok(PortNumber { port_no: (no as PortNo) })
		}
	}
	pub fn new0() -> PortNumber { PortNumber { port_no: (0 as PortNo) } }
	pub fn get_port_no(&self) -> PortNo { self.port_no }
}
impl fmt::Display for PortNumber {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_no) }
}
#[derive(Debug)]
pub struct PortNumberError { msg: String }
impl PortNumberError {
	pub fn new(port_no: PortNo, no_ports: PortNo) -> PortNumberError {
		let msg = format!("You asked for port number {}, but this cell only has {} ports",
			port_no, no_ports);
		PortNumberError { msg: msg }
	}
}
impl Error for PortNumberError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for PortNumberError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.msg) }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct Path { port_number: PortNumber }
#[deny(unused_must_use)]
impl Path {
	pub fn new(port_no: PortNo, no_ports: PortNo) -> Result<Path, PortNumberError> {
		let port_number = try!(PortNumber::new(port_no, no_ports));
		Ok(Path { port_number: port_number })
	}
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
}
impl fmt::Display for Path {
	fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_number) }
}