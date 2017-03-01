use std::thread;
use message::{Sender, Receiver};
use name::CellID;

#[derive(Debug)]
pub struct PacketEngine {
	cell_id: CellID
}
impl PacketEngine {
	pub fn new(cell_id: CellID, send_to_ca: Sender, recv_from_ca: Receiver, pe_ports: Vec<(Sender,Receiver)>) -> PacketEngine {
		let pe = PacketEngine { cell_id: cell_id.clone()};
		thread::spawn( || { PacketEngine::ca_channel(cell_id, send_to_ca, recv_from_ca) });
		pe
	}
	fn ca_channel(cell_id: CellID, send_to_ca: Sender, recv_from_ca: Receiver) {
		println!("Packet Engine for cell {} here", cell_id);
	}
}