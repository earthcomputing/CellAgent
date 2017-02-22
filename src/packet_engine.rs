use std::collections::VecDeque;
use packet::Packet;

#[derive(Debug, Clone)]
pub struct PacketEngine {
	send_buffer: VecDeque<Packet>,
	recv_buffer: VecDeque<Packet>
}
impl PacketEngine {
	pub fn new() -> PacketEngine {
		PacketEngine { send_buffer: VecDeque::new(), recv_buffer: VecDeque::new() }
	}
}