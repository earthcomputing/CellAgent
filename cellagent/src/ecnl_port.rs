#[allow(non_snake_case)]
#[allow(unused)]

use crossbeam::crossbeam_channel as mpsc;

use crate::ec_message_formats::{PortToPePacket};
use crate::port::{Port, PortStatus};
use crate::utility::{PortNo};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct BufferDesc {
    pub len: u32,
    pub frame: *mut u8,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ECNL_Port {
    pub port_module_id: u32,
    pub port_sock: *const ::std::os::raw::c_void,
    pub port_esock: *const ::std::os::raw::c_void,
    pub port_name: *const ::std::os::raw::c_char,
    pub port_id: u8,
    pub port_up_down: ::std::os::raw::c_int,
}

#[cfg(feature = "cell")]
impl ECNL_Port {
     pub fn new(port_id: u8) -> ECNL_Port {
     	  unsafe {
	      return port_create(port_id);
	  }
     }
     pub fn is_connected(&self) -> bool {
	 return self.port_up_down > 0;
     }
     pub fn listen(&self, port: &Port, port_to_pe: mpsc::Sender<PortToPePacket>) -> Result<(), Error> {
	 println!("Listening for events on port {}",  self.port_id);
     	 loop {
            let mut event : ECNL_Event;
            unsafe { 
                event = std::mem::uninitialized();
                port_get_event(self, &mut event);
		println!("Port {} is {}", self.port_id, if (event.event_up_down != 0) {"up"} else {"down"});
            }
	    port_to_pe.send(PortToPePacket::Status((PortNo(self.port_id), port.is_border(), if (event.event_up_down != 0) {PortStatus::Connected} else {PortStatus::Disconnected}))).unwrap();
         }
     }
}

unsafe impl Send for ECNL_Port {}
unsafe impl Sync for ECNL_Port {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ECNL_Event {
    pub event_module_id: u32,
    pub event_port_id: u8,
    pub event_cmd_id: ::std::os::raw::c_int,
    pub event_n_msgs: u32,
    pub event_up_down: ::std::os::raw::c_int,
}

#[cfg(feature = "cell")]
#[link(name = ":port.o")]

extern "C" {
    pub fn ecnl_init(debug: bool) -> ::std::os::raw::c_int;
    pub fn port_create(port_id: u8) -> ECNL_Port;
    pub fn port_destroy(port: *const ECNL_Port);

    pub fn port_do_read_async(port: *const ECNL_Port, actual_buf: *mut BufferDesc);
    pub fn port_do_read(port: *const ECNL_Port, actual_buf: *mut BufferDesc, nsecs: ::std::os::raw::c_int);
    pub fn port_do_xmit(port: *const ECNL_Port, buf: *mut BufferDesc);
    pub fn port_update(port: *const ECNL_Port);

    pub fn port_get_event(port: *const ECNL_Port, event: *mut ECNL_Event);

    pub fn port_dumpbuf(port: *const ECNL_Port, tag: *const ::std::os::raw::c_char, buf: *mut BufferDesc);
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum ECNL_PortError {
    #[fail(display = "ECNL_PortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}