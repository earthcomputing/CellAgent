use std::fmt;
use std::thread::{JoinHandle, sleep, spawn};
use std::sync::mpsc::channel;
use std::time;

use config::{CellNo, PortNo};
use container::Service;
use datacenter::Datacenter;
use message::{Message, MsgType, SetupVMsMsg};
use message_types::{NocToPort, NocFromPort, PortToNoc, PortFromNoc, NocFromOutside};
use name::UpTreeID;
use packet::{PacketAssembler, PacketAssemblers, Packetizer, Serializer};

#[derive(Debug, Clone)]
pub struct Noc {
	id: UpTreeID,
	packet_assemblers: PacketAssemblers
}
impl Noc {
	pub fn new(id: &str) -> Result<Noc> {
		let id = UpTreeID::new(id).chain_err(|| ErrorKind::NocError)?;
		Ok(Noc { id: id, packet_assemblers: PacketAssemblers::new() })
	}
	pub fn initialize(&self, ncells: CellNo, nports: PortNo, edges: Vec<(CellNo, CellNo)>,
			noc_from_outside: NocFromOutside) -> Result<Vec<JoinHandle<()>>> {
		let (noc_to_port, port_from_noc): (NocToPort, NocFromPort) = channel();
		let (port_to_noc, noc_from_port): (PortToNoc, PortFromNoc) = channel();
		let (mut dc, join_handles) = self.build_datacenter(&self.id, nports, ncells, edges)?;
		dc.connect_to_noc(port_to_noc, port_from_noc).chain_err(|| ErrorKind::NocError)?;
		let noc = self.clone();
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
	fn build_datacenter(&self, id: &UpTreeID, nports: u8, ncells: usize, edges: Vec<(CellNo,CellNo)>) 
			-> Result<(Datacenter, Vec<JoinHandle<()>>)> {
		let mut dc = Datacenter::new(id);
		let join_handles = dc.initialize(ncells, nports, edges)?;
		Ok((dc, join_handles))
	}
	fn get_msg(&self, msg_type: MsgType, serialized_msg:String) -> Result<Box<Message>> {
		Ok(match msg_type {
			_ => panic!("Noc doesn't recognize message type {}", msg_type)
		})
	}
	fn listen_port(&mut self, noc_from_port: NocFromPort) -> Result<()> {
		let noc = self.clone();
		loop {
			let packet = noc_from_port.recv()?;
			let msg_id = packet.get_header().get_msg_id();
			let mut packet_assembler = self.packet_assemblers.remove(&msg_id).unwrap_or(PacketAssembler::new(msg_id));
			if let Some(packets) = packet_assembler.add(packet) {
				let (msg_type, serialized_msg) = MsgType::get_type_serialized(packets).chain_err(|| ErrorKind::NocError)?;
				let msg = self.get_msg(msg_type, serialized_msg)?;
				println!("Noc received {}", msg);
			} else {
				self.packet_assemblers.insert(msg_id, packet_assembler);
			}
		}
	}
	fn listen_outside(&self, noc_from_outside: NocFromOutside, noc_to_port: NocToPort) -> Result<()> {
		loop {
			let input = &noc_from_outside.recv()?;
			let mut parsed = input.split_whitespace();
			if let Some(cmd) = parsed.next() {
				match cmd {
					"new_uptree" => self.new_uptree(noc_to_port.clone())?,
					_ => println!("Unknown command: {}", input)
				};
			}
		}
	}
	fn new_uptree(&self, noc_to_port: NocToPort) -> Result<()> {
		let msg = SetupVMsMsg::new("NocMaster", vec![vec![Service::NocMaster]])?;
		let other_index = 0;
		let direction = msg.get_header().get_direction();
		let bytes = Serializer::serialize(&msg)?;
		let packets = Packetizer::packetize(bytes, direction, other_index)?;
		for packet in packets.iter() {
			noc_to_port.send(**packet)?;
		}
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
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		Send(::message_types::NocPortError);
	}
	links {
		Datacenter(::datacenter::Error, ::datacenter::ErrorKind);
		Message(::message::Error, ::message::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		Packet(::packet::Error, ::packet::ErrorKind);
	}
	errors { NocError
		Input(input: String) {
			description("Invalid input")
			display("{} is not a valid command to the NOC", input)
		}
	}
}
