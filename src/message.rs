use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use cellagent::CellAgent;
use packet::Packet;

pub type Sender = mpsc::Sender<Packet>;
pub type Receiver = mpsc::Receiver<Packet>;
pub type SendError = mpsc::SendError<Packet>;

pub trait Message {
	fn process(&self, port_no: u8, cell_agent: &CellAgent);
}