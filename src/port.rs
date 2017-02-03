use name::{NameError,PortID,CellID};

pub struct Port {
	id: PortID,
	is_connected: bool,
	is_broken: bool,
}
impl Port {
	pub fn new(cell_id: CellID, port_no: u8) -> Result<Port,NameError>{
		let port_label = format!("-P:{}", port_no);
		let temp_id = try!(cell_id.add_component(&port_label));
		let port_id = try!(PortID::new(&temp_id.to_string()));
		Ok(Port{ id: port_id, is_connected: false, is_broken: true })
	}
}