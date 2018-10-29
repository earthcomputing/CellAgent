use std::fmt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json;
//use uuid::Uuid;

extern crate backtrace;

#[derive(Debug, Clone, Serialize)]
pub struct CFrame {
    p : String,
    f : String,
    a : u64
}
impl CFrame {
pub fn new(path: String, name: String, ip: u64) -> CFrame {
    CFrame { p: path, f: name, a: ip }
}
}
impl fmt::Display for CFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("CFrame {} {} {}", self.p, self.f, self.a);
        write!(f, "{}", s)
    }
}
pub fn captureStack() -> Vec<CFrame> {
    let mut cframes = Vec::new();
    backtrace::trace(|frame| {
        // let symbol_address = frame.symbol_address();
        let ip = frame.ip();
        backtrace::resolve(ip, |symbol| {
            if let Some(filename) = symbol.filename() {
                if let Some(path) = filename.to_str() {
                    if path.starts_with("src/") {
                        if let Some(name) = symbol.name() {
                            let item = CFrame::new(path.to_string(), name.to_string(), ip as u64);
                            cframes.push(item);
                        }
                    }
                }
            }
        });
        true // keep going to the next frame
    });
    cframes
}
use std::thread;
use TraceHeader;
pub fn dumpstack() {
    let thread_id = TraceHeader::parse(thread::current().id());
    let mut v = captureStack();
    // trim first couple of frames
    if v[0].f.starts_with("multicell::traph::captureStack::") {
        if v[1].f.starts_with("multicell::traph::dumpstack::") {
            let chop = v.drain(2..).collect();
            v = chop;
        }
    }

//    let mut s = String::new();
//    for item in v.iter() { s += &format!("\n{}", item); }
    let s = json!({ "thread_id": &thread_id, "frames": &v });
    println!("backtrace: {}", s);
}
