#[allow(non_snake_case)]
#[allow(unused)]

use std::{
    mem::{size_of},
    os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_void},
    ptr::{null, null_mut},
};

use crossbeam::crossbeam_channel as mpsc;

use crate::ec_message_formats::{LinkToPortPacket, PortToPePacket};
use crate::packet::{Packet};
use crate::port::{Port, PortStatus};
use crate::utility::{PortNo};

#[repr(C)]
enum NL_ECND_Commands {
    NL_ECNL_CMD_UNSPEC,
    NL_ECNL_CMD_ALLOC_DRIVER,
    NL_ECNL_CMD_GET_MODULE_INFO,
    NL_ECNL_CMD_GET_PORT_STATE,
    NL_ECNL_CMD_ALLOC_TABLE,
    NL_ECNL_CMD_FILL_TABLE,
    NL_ECNL_CMD_FILL_TABLE_ENTRY,
    NL_ECNL_CMD_SELECT_TABLE,
    NL_ECNL_CMD_DEALLOC_TABLE,
    NL_ECNL_CMD_MAP_PORTS,
    NL_ECNL_CMD_START_FORWARDING,
    NL_ECNL_CMD_STOP_FORWARDING,
    NL_ECNL_CMD_SEND_AIT_MESSAGE,
    NL_ECNL_CMD_SIGNAL_AIT_MESSAGE,
    NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE,
    NL_ECNL_CMD_WRITE_ALO_REGISTER,
    NL_ECNL_CMD_READ_ALO_REGISTERS,
    NL_ECNL_CMD_SEND_DISCOVER_MESSAGE,
    __NL_ECNL_CMD_AFTER_LAST,
    NL_ECNL_CMD_MAX, // This value is NOT set properly
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InBufferDesc {
    pub len: c_uint,
    pub frame: *mut Packet,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct OutBufferDesc {
    pub len: c_uint,
    pub frame: *const Packet,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ECNL_Port {
    pub port_module_id: u32,
    pub port_sock: *mut c_void, // Can this be const?
    pub port_esock: *const c_void,
    pub port_name: *const c_char,
    pub port_id: u8,
    pub port_up_down: c_int,
}

#[cfg(feature = "cell")]
#[link(name = ":ecnl_proto.o")]
impl ECNL_Port {
     pub fn new(port_id: u8) -> ECNL_Port {
     	  unsafe {
	      return port_create(port_id);
	  }
     }
     pub fn is_connected(&self) -> bool {
	 return self.port_up_down > 0;
     }
     pub fn refresh_connected_status(&self) {
         unsafe {
	     return port_update(self);
	 }
     }
     pub fn listen(&self, port: &Port, port_to_pe: mpsc::Sender<PortToPePacket>) -> Result<(), Error> {
         let _f = "listen";
	 println!("Listening for events on port {}",  self.port_id);
	 let mut bd = InBufferDesc {
	     len: 0,
	     frame: null_mut(),
	 };
     	 loop {
            let mut event : ECNL_Event;
            unsafe { 
                event = std::mem::uninitialized();
                port_get_event(self, &mut event);
		match event.event_cmd_id {
		    cmd_id if (cmd_id == NL_ECND_Commands::NL_ECNL_CMD_GET_PORT_STATE as c_int) => {
			println!("Port {} is {}", self.port_id, if (event.event_up_down != 0) {"up"} else {"down"});
			port_to_pe.send(PortToPePacket::Status((PortNo(self.port_id), port.is_border(), if (event.event_up_down != 0) {PortStatus::Connected} else {PortStatus::Disconnected}))).unwrap();
		    }
		    cmd_id if (cmd_id == NL_ECND_Commands::NL_ECNL_CMD_SIGNAL_AIT_MESSAGE as c_int) => {
                        println!("AIT Message Signal Received...");
			port_to_pe.send(PortToPePacket::Packet((PortNo(self.port_id), self.retrieve(&mut bd)?))).unwrap();
		    }
		    _ => {
			return Err(ECNL_PortError::UnknownCommand { func_name: _f, cmd_id: event.event_cmd_id}.into());
		    }
		}
            }
         }

     }
     pub fn retrieve(&self, bdp: &mut InBufferDesc) -> Result<Packet, Error> {
         let _f = "retrieve";
         println!("Retrieving Packet...");
    	 unsafe {
             port_do_read_async(self, bdp);
	     let packet: &Packet = &*((*bdp).frame);
             println!("Received Packet: {}", packet.to_string()?); // Probably usually sufficient to print ec_msg_type.
	     return Ok((*packet).clone()); // Can't keep this clone!
	 }
     }
    pub fn send(&self, packet: &Packet) -> Result<(), Error> {
        let bufferDesc: OutBufferDesc = OutBufferDesc {
	    len: size_of::<Packet>() as c_uint,
	    frame: packet,
	};
	println!("Sending Packet: {}", packet.to_string()?); // Probably usually sufficient to print ec_msg_type.
        unsafe {
	    port_do_xmit(self, &bufferDesc)
	}
	return Ok(())
    }
}

unsafe impl Send for ECNL_Port {}
unsafe impl Sync for ECNL_Port {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ECNL_Event {
    pub event_module_id: u32,
    pub event_port_id: u8,
    pub event_cmd_id: c_int,
    pub event_n_msgs: u32,
    pub event_up_down: c_int,
}

#[cfg(feature = "cell")]
#[link(name = ":port.o")]
extern "C" {
    pub fn ecnl_init(debug: bool) -> ::std::os::raw::c_int;
    pub fn port_create(port_id: u8) -> ECNL_Port;
    pub fn port_destroy(port: *const ECNL_Port);

    // which of these should we be using
    pub fn port_do_read_async(port: *const ECNL_Port, bdp: *mut InBufferDesc);
    pub fn port_do_read(port: *const ECNL_Port, bdp: *mut InBufferDesc, nsecs: ::std::os::raw::c_int);
    pub fn ecnl_retrieve_ait_message(nl_session: *mut c_void, port_id: c_uint, bdpp: *const *const InBufferDesc) -> c_int;
    pub fn port_do_xmit(port: *const ECNL_Port, buf: *const OutBufferDesc);
    pub fn port_update(port: *const ECNL_Port);

    pub fn port_get_event(port: *const ECNL_Port, event: *mut ECNL_Event);

    pub fn port_dumpbuf(port: *const ECNL_Port, tag: *const ::std::os::raw::c_char, buf: *mut OutBufferDesc);
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum ECNL_PortError {
    #[fail(display = "ECNL_PortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "ECNL_PortError::UnknownCommand {}: Unknown event command {}", func_name, cmd_id)]
    UnknownCommand { func_name: &'static str, cmd_id: c_int},
    #[fail(display = "ECNL_PortError::RetrievedNull {}: Retrieved null message", func_name)]
    RetrievedNull { func_name: &'static str},
}
