use std::fmt;
use std::collections::HashSet;

use failure::{Error, Fail, ResultExt};

use config::{MAX_PORTS, OUTPUT_FILE_NAME, MaskValue, PortNo};
use name::CellID;
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
pub const BASE_TENANT_MASK: Mask = Mask { mask: MaskValue(255) };   // All ports
pub const DEFAULT_USER_MASK: Mask = Mask { mask: MaskValue(254) };  // All ports except port 0
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Mask { mask: MaskValue }
#[deny(unused_must_use)]
impl Mask {
	pub fn new(i: PortNumber) -> Mask {
	    let mask = MaskValue((1 as u16).rotate_left(i.get_port_no().v as u32));
        Mask { mask: mask } 
	}
	pub fn new0() -> Mask { Mask { mask: MaskValue(1) } }
	pub fn empty() -> Mask { Mask { mask: MaskValue(0) } }
	pub fn all_but_zero(no_ports: PortNo) -> Mask { 
		Mask { mask: MaskValue((2 as u16).pow(no_ports.v as u32)-2) }
	}
	pub fn equal(&self, other: Mask) -> bool { *self.mask == *other.mask }
	pub fn get_as_value(&self) -> MaskValue { self.mask }
	pub fn or(&self, mask: Mask) -> Mask {
		Mask { mask: MaskValue(*self.mask | *mask.mask) }
	}
	pub fn and(&self, mask: Mask) -> Mask {
		Mask { mask: MaskValue(*self.mask & *mask.mask) }
	}
	pub fn not(&self) -> Mask {
		Mask { mask: MaskValue(!*self.mask) }
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
				Err(_) => panic!("Mask get_port_no cannont generate an error")
			};
			let test = Mask::new(port_number);
			if *test.mask & *self.mask != 0 { port_nos.push(PortNo{v:i}) }
		}
		port_nos
	}
}
impl fmt::Display for Mask {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		write!(f, " {:016.b}", *self.mask) 
	}
}
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortNumber { pub port_no: PortNo }
impl PortNumber {
	pub fn new(no: PortNo, no_ports: PortNo) -> Result<PortNumber, UtilityError> {
		if no.v > no_ports.v {
			Err(UtilityError::PortNumber{ port_no: no, func_name: S("PortNumber::new"), max: no_ports }.into())
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
impl Path {
	pub fn new(port_no: PortNo, no_ports: PortNo) -> Result<Path, UtilityError> {
		let port_number = PortNumber::new(port_no, no_ports)?;
		Ok(Path { port_number: port_number })
	}
	pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
}
impl fmt::Display for Path {
	fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_number) }
}
use std::fs::{File, OpenOptions};
use std::io::Write;
pub fn append2file(line: String) -> Result<(), Error> {
	let mut file_handle = match OpenOptions::new().append(true).open(OUTPUT_FILE_NAME) {
		Ok(f) => Ok(f),
		Err(_) => {
			println!("Writing output to {}", OUTPUT_FILE_NAME);
			File::create(OUTPUT_FILE_NAME)
		}
	}?;
	file_handle.write(&(line + "\n").into_bytes())?;
	Ok(())
}
pub fn write_err(caller: &str, e: Error) {
	use ::std::io::Write;
	let stderr = &mut ::std::io::stderr();
	let _ = writeln!(stderr, "*** Error {}: {}", caller, e);
	for cause in e.causes() {
		println!("*** Caused by {}", cause);
	}
	let fail: &Fail = e.cause();
	if let Some(backtrace) = fail.cause().and_then(|cause| cause.backtrace()) {
		let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
	}
}
// There are so many places in my code where it's more convenient
// to provide &str but I need String that I made the following
pub fn S<T: fmt::Display>(s: T) -> String { s.to_string() }
// Errors
#[derive(Debug, Fail)]
pub enum UtilityError {
    #[fail(display = "{}: Utility: Cell {} has no tenant mask", func_name, cell_id)]
    Mask { cell_id: CellID, func_name: String},
    #[fail(display = "{}: Utility: Port number {:?} is larger than the maximum of {:?}", func_name, port_no, max)]
    PortNumber { port_no: PortNo, func_name: String, max: PortNo },
    #[fail(display = "{}: Utility: {} is not implemented", func_name, feature)]
    Unimplemented { feature: String, func_name: String }
}
