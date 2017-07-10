use std::fmt;
use std::io::{stdin, stdout, Read, Write};
use crossbeam::Scope;
use container::Service;
use message::{Message, SetupVMsMsg};
use message_types::{OutsideToPort, OutsideFromPort, OutsideToPortMsg};
use packet::Packetizer;

#[derive(Debug, Clone)]
pub struct Noc {}
impl Noc {
	pub fn new() -> Noc {
		Noc {  }
	}
	pub fn initialize(&self, scope: &Scope,
			outside_to_port: OutsideToPort, outside_from_port: OutsideFromPort) -> Result<()> {
		let noc = self.clone();
		let outside_to_port_clone = outside_to_port.clone();
		scope.spawn( move || {
			let _ = noc.listen_loop(outside_to_port_clone, outside_from_port).chain_err(|| ErrorKind::NocError).map_err(|e| noc.write_err(e));
		});
		scope.spawn( move || -> Result<()> {
			loop {
				stdout().write(b"Enter a command\n").chain_err(|| ErrorKind::NocError)?;
				let mut input = String::new();
				let _ = stdin().read_line(&mut input).chain_err(|| "Error reading from console")?;
				stdout().write(b"Got command\n").chain_err(|| ErrorKind::NocError)?;
				outside_to_port.send(input).chain_err(|| ErrorKind::NocError)?;
			}
		});
		Ok(())
	}
	fn listen_loop(&self, sendr: OutsideToPort, recvr: OutsideFromPort) -> Result<()> {
		loop {
			let msg = recvr.recv()?;
			println!("Noc received: {}", msg);
		}
	}
	fn setup_vms(outside_to_port: OutsideToPort) -> Result<()> {
		let msg = SetupVMsMsg::new("NocMaster", vec![vec![Service::NocMaster]])?;
		let other_index = 0;
		let direction = msg.get_header().get_direction();
		let bytes = Packetizer::serialize(&msg)?;
		let packets = Packetizer::packetize(bytes, direction, other_index)?;
		for packet in packets.iter() {
			//outside_to_port.send(**packet)?;
		}
		Ok(())
	}
	fn write_err(&self, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Noc error: {}", e);
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
	channel: (OutsideToPort, OutsideFromPort)
}
impl ControlChannel {
	fn new(send: OutsideToPort, recv: OutsideFromPort) -> ControlChannel {
		ControlChannel { channel: (send, recv) }
	}
	fn get_send(&self) -> &OutsideToPort { &self.channel.0 }
	fn get_recv(&self) -> &OutsideFromPort { &self.channel.1 }
}
impl fmt::Display for ControlChannel {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Control Channel")	
	}
}
// Errors
error_chain! {
	foreign_links {
		Io(::std::io::Error);
		Recv(::std::sync::mpsc::RecvError);
		Send(::std::sync::mpsc::SendError<OutsideToPort>);
	}
	links {
		Message(::message::Error, ::message::ErrorKind);
		Name(::name::Error, ::name::ErrorKind);
		Packet(::packet::Error, ::packet::ErrorKind);
	}
	errors { NocError
	}
}
