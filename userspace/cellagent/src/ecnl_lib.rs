/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
extern crate crossbeam;
extern crate futures;
extern crate libc; // use libc::size_t;
extern crate serde;
extern crate serde_derive;
#[allow(unused_imports)]
#[macro_use] extern crate serde_json;

mod ecnl_endpoint;

#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
#[allow(unused_parens)]
#[cfg(feature = "test")]
mod tests {
    use std::ffi::{CString, CStr};
    use crate::ecnl_endpoint::ept;

    fn build_asciz_buf() -> ept::buf_desc_t {
        let str : &'static str = "Plain Text Message"; // 506c61696e2054657874204d65737361676500
        let encoded : std::str::EscapeDefault<'_> = str.escape_default(); // escape_unicode();
        let string : std::string::String = encoded.to_string();
        let c_string : std::ffi::CString = CString::new(string).unwrap();
        let bytes : &[u8] = c_string.as_bytes_with_nul();
        let len : usize = bytes.len(); // includes NUL
        assert_eq!(19, len);
        let asciz_FRAME : *const u8 = bytes.as_ptr();
        std::mem::forget(c_string); // rather than bytes
        let asciz_buf : ept::buf_desc_t = ept::buf_desc_t{len: len as u32, frame: asciz_FRAME as *mut _}; // *mut u8
        // println!("len {}", asciz_buf.len);
        return asciz_buf;
    }

    // extra test - full binary buffer
    // char ecad_data[EC_MESSAGE_MAX]; // 9000
    fn build_blob_buf() -> ept::buf_desc_t {
        const len : usize = 9000 / 2;
        let mut ary: Vec<u16> = vec![0; len];
        for i in 0..len { ary[i] = i as u16; } // might want: i | 0x8080 ?
        let ary_FRAME : *mut u16 = ary.as_mut_ptr();
        const shortened : usize = 1500 + 26; // MTU + ethernet header
        unsafe {
            let blob_FRAME = std::mem::transmute::<*mut u16, *mut u8>(ary_FRAME); // magic 'cast'
            let blob_buf : ept::buf_desc_t = ept::buf_desc_t{len: shortened as u32, frame: blob_FRAME};
            std::mem::forget(ary);
            return blob_buf;
        }
    }

    // CStr::from_bytes_with_nul

    fn dump_ept(ept: *const ept::ecnl_endpoint_t) {
        unsafe {
            let ept_port_id = (*ept).ept_port_id;
            let ept_name = CStr::from_ptr((*ept).ept_name).to_string_lossy().into_owned();
            let ept_up_down = (*ept).ept_up_down;
            let carrier = if (ept_up_down != 0) { "UP" } else { "DOWN" };
            println!("ept {} {} {}", ept_port_id, ept_name, carrier);
        }
    }

    fn dump_buf(tag: &'static str, buf: ept::buf_desc_t) {
        let len : u32 = buf.len;
        let _frame : *const u8 = buf.frame;
        println!("{} buf {}", tag, len);

    }

use std::thread;

    fn event_loop(ept: *const ept::ecnl_endpoint_t, _tx: crossbeam::Sender<String>) {
        let worker = thread::current();
        // let tid = format!("{:?}", worker.id()); // looks like : "ThreadId(8)"
        let name = worker.name().unwrap(); // name is guaranteed

        loop {
            let mut event : ept::ecnl_event_t;
            unsafe { 
                event = std::mem::uninitialized();
                ept::ept_get_event(ept, &mut event);
            }

            let ept_port_id; unsafe { ept_port_id = (*ept).ept_port_id; }
            if (event.event_port_id == ept_port_id) {
                let carrier = if (event.event_up_down != 0) { "UP" } else { "DOWN" };
                let ref body = json!({
                    "thread": name,
                    "module_id": event.event_module_id,
                    "port_id": event.event_port_id,
                    "cmd_id": event.event_cmd_id,
                    "n_msgs": event.event_n_msgs,
                    "carrier": carrier,
                });
                println!("{}", body);
            }

            // if received == "exit" { return; } // FIXME: how do we terminate this thread??
            // tx.send();
        }
    }

    fn event_listener(ept: *const ept::ecnl_endpoint_t, tx: crossbeam::Sender<String>) -> thread::JoinHandle<String> {
        let ept_port_id;
        unsafe { ept_port_id = (*ept).ept_port_id; }
        let ept2; unsafe { ept2  = (*ept); }
        let ept_name; unsafe {ept_name = CStr::from_ptr((*ept).ept_name).to_string_lossy().into_owned(); }
        let thread_name = format!("{} ({}) event_loop", ept_name, ept_port_id);
        let h = thread::Builder::new().name(thread_name.into()).spawn(move || {
            let ept_ref : &ept::ecnl_endpoint_t = &ept2;
            let ept_ptr : *const ept::ecnl_endpoint_t = ept_ref;
            event_loop(ept_ptr, tx);
            let worker = thread::current();
            format!("{:?} {}", worker.id(), worker.name().unwrap())
        }).unwrap();
        h
    }

use crossbeam::crossbeam_channel::unbounded as channel;

    #[test]
    fn it_works() {
        let mut channels: Vec<crossbeam::Receiver<String>> = Vec::new();
        let mut handles: Vec<thread::JoinHandle<String>> = Vec::new();
        unsafe {
            let num_ports = ept::ecnl_init(false);
            for port_id in 0..num_ports {
                println!();
                let ept = ept::ept_create(port_id as u32);
                dump_ept(ept);

                // creates an event thread that orginates 'status' application msgs posted to 'tx':
                let (tx, rx) = channel();
                channels.push(rx);
                let h = event_listener(ept, tx);
                handles.push(h);

                ept::ept_update(ept);
                dump_ept(ept);

                let asciz_buf = build_asciz_buf();
                let blob_buf = build_blob_buf();

                dump_buf("asciz", asciz_buf);
                dump_buf("blob", blob_buf);

                let p1 : *const ept::buf_desc_t = &asciz_buf;
                let p2 : *const ept::buf_desc_t = &blob_buf;

                // back-to-back send of an asciz and a blob:
                ept::ept_do_xmit(ept, p1 as *mut _);
                ept::ept_do_xmit(ept, p2 as *mut _);

                // FIXME: data buffer allocated by lib
                let read1_buf = build_blob_buf();
                let p3 : *const ept::buf_desc_t = &read1_buf;
                ept::ept_do_read_async(ept, p3 as *mut _);
                println!("async: {}", (*p3).len);

                // comsumes data w/o library print
                if (*p3).len > 0 {
                    let c_string : std::ffi::CString = CString::new("async dumpbuf").unwrap();
                    let tag = c_string.as_ptr();
                    ept::ept_dumpbuf(ept, tag as *mut _, p3 as *mut _);
                }

                let num_secs = 10;
                let read2_buf = build_blob_buf();
                let p4 : *const ept::buf_desc_t = &read2_buf;
                ept::ept_do_read(ept, p4 as *mut _, num_secs);
                println!("sync: {}", (*p4).len);

                ept::ept_destroy(ept);
            }
        }

        // non-optimal latency for set-join
        let mut hist: Vec<Result<String, _>> = Vec::new();
        while let Some(h) = handles.pop() {
            let rc = h.join();
            hist.push(rc);
        }

        for rc in hist.iter() {
            let outcome = format!("{:?}", rc);
            let ref body = json!({ "outcome": outcome });
            println!("{}", body);
        }

        assert_eq!(2 + 2, 4);
    }
}
