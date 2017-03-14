use std::fmt;
#[derive(Copy)]
pub struct Packet64 {
	index: u32,
	sending_port: u8,
	receiving_port: u8,
	is_rootcast: bool,
	payload: [char; 56],
}
impl Packet64 {
	pub fn new(index: u32, is_rootcast: bool, payload: [char; 56] ) -> Packet64 {
		Packet64 { index: index, sending_port: 0, receiving_port: 0, is_rootcast: is_rootcast, payload: payload }
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("Table Index {}: ", self.index);
		if self.is_rootcast { s = s + "Rootward"; }
		else                { s = s + "Leafward"; }
		s = s + &format!(", Sending port {}", self.sending_port);
		s = s + ": Payload = ";
		for c in self.payload.iter() {
			s.push(*c);
		}
		s
	}
}
impl Clone for Packet64 {
	fn clone(&self) -> Packet64 { *self }
}
impl fmt::Debug for Packet64 { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl fmt::Display for Packet64 { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}