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
    port_state_ptr_vector: Vec<*const PortState>,
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
    pub fn ecnl_get_port_state(nl_session: *mut c_void, port_id: c_uint, pspp: *const *const PortState) -> c_int;
    pub fn free_nl_session(nl_session: *mut c_void) -> c_int;
}

impl ECNL_Session {
    pub fn new() -> ECNL_Session {
        let nsp: *mut c_void = null_mut();
        let mip: *const ModuleInfo = null();
        let mut pspv: Vec<*const PortState>;
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
            pspv = Vec::with_capacity(num_ports as usize);
            let psp: *const PortState = null();
            for i in 0..=num_ports-1 {
                pspv.push(psp);
                ecnl_get_port_state(nsp, i as u32, &(pspv[i as usize]));
            }
        }
        #[cfg(feature = "simulator")] {
            pspv = Vec::new();
        }
        ECNL_Session {
            nl_session: nsp,
            module_info_ptr: mip,
            port_state_ptr_vector: pspv,
        }
    }
    pub fn num_ecnl_ports(&self) -> PortQty {
        unsafe {
            PortQty((*(self.module_info_ptr)).num_ports as u8)
        }
    }
    pub fn port_is_connected(&self, port_id: u8) -> bool {
        unsafe {
            (*(*(self.port_state_ptr_vector))[port_id as usize]).port_link_state > 0
        }
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
