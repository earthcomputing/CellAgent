use std::fmt;
use crossbeam::{Scope, ScopedJoinHandle};
use nalcell::{LinkToPort, LinkFromPort, LinkToPortMsg};
use name::{Name, LinkID, PortID};
use port::{PortStatus};

#[derive(Debug, Clone)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		      //     Left Port        Link        Right Port
}
impl Link {
	pub fn new(left_id: &PortID, rite_id: &PortID) -> Result<Link> {
		let rite_name = rite_id.get_name();
		let temp_id = left_id.add_component(&rite_name).chain_err(|| ErrorKind::LinkError)?;
		let id = LinkID::new(&temp_id.get_name()).chain_err(|| ErrorKind::LinkError)?;
		Ok(Link { id: id, is_broken: false, is_connected: true })
	}
	pub fn start_threads(&self, scope: &Scope,
			link_to_left: LinkToPort, link_from_left: LinkFromPort,
			link_to_rite: LinkToPort, link_from_rite: LinkFromPort ) 
				-> Result<(ScopedJoinHandle<()>, ScopedJoinHandle<()>)> {
		let left_handle = self.listen(scope, link_to_left.clone(), link_from_left, link_to_rite.clone());
		let rite_handle = self.listen(scope, link_to_rite, link_from_rite, link_to_left);
		Ok((left_handle, rite_handle))
	}
	fn write_err(&self, e: Error) {
		use ::std::io::Write;
		let stderr = &mut ::std::io::stderr();
		let _ = writeln!(stderr, "Link {}: {}", self.id, e);
		for e in e.iter().skip(1) {
			let _ = writeln!(stderr, "Caused by: {}", e);
		}
		if let Some(backtrace) = e.backtrace() {
			let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
		}
	}
	fn listen(&self, scope: &Scope, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort) 
				-> ScopedJoinHandle<()> {
		let link = self.clone();
		scope.spawn( move || {
			let _ = status.send(LinkToPortMsg::Status(PortStatus::Connected)).chain_err(|| ErrorKind::LinkError).map_err(|e| link.write_err(e));
			let _ = link.listen_loop(link_from, link_to).chain_err(|| ErrorKind::LinkError).map_err(|e| link.write_err(e));
		})
	}			
	fn listen_loop(&self, link_from: LinkFromPort, link_to: LinkToPort) -> Result<()> {
		loop {
			//println!("Link {}: waiting to recv left", link_id);
			let packet = link_from.recv().chain_err(|| ErrorKind::LinkError)?;
			link_to.send(LinkToPortMsg::Msg(packet)).chain_err(|| ErrorKind::LinkError)?;
			//println!("Link {}: sent packet {} right", link_id, packet.get_count());
		}
	}
}
impl fmt::Display for Link { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nLink {}", self.id.get_name().to_string());
		if self.is_connected { s = s + " is connected"; }
		else                 { s = s + " is not connected"; }
		write!(f, "{}", s) 
	}
}
// Errors
error_chain! {
	foreign_links {
		Recv(::std::sync::mpsc::RecvError);
		Send(::nalcell::LinkPortError);
	}
	links {
		Name(::name::Error, ::name::ErrorKind);
		Port(::port::Error, ::port::ErrorKind);
	}
	errors {
		LinkError
	}
}
