use std::fmt;

use packet::Packet;

pub trait Message {
	fn process(&self, port_no: u8, cell_agent: &CellAgent);
}