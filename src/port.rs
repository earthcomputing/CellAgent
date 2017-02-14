use std::fmt;
use std::cell::RefCell;
use name::{NameError,PortID,CellID};

#[derive(Debug, Clone)]
pub struct Port {
	id: PortID,
	port_no: u8,
	is_border: bool,
	is_connected: RefCell<[bool;1]>,
	is_broken: bool,
}
impl Port {
	pub fn new(cell_id: &CellID, port_no: u8, is_border: bool) -> Result<Port,NameError>{
		let port_label = format!("P:{}", port_no);
		let temp_id = try!(cell_id.add_component(&port_label));
		let port_id = try!(PortID::new(&temp_id.get_name().to_string()));
		let is_connected = RefCell::new([false]);
		Ok(Port{ id: port_id, port_no: port_no, 
				 is_border: is_border, is_connected: is_connected, is_broken: false })
	}
	pub fn get_id(&self) -> PortID { self.id }
	pub fn get_port_no(&self) -> u8 { self.port_no }
	pub fn is_connected(&self) -> bool { self.is_connected.borrow()[0] }
	pub fn is_broken(&self) -> bool { self.is_broken }
	pub fn is_border(&self) -> bool { self.is_border }
	pub fn set_connected(&self) { 
		let mut state = self.is_connected.borrow_mut();
		state[0] = true;
	}
	pub fn to_string(&self) -> String {
		let is_connected = self.is_connected.borrow()[0];
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
