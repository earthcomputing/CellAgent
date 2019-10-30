#[cfg(feature = "cell")]
use libc::{free};
#[cfg(feature = "cell")]
use std::{
    ffi::CStr,
};

use std::{
    os::raw::{c_char, c_int, c_uchar, c_uint, c_void},
    ptr::{null, null_mut},
};

use crate::config::{PortQty};

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
        #[cfg(feature = "cell")]
        unsafe {
            alloc_nl_session(&nsp);
            ecnl_get_module_info(nsp, &mip as *const *const ModuleInfo);
            let module_id = (*mip).module_id;
            println!("Module id: {:?} ", module_id);
            let module_name = CStr::from_ptr((*mip).module_name).to_string_lossy().into_owned();
            println!("Module name: {:?} ", module_name);
        }
        ECNL_Session {
            nl_session: nsp,
            module_info_ptr: mip,
        }
    }
    pub fn get_num_ecnl_ports(&self) -> PortQty {
        unsafe {
            PortQty((*(self.module_info_ptr)).num_ports as u8)
        }
    }
}

unsafe impl Send for ECNL_Session {}

impl Drop for ECNL_Session {
    fn drop(&mut self) {
        #[cfg(feature = "cell")]
        unsafe {
            free_nl_session((*self).clone().nl_session);
            free(self.module_info_ptr as *mut libc::c_void);
        }
    }
}