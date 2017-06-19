use std::fmt;
use crossbeam::Scope;
use nalcell::{LinkToPort, LinkFromPort};
use name::{Name, LinkID, PortID};
use port::{PortStatus};

#[derive(Debug, Clone)]
pub struct Link {
	id: LinkID,
	is_broken: bool,
	is_connected: bool,		      //     Left Port        Link        Right Port
}
impl Link {
	pub fn new(scope: &Scope, left_id: &PortID, rite_id: &PortID,
			link_to_left: LinkToPort, link_from_left: LinkFromPort,
			link_to_rite: LinkToPort, link_from_rite: LinkFromPort )
				-> Result<Link> {
		let rite_name = rite_id.get_name();
		let temp_id = left_id.add_component(&rite_name).chain_err(|| ErrorKind::LinkError)?;
		let id = LinkID::new(&temp_id.get_name()).chain_err(|| ErrorKind::LinkError)?;
		let link = Link { id: id, is_broken: false, is_connected: true };
		link.listen(scope, link_to_left.clone(), link_from_left, link_to_rite.clone()).chain_err(|| ErrorKind::LinkError)?;
		link.listen(scope, link_to_rite, link_from_rite, link_to_left).chain_err(|| ErrorKind::LinkError)?;
		Ok(link)
	}
	fn listen(&self, scope: &Scope, status: LinkToPort, link_from: LinkFromPort, link_to: LinkToPort) 
				-> Result<()> {
		let link = self.clone();
		scope.spawn( move || -> Result<()> {
			status.send((Some(PortStatus::Connected),None)).chain_err(|| ErrorKind::LinkError)?;
			link.listen_loop(link_from, link_to).chain_err(|| ErrorKind::LinkError)?;
			Ok(())
		});
		Ok(())
	}			
	fn listen_loop(&self, link_from: LinkFromPort, link_to: LinkToPort) -> Result<()> {
		loop {
			//println!("Link {}: waiting to recv left", link_id);
			let packet = link_from.recv().chain_err(|| ErrorKind::LinkError)?;
			link_to.send((None,Some(packet))).chain_err(|| ErrorKind::LinkError)?;
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
