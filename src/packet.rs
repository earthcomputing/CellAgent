use std::fmt;
#[derive(Copy)]
pub struct Packet {
	index: u32,
	packet: [char; 32],
}
impl Packet {
	fn to_string(&self) -> String {
		let mut s = format!("Table Index {}: {:?}", self.index, self.packet);
		for c in self.packet.iter() {
			s = s + &c.to_string();
		}
		s
	}
}
impl Clone for Packet {
	fn clone(&self) -> Packet { Packet { index: self.index, packet: self.packet } }
}
impl fmt::Debug for Packet { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}