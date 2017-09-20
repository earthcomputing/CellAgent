use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::time;

use serde_json;

use blueprint::{Blueprint};
use config::{SEPARATOR, CellNo, CellType, DatacenterNo, Edge, PortNo};
use datacenter::{Datacenter};
use message::{MsgType};
use message_types::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside};
use name::UpTraphID;
use packet::{PacketAssembler, PacketAssemblers};

#[derive(Debug, Clone)]
pub struct Noc {
	id: UpTraphID,
	cell_type: CellType,
	no_datacenters: DatacenterNo,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(id: &str, cell_type: CellType) -> Result<Noc> {
		let id = UpTraphID::new(id)?;
		Ok(Noc { id: id, cell_type: cell_type, packet_assemblers: PacketAssemblers::new(),
				 no_datacenters: DatacenterNo(0) })
	}
	pub fn initialize(&self, blueprint: Blueprint, noc_from_outside: NocFromOutside) -> Result<Vec<JoinHandle<()>>> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, join_handles) = self.build_datacenter(&self.id, self.cell_type, blueprint)?;
		dc.connect_to_noc(port_to_noc, port_from_noc)?;
		let mut noc = self.clone();
		spawn( move || { 
			let _ = noc.listen_outside(noc_from_outside, noc_to_port).map_err(|e| noc.write_err("outside", e));
		});
		let mut noc = self.clone();
		spawn( move || {
			let _ = noc.listen_port(noc_from_port).map_err(|e| noc.write_err("port", e));	
		});
		let nap = time::Duration::from_millis(1000);
		sleep(nap);
		println!("{}", dc);
		self.control(&mut dc)?;
		Ok(join_handles)
	}
	fn control(&self, dc: &mut Datacenter) -> Result<()> {
		Ok(())
	}
	fn build_datacenter(&self, id: &UpTraphID, cell_type: CellType, blueprint: Blueprint) 
			-> Result<(Datacenter, Vec<JoinHandle<()>>)> {
		let mut dc = Datacenter::new(id, cell_type);
		let join_handles = dc.initialize(self.cell_type, blueprint)?;
		Ok((dc, join_handles))
	}
//	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
//		Ok(match msg_type {
//			_ => panic!("Noc doesn't recognize message type {}", msg_type)
//		})
//	}
	fn listen_port(&mut self, noc_from_port: NocFromPort) -> Result<()> {
		loop {
			let packet = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets)?;
				println!("Noc received {}", msg);
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<()> {
		loop {
			let input = &noc_from_outside.recv()?;
			let mut split_input = input.splitn(2, "");
			if let Some(cmd) = split_input.next() {
				match cmd {
					//"new_uptraph" => self.new_uptraph(split_input.next(), noc_to_port.clone())?,
					_ => println!("Unknown command: {}", input)
				};
			}
		}
	}
	/*
	fn new_uptraph(&mut self, str_params: Option<&str>, noc_to_port: NocToPort) -> Result<()> {
		let new_cell_type = match self.cell_type {
			CellType::NalCell => CellType::Vm,
			CellType::Vm => CellType::Container,
			_ => panic!("Bad CellType")
		};
		let name = format!("{}{}{}", self.id, SEPARATOR, *self.no_datacenters);
		let up_id = UpTraphID::new(&name)?;
		type Params = (CellNo, PortNo, Vec<Edge>);
		if let Some(str_params) = str_params {
			let params: Params = serde_json::from_str(str_params)?;
			let _ = self.build_datacenter(&up_id, new_cell_type, params.0, params.1, params.2)?;
			self.no_datacenters = DatacenterNo(*self.no_datacenters + 1);
		} else { panic!("Parameter problem"); }
		Ok(())
	}
	*/
	fn write_err(&self, s: &str, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Noc {} error: {}", s, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
	}
}
/*
#[derive(Debug)]
struct ControlChannel {
	channel: (NocToPort, NocFromPort)
}
impl ControlChannel {
	fn new(send: NocToPort, recv: NocFromPort) -> ControlChannel {
		ControlChannel { channel: (send, recv) }
	}
	fn get_send(&self) -> &NocToPort { &self.channel.0 }
	fn get_recv(&self) -> &NocFromPort { &self.channel.1 }
}
impl fmt::Display for ControlChannel {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Control Channel")	
	}
}
*/
// Errors
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		Serialize(::serde_json::Error);
	}
	links {
		Datacenter(::datacenter::Error, ::datacenter::ErrorKind);
		Message(::message::Error, ::message::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
	}
	errors { 
	}
}
