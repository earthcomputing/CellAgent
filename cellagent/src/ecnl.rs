use either::Either;
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
};

use crate::config::{CONFIG, PortQty};
use crate::dal::{add_to_trace};
use crate::ec_message_formats::{PortFromPe};
use crate::ecnl_port::{ECNL_Port};
use crate::name::{CellID};
use crate::port::{Port};

#[derive(Debug)]
#[repr(C)]
pub struct ModuleInfo {
    module_id: c_uint,
    module_name: *const c_char,
    num_ports: c_uint,
}

#[derive(Debug)]
#[repr(C)]
pub struct PortState {
    module_name: *const c_char,
    port_name: *const c_char,
    port_link_state: c_uint,
    port_s_counter: c_ulong,
    port_r_counter: c_ulong,
    port_recover_counter: c_ulong,
    port_recovered_counter: c_ulong,
    port_entt_count: c_ulong,
    port_aop_count: c_ulong,
    num_ait_messages: c_uint,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ECNL_Session {
    #[allow(dead_code)]
    nl_session: *mut c_void,
    module_info_ptr: *const ModuleInfo,
    ecnl_port_ptr_vector: Vec<ECNL_Port>,
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
        let nsp: *mut c_void = null_mut();
        let mip: *const ModuleInfo = null();
        let mut eppv: Vec<ECNL_Port>;
        #[cfg(feature = "cell")]
        unsafe {
            alloc_nl_session(&nsp);
            ecnl_get_module_info(nsp, &mip as *const *const ModuleInfo);
            let module_id = (*mip).module_id;
            println!("Module id: {:?} ", module_id);
            let module_name = CStr::from_ptr((*mip).module_name).to_string_lossy().into_owned();
            println!("Module name: {:?} ", module_name);
            let num_ports = ((*mip).num_ports as u8);
            println!("Num ecnl ports: {} ", num_ports);
            eppv = Vec::with_capacity(num_ports as usize);
            for port_id in 0..=num_ports-1 {
                eppv.push(ECNL_Port::new(port_id as u8));
		println!("Created ECNL port {} as {}", port_id, eppv[port_id as usize].port_id);
            }
        }
        #[cfg(feature = "simulator")] {
            eppv = Vec::new();
        }
        return ECNL_Session {
            nl_session: nsp,
            module_info_ptr: mip,
            ecnl_port_ptr_vector: eppv,
        };
    }
    pub fn num_ecnl_ports(&self) -> PortQty {
        unsafe {
            return PortQty((*(self.module_info_ptr)).num_ports as u8)
        }
    }
    pub fn get_port(&self, port_id: u8) -> ECNL_Port {
	return self.ecnl_port_ptr_vector[port_id as usize];
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
