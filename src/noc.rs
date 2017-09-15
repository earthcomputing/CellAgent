use std::fmt;
use std::collections::HashMap;
use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::time;

use serde_json;

use config::{SEPARATOR, CellNo, DatacenterNo, Edge, PortNo};
use container::Service;
use datacenter::{Datacenter};
use message::{Message, MsgType, SetupVMsMsg};
use message_types::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside};
use name::UpTraphID;
use nalcell::CellType;
use packet::{PacketAssembler, PacketAssemblers, Packetizer, Serializer};
use utility::S;

#[derive(Debug, Clone)]
pub struct Noc {
	id: UpTraphID,
	cell_type: CellType,
	no_datacenters: DatacenterNo,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(id: &str, cell_type: CellType) -> Result<Noc> {
		let f = "new";
		let id = UpTraphID::new(id).chain_err(|| ErrorKind::Name(S(f), S(id)))?;
		Ok(Noc { id: id, cell_type: cell_type, packet_assemblers: PacketAssemblers::new(),
				 no_datacenters: DatacenterNo(0) })
	}
	pub fn initialize(&self, ncells: CellNo, nports: PortNo, edges: Vec<Edge>,
			noc_from_outside: NocFromOutside) -> Result<Vec<JoinHandle<()>>> {
		let f = "initialize";
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, join_handles) = self.build_datacenter(&self.id, self.cell_type, ncells, nports, edges)?;
		dc.connect_to_noc(port_to_noc, port_from_noc).chain_err(|| ErrorKind::Datacenter(S(f)))?;
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
	fn build_datacenter(&self, id: &UpTraphID, cell_type: CellType, 
			ncells: CellNo, nports: PortNo, edges: Vec<Edge>) -> Result<(Datacenter, Vec<JoinHandle<()>>)> {
		let f = "build_datacenter";
		let mut dc = Datacenter::new(id, cell_type);
		let join_handles = dc.initialize(ncells, nports, edges, self.cell_type).chain_err(|| ErrorKind::Datacenter(S(f)))?;
		Ok((dc, join_handles))
	}
	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
		Ok(match msg_type {
			_ => panic!("Noc doesn't recognize message type {}", msg_type)
		})
	}
	fn listen_port(&mut self, noc_from_port: NocFromPort) -> Result<()> {
		let f = "listen_port";
		let noc = self.clone();
		loop {
			let packet = noc_from_port.recv().chain_err(|| ErrorKind::Recv(S(f), S("Port")))?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			let (last_packet, packets) = packet_assembler.add(packet);
			if last_packet {
				let msg = MsgType::get_msg(&packets).chain_err(|| ErrorKind::Message(S(f)))?;
				println!("Noc received {}", msg);
			} else {
				let assembler = PacketAssembler::create(msg_id, packets);
				self.packet_assemblers.insert(msg_id, assembler);
			}
		}
	}
	fn listen_outside(&mut self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<()> {
		let f = "listen_outside";
		loop {
			let input = &noc_from_outside.recv().chain_err(|| ErrorKind::Recv(S(f), S("Outside")))?;
			let mut split_input = input.splitn(2, "");
			if let Some(cmd) = split_input.next() {
				match cmd {
					"new_uptraph" => self.new_uptraph(split_input.next(), noc_to_port.clone())?,
					_ => println!("Unknown command: {}", input)
				};
			}
		}
	}
	fn new_uptraph(&mut self, str_params: Option<&str>, noc_to_port: NocToPort) -> Result<()> {
		let f = "new_uptraph";
		let new_cell_type = match self.cell_type {
			CellType::NalCell => CellType::Vm,
			CellType::Vm => CellType::Container,
			_ => panic!("Bad CellType")
		};
		let name = format!("{}{}{}", self.id, SEPARATOR, *self.no_datacenters);
		let up_id = UpTraphID::new(&name).chain_err(|| ErrorKind::Name(S(f), S(name)))?;
		type Params = (CellNo, PortNo, Vec<Edge>);
		if let Some(str_params) = str_params {
			let params: Params = serde_json::from_str(str_params).chain_err(|| ErrorKind::Deserialize(S(f), S(str_params)))?;
			let dc = self.build_datacenter(&up_id, new_cell_type, params.0, params.1, params.2).chain_err(|| ErrorKind::Input(str_params.to_string(), "new_uptraph".to_string()))?;
			self.no_datacenters = DatacenterNo(*self.no_datacenters + 1);
		} else { panic!("Parameter problem"); }
		Ok(())
	}
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
// Errors
error_chain! {
	errors { 
		Build(up_id: UpTraphID, func_name: String) {
			display("Noc {}: Problem building datacenter at up_traph {}", func_name, up_id)
		}
		Datacenter(func_name: String) { 
			display("Noc {}: Cannot connect to datacenter", func_name) 
		}
		Deserialize(func_name: String, serialized: String) {
			display("Message {}: Can't deserialize {}", func_name, serialized)
		}
		Input(input: String, func_name: String) {
			display("Noc {}: {} is not a valid command to the NOC", func_name, input)
		}
		Message(func_name: String) { 
			display("Noc {}: Problem reading message", func_name) 
		}
		Name(func_name: String, name: String) {
			display("Noc {}: {} is not a valid name", func_name, name)
		}
		Recv(func_name: String, source: String) { display("Noc {}: Problem receiving from {}", func_name, source) }
	}
}
