use std::fmt;
#[derive(Copy)]
pub struct Packet {
	index: u32,
	is_rootcast: bool,
	payload: [char; 64],
}
impl Packet {
	pub fn new(index: u32, is_rootcast: bool, payload: [char; 64] ) -> Packet {
		Packet { index: index, is_rootcast: is_rootcast, payload: payload }
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("Table Index {}: ", self.index);
		if self.is_rootcast { s = s + "Rootward"; }
		else                { s = s + "Leafward"; }
		s = s + ": Payload = ";
		for c in self.payload.iter() {
			s.push(*c);
		}
		s
	}
}
impl Clone for Packet {
	fn clone(&self) -> Packet { *self }
}
impl fmt::Debug for Packet { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl fmt::Display for Packet { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}