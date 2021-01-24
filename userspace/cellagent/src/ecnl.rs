use failure::{Error, ResultExt, Fail};
#[cfg(feature = "cell")]
use libc::{free};
#[cfg(feature = "cell")]
use std::{
    ffi::CStr,
};

use std::{
    collections::{HashMap},
    os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_void},
    ptr::{null, null_mut},
    thread,
};

use crate::config::{CONFIG, PortQty};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{PortFromPe};
use crate::ecnl_port::{ECNL_Port};
use crate::nalcell::{NalCell};
use crate::simulated_border_port::{SimulatedBorderPort};
use crate::utility::{PortNo, TraceHeader, TraceHeaderParams, TraceType};

#[derive(Debug)]
#[repr(C)]
pub struct ModuleInfo {
    module_id: c_uint,
    module_name: *const c_char,
    num_ports: c_uint,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ECNL_Session {
    #[allow(dead_code)]
    nl_session: *mut c_void,
    module_info_ptr: *const ModuleInfo,
    ecnl_port_ptr_vector: Vec<ECNL_Port>,
}

#[derive(Debug)]
#[repr(C)]
pub struct BuffDesc {
    len: c_uint,
    frame: *const c_uchar,
}


#[cfg(feature = "cell")]
#[allow(improper_ctypes)]
#[link(name = ":ecnl_sdk.o")]
#[link(name = ":ecnl_proto.o")]
#[link(name = ":libnl-3.so")]
#[link(name = ":libnl-genl-3.so")]
extern {
    pub fn alloc_nl_session(nl_session_ptr: *const *mut c_void) -> c_int;
    pub fn ecnl_get_module_info(nl_session: *mut c_void, mipp: *const *const ModuleInfo) -> c_int;
    pub fn free_nl_session(nl_session: *mut c_void) -> c_int;
}

impl ECNL_Session {
    pub fn new() -> ECNL_Session {
        let nsp: *mut c_void = null_mut(); // initialization required to keep Rust compiler happy
        let mip: *const ModuleInfo = null(); // initialization required to keep Rust compiler happy
        let eppv: Vec<ECNL_Port>;
        #[cfg(feature = "cell")]
        unsafe {
            alloc_nl_session(&nsp);
            ecnl_get_module_info(nsp, &mip as *const *const ModuleInfo);
        }
        let ecnl_session: ECNL_Session;
        #[cfg(any(feature = "noc", feature = "simulator"))] {
            return ECNL_Session {
                nl_session: null_mut(),
                module_info_ptr: null(),
                ecnl_port_ptr_vector: Vec::new(),
            };
        }
        unsafe {
            let num_ports = ((*mip).num_ports as u8);
            eppv = Vec::with_capacity(num_ports as usize);
            let ecnl_session: ECNL_Session = ECNL_Session {
                nl_session: nsp,
                module_info_ptr: mip,
                ecnl_port_ptr_vector: eppv,
            };
            println!("Created ECNL session for module #{}, {} with {} ECNL ports", (*mip).module_id, ecnl_session.get_module_name(), num_ports);
            return ecnl_session
        }
    }
    pub fn num_ecnl_ports(&self) -> PortQty {
        unsafe {
            return PortQty((*(self.module_info_ptr)).num_ports as u8)
        }
    }
    pub fn get_port(&self, port_id: u8) -> ECNL_Port {
	return self.ecnl_port_ptr_vector[port_id as usize].clone();
    }
    pub fn push_port(&mut self, ecnl_port: ECNL_Port) -> () {
        self.ecnl_port_ptr_vector.push(ecnl_port);
    }
    pub fn get_module_name(&self) -> String {
        #[cfg(feature = "cell")]
        unsafe {
            return CStr::from_ptr((*(self.module_info_ptr)).module_name).to_string_lossy().into_owned();
        }
        #[cfg(any(feature = "noc", feature = "simulator"))]
        return "Simulated Module".to_string();
    }
    pub fn listen_link_and_pe_loops(&mut self, nalcell: &mut NalCell<ECNL_Port, SimulatedBorderPort>) -> Result<(), Error> {
        let _f = "link_ecnl_channels";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.ca {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                let _ = add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        #[cfg(feature="cell")]
        for port_id in 0..=*(self.num_ecnl_ports())-1 {
            let port = nalcell.get_ports()[port_id as usize].clone();
            let ecnl_port: ECNL_Port = ECNL_Port::new(port_id as u8, port.clone());
            port.listen_link_and_pe(ecnl_port.clone(), nalcell.get_port_from_pe(&PortNo(port_id as u8)));
            self.push_port(ecnl_port);
        }
        #[cfg(feature="cell")]
        println!("Linked ecnl channels");
        Ok(())
    }
}

unsafe impl Send for ECNL_Session {}
unsafe impl Sync for ECNL_Session {}

impl Drop for ECNL_Session {
    fn drop(&mut self) {
        #[cfg(feature = "cell")]
        unsafe {
            println!("CLOSING ECNL!!!");
            free_nl_session((*self).clone().nl_session);
            free(self.module_info_ptr as *mut libc::c_void);
        }
    }
}

#[derive(Debug, Fail)]
pub enum EcnlError {
    #[fail(display = "EcnlError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
