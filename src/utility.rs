use std::fmt;
use std::collections::HashSet;

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
pub const BASE_TENANT_MASK: Mask = Mask { mask: MaskValue{v:255} };   // All ports
pub const DEFAULT_USER_MASK: Mask = Mask { mask: MaskValue{v:254} };  // All ports except port 0
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Mask { mask: MaskValue }
#[deny(unused_must_use)]
impl Mask {
	pub fn new(i: PortNumber) -> Mask {
	    let mask = MaskValue{v:(1 as u16).rotate_left(i.get_port_no().v as u32)};
        Mask { mask: mask } 
	}
	pub fn new0() -> Mask { Mask { mask: MaskValue{v:1} } }
	pub fn empty() -> Mask { Mask { mask: MaskValue{v:0} } }
	pub fn all_but_zero() -> Mask {
		Mask::empty().not().all_but_port(PortNumber::new0())
	}
	pub fn equal(&self, other: Mask) -> bool { self.mask.v == other.mask.v }
	pub fn get_as_value(&self) -> MaskValue { self.mask }
	pub fn or(&self, mask: Mask) -> Mask {
		Mask { mask: MaskValue{v:self.mask.v | mask.mask.v} }
	}
	pub fn and(&self, mask: Mask) -> Mask {
		Mask { mask: MaskValue{v:self.mask.v & mask.mask.v} }
	}
	pub fn not(&self) -> Mask {
		Mask { mask: MaskValue{v:!self.mask.v} }
	}
	pub fn all_but_port(&self, port_number: PortNumber) -> Mask {
		let port_mask = Mask::new(port_number);
		self.and(port_mask.not())
	}
	pub fn make(port_numbers: &HashSet<PortNumber>) -> Mask {
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
		for i in 0..MAX_PORTS.v {
			let port_number = match PortNumber::new(PortNo{v:i}, MAX_PORTS) {
				Ok(n) => n,
				Err(_) => panic!("Mask port_nos_from_mask cannont generate an error")
			};
			let test = Mask::new(port_number);
			if test.mask.v & self.mask.v != 0 { port_nos.push(PortNo{v:i}) }
		}
		port_nos
	}
}
impl fmt::Display for Mask {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, " {:016.b}", self.mask.v) 
	}
}
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortNumber { pub port_no: PortNo }
impl PortNumber {
	pub fn new(no: PortNo, no_ports: PortNo) -> Result<PortNumber> {
		if no.v > no_ports.v {
			Err(ErrorKind::PortNumber(no, no_ports).into())
		} else {
			Ok(PortNumber { port_no: (no as PortNo) })
		}
	}
	pub fn new0() -> PortNumber { PortNumber { port_no: (PortNo{v:0}) } }
	pub fn get_port_no(&self) -> PortNo { self.port_no }
}
impl fmt::Display for PortNumber {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_no.v) }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct Path { port_number: PortNumber }
#[deny(unused_must_use)]
impl Path {
	pub fn new(port_no: PortNo, no_ports: PortNo) -> Result<Path> {
		let port_number = try!(PortNumber::new(port_no, no_ports));
		Ok(Path { port_number: port_number })
	}
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
}
impl fmt::Display for Path {
	fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_number) }
}
// Errors
use name::CellID;
error_chain! {
	errors { UtilityError
		Mask(cell_id: CellID) {
			description("Mask error")
			display("Cell {} has no tenant mask", cell_id)
		}
		PortNumber(port_no: PortNo, max: PortNo) {
			description("Invalid port number")
			display("Port number {} is larger than the maximum of {}", port_no.v, max.v)
		}
		Unimplemented(feature: String) {
			description("Feature is not implemented")
			display("{} is not implemented", feature)
		}
	}
}
