use std::fmt;
use nalcell::NalCell;

#[derive(Debug,Clone)]
pub struct NOC<'a> {
	control: &'a NalCell,
	backup: &'a NalCell
}
#[deny(unused_must_use)]
impl<'a> NOC<'a> {
/*	
	pub fn new(control: &'a NalCell, backup: &'a NalCell) -> NOC<'a> { 
		NOC { control: control, backup: backup }
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("Control Cell = {}, Backup Cell = {}", 
			self.control.get_id(), self.backup.get_id());
		s
	}
*/	
}
// Errors
error_chain! {
	links {
		Name(::name::Error, ::name::ErrorKind);
	}
	errors { NOCError
		
	}
}
