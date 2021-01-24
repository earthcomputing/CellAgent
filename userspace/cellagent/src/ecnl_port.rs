#[allow(non_snake_case)]
#[allow(unused)]

use std::{
    ffi::CStr,
    mem::{size_of},
    os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_void},
    ptr::{null, null_mut},
    thread::{sleep},
    time::Duration,
};

use crossbeam::crossbeam_channel as mpsc;

use crate::app_message_formats::{PortToCa};
use crate::ec_message_formats::{PortToPePacket, PortToPe};
use crate::name::{CellID};
use crate::packet::{Packet};
use crate::port::{InteriorPortLike, BasePort, PortStatus};
use crate::simulated_border_port::{SimulatedBorderPort};
use crate::utility::{PortNo, PortNumber};

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
pub struct ECNL_Port_Sub {
    pub port_module_id: u32,
    pub port_sock: *mut c_void, // Can this be const?
    pub port_esock: *const c_void,
    pub port_name: *const c_char,
    pub port_id: u8,
    pub port_up_down: c_int,
}

#[derive(Debug, Clone)]
pub struct ECNL_Port {
    base_port: BasePort<ECNL_Port, SimulatedBorderPort>,
    pub ecnl_port_sub_ptr: *mut ECNL_Port_Sub,
}

#[cfg(feature = "cell")]
#[link(name = ":ecnl_proto.o")]
impl InBufferDesc {
     pub fn new() -> InBufferDesc {
          const len : usize = size_of::<Packet>();
          let mut ary: Vec<u16> = vec![0; len/2];
	  for i in 0..len/2 { ary[i] = i as u16; } // might want: i | 0x8080 ?
          let ary_FRAME : *mut u16 = ary.as_mut_ptr();
          const shortened : usize = len;// 1500 + 26; // MTU + ethernet header
	  unsafe {
	         let blob_FRAME = std::mem::transmute::<*mut u16, *mut Packet>(ary_FRAME); // magic 'cast'
		 let blob_buf : InBufferDesc = InBufferDesc {
		     len: shortened as c_uint, // u32
		     frame: blob_FRAME
		 };
                 std::mem::forget(ary);
		 return blob_buf
	  }
     }
}

#[cfg(feature="cell")]
#[link(name = ":ecnl_proto.o")]
impl ECNL_Port {
     pub fn new(port_id: u8, base_port: BasePort<ECNL_Port, SimulatedBorderPort>) -> ECNL_Port {
     	  unsafe {
              let ecnl_port_sub_ptr: *mut ECNL_Port_Sub = port_create(port_id);
              let ecnl_port = ECNL_Port {
                  base_port,
                  ecnl_port_sub_ptr,
              };
              println!("Created ECNL port #{}, {} as {}", port_id, ecnl_port.get_port_name(), (*ecnl_port_sub_ptr).port_id);
              return ecnl_port;
	  }
     }
     pub fn is_connected(&self) -> bool {
         unsafe {
             let ecnl_port_sub = (*(self.ecnl_port_sub_ptr));
	     return ecnl_port_sub.port_up_down > 0;
         }
     }
     pub fn refresh_connected_status(&self) {
         unsafe {
	     return port_update(self);
	 }
     }
     pub fn retrieve(&self, bdp: &mut InBufferDesc) -> Option<Result<Packet, Error>> {
         let _f = "retrieve";
         println!("Retrieving Packet...");
	 unsafe {
             port_do_read_async(self, bdp);
	     if ((*bdp).frame != null_mut() && (*bdp).len != 0) {
	         sleep(Duration::from_millis(100));
	         let packet: &Packet = &*((*bdp).frame);
                 println!("Received Packet: {}", packet.to_string()); // Probably usually sufficient to print ec_msg_type.
	         return Some(Ok(*packet));
	     } else {
	         return None;
	     }
	 }
     }
    pub fn get_port_name(&self) -> String {
        unsafe {
            return CStr::from_ptr((*(self.ecnl_port_sub_ptr)).port_name).to_string_lossy().into_owned();
        }
    }
}

#[cfg(feature = "cell")]
impl InteriorPortLike for ECNL_Port {
     fn send(self: &mut Self, packet: &mut Packet) -> Result<(), Error> {
        let bufferDesc: OutBufferDesc = OutBufferDesc {
	    len: size_of::<Packet>() as c_uint, // Always send fixed-length frames
	    frame: packet,
	};
	println!("Sending Packet: {}", packet.to_string()); // Probably usually sufficient to print ec_msg_type.
        unsafe {
	    port_do_xmit(self, &bufferDesc)
	}
	return Ok(())
    }
     fn listen(self: &mut Self, port_to_pe: PortToPe) -> Result<(), Error> {
         let _f = "listen";
         unsafe {
             let ecnl_port_sub = (*(self.ecnl_port_sub_ptr));
	     println!("Listening for events on port {}",  ecnl_port_sub.port_id);
	     let mut bd = InBufferDesc::new();
             loop {
                 let mut event : ECNL_Event;
                 event = std::mem::uninitialized();
                 port_get_event(self, &mut event);
		 match event.event_cmd_id {
		     cmd_id if (cmd_id == NL_ECND_Commands::NL_ECNL_CMD_GET_PORT_STATE as c_int) => {
                         let mut port_status_name: &str;
                         if (event.event_up_down != 0) {
                             port_status_name = "up";
                             self.base_port.set_connected();
                         } else {
                             port_status_name = "down";
                             self.base_port.set_disconnected();
                         }
			 println!("Port {} is {}", ecnl_port_sub.port_id, port_status_name);
			port_to_pe.send(PortToPePacket::Status((PortNo(ecnl_port_sub.port_id), self.base_port.is_border(), if (event.event_up_down != 0) {PortStatus::Connected} else {PortStatus::Disconnected}))).unwrap();
		    }
		    cmd_id if (cmd_id == NL_ECND_Commands::NL_ECNL_CMD_SIGNAL_AIT_MESSAGE as c_int) => {
                        println!("AIT Message Signal Received...");
			let mut first: bool = true; // Require at least one packet
			while true {
			    let possible_packet_or_err: Option<Result<Packet, Error>> = self.retrieve(&mut bd);
			    match possible_packet_or_err {
		                Some(packet_or_err) => {
				    port_to_pe.send(PortToPePacket::Packet((PortNo(ecnl_port_sub.port_id), packet_or_err?))).unwrap();
				    first = false;
				},
				None => {
				    if first {
				        return Err(ECNL_PortError::NoPacketRetrieved { func_name: _f, port_id: ecnl_port_sub.port_id }.into())
				    }
				    break;
				},
		            }
			}
		    }
		    _ => {
			return Err(ECNL_PortError::UnknownCommand { func_name: _f, cmd_id: event.event_cmd_id}.into());
		    }
		}
            }
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
    pub event_cmd_id: c_int,
    pub event_n_msgs: u32,
    pub event_up_down: c_int,
}

#[cfg(feature = "cell")]
#[link(name = ":port.o")]
extern "C" {
    pub fn ecnl_init(debug: bool) -> ::std::os::raw::c_int;
    pub fn port_create(port_id: u8) -> *mut ECNL_Port_Sub;
    pub fn port_destroy(port: *const ECNL_Port_Sub);

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
    #[fail(display = "ECNL_PortError::NoPacketRetrieved {}: No packet retrieved on port {}", func_name, port_id)]
    NoPacketRetrieved { func_name: &'static str, port_id: u8},
}
