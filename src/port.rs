use std::fmt;
use std::sync::mpsc;
use message::{Sender, Receiver};
use name::{Name, NameError,PortID,CellID};

#[derive(Debug)]
pub struct Port {
	id: PortID,
	port_no: u8,
	is_border: bool,
	is_connected: bool,
	is_broken: bool,
	send_to_link: Option<Sender>,
	recv_from_link: Option<Receiver>,
	send_to_pe: Sender,
	recv_from_pe: Receiver
}
impl Port {
	pub fn new(cell_id: &CellID, port_no: u8, is_border: bool,
			   send_to_pe: Sender, recv_from_pe: Receiver) -> Result<Port,NameError>{
		let port_id = try!(PortID::new(port_no));
		let temp_id = try!(port_id.add_component(&cell_id.get_name()));
		let is_connected = false;
		Ok(Port{ id: temp_id, port_no: port_no, send_to_link: None, recv_from_link: None,
				 is_border: is_border, is_connected: is_connected, is_broken: false,
				 send_to_pe: send_to_pe, recv_from_pe: recv_from_pe 
		})
	}
	pub fn get_id(&self) -> PortID { self.id.clone() }
	pub fn get_port_no(&self) -> u8 { self.port_no }
	pub fn is_connected(&self) -> bool { self.is_connected }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&mut self, send: Option<Sender>, recv: Option<Receiver>) { 
		self.is_connected = true; 
		self.send_to_link = send;
		self.recv_from_link = recv;
	}
	pub fn to_string(&self) -> String {
		let is_connected = self.is_connected;
		let mut s = format!("Port {} {}", self.port_no, self.id);
		if self.is_border { s = s + " is TCP  port,"; }
		else              { s = s + " is ECLP port,"; }
		if is_connected   { s = s + " is connected"; }
		else              { s = s + " is not connected"; }
		if self.is_broken { s = s + " and is broken"; }
		else              { s = s + " and is not broken"; }
		s
	}
}
impl fmt::Display for Port { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.to_string()) }
}
