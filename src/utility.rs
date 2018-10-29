use std::fmt;
use std::collections::{HashSet};
use std::thread::ThreadId;
use serde_json;
use serde_json::{Value};

use time;

use config::{MAX_PORTS, REPO, MaskValue, PortNo};
/*
pub fn get_first_arg(a: Vec<String>) -> Option<i32> {
    if a.len() != 2 {
        None
    } else {
        match a[1].parse::<i32>() {
            Ok(x) => Some(x),
            Err(_) => None
        }
    }
}
pub fn chars_to_string(chars: &[char]) -> String {
    let mut s = String::new();
    for c in chars.iter() {
        if *c == ' ' { break; }
        s = s + &c.to_string();
    }
    s
}
*/
pub const BASE_TENANT_MASK: Mask = Mask { mask: MaskValue(255) };   // All ports
pub const DEFAULT_USER_MASK: Mask = Mask { mask: MaskValue(254) };  // All ports except port 0
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Mask { mask: MaskValue }
impl Mask {
    pub fn new(port_number: PortNumber) -> Mask {
        let mask = MaskValue((1 as u16).rotate_left((*port_number.get_port_no()) as u32));
        Mask { mask }
    }
    pub fn port0() -> Mask { Mask { mask: MaskValue(1) } }
    pub fn empty() -> Mask { Mask { mask: MaskValue(0) } }
    pub fn all_but_zero(no_ports: PortNo) -> Mask {
        Mask { mask: MaskValue((2 as u16).pow((*no_ports) as u32)-2) }
    }
    pub fn equal(&self, other: Mask) -> bool { *self.mask == *other.mask }
    //pub fn get_as_value(&self) -> MaskValue { self.mask }
    pub fn or(&self, mask: Mask) -> Mask {
        Mask { mask: MaskValue(*self.mask | *mask.mask) }
    }
    pub fn and(&self, mask: Mask) -> Mask {
        Mask { mask: MaskValue(*self.mask & *mask.mask) }
    }
    pub fn not(&self) -> Mask {
        Mask { mask: MaskValue(!*self.mask) }
    }
    pub fn all_but_port(&self, port_number: PortNumber) -> Mask {
        let port_mask = Mask::new(port_number);
        self.and(port_mask.not())
    }
    pub fn make(port_numbers: &HashSet<PortNumber>) -> Mask {
        port_numbers
            .iter()
            .fold(Mask::empty(), |mask, port_number|
                mask.or(Mask::new(*port_number)) )
    }
    pub fn get_port_nos(&self) -> Vec<PortNo> {
        (0..*MAX_PORTS)
            .map(|i| PortNo(i).make_port_number(MAX_PORTS)
                .expect("Mask make_port_number cannont generate an error"))
            .map(|port_number| Mask::new(port_number))
            .enumerate()
            .filter(|(_, test_mask)| *test_mask.mask & *self.mask != 0)
            .map(|(i, _)| PortNo(i as u8))
            .collect::<Vec<_>>()
    }
}
impl fmt::Display for Mask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, " {:016.b}", *self.mask)
    }
}
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortNumber { pub port_no: PortNo }
impl PortNumber {
    pub fn new(no: PortNo, no_ports: PortNo) -> Result<PortNumber, UtilityError> {
        if *no > *no_ports {
            Err(UtilityError::PortNumber{ port_no: no, func_name: "PortNumber::new", max: no_ports }.into())
        } else {
            Ok(PortNumber { port_no: (no as PortNo) })
        }
    }
    pub fn new0() -> PortNumber { PortNumber { port_no: (PortNo(0)) } }
    pub fn get_port_no(&self) -> PortNo { self.port_no }
}
impl fmt::Display for PortNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", *self.port_no) }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Path { port_number: PortNumber }
impl Path {
    pub fn new(port_no: PortNo, no_ports: PortNo) -> Result<Path, Error> {
        let port_number = port_no.make_port_number(no_ports).context(UtilityError::Chain { func_name: "Path::new", comment: S("")})?;
        Ok(Path { port_number })
    }
    pub fn new0() -> Path { Path { port_number: PortNumber::new0() } }
    pub fn get_port_number(&self) -> PortNumber { self.port_number }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
}
impl fmt::Display for Path {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.port_number) }
}
use std::thread;
/*

THDR - "trace_header":{"thread_id":[0-9]*,"event_id":[[0-9]*]*,"trace_type":["Trace","Debug"]},
CELLID - "cell_id":{"name":"C:[0-9]*","uuid":{"uuid":\[[0-9]*,0\]}},
FCN - "module":"[^"]*","function":"[^"]*",
COMMENT - "comment":"[^"]*"
VMID - "vm_id":{"name":"VM:C:[0-9]*+vm[0-9]*","uuid":{"uuid":[[0-9]*,0]}},
SENDER - "sender_id":{"name":"Sender:C:[0-9]*+VM:C:[0-9]*+vm[0-9]*","uuid":{"uuid":[[0-9]*,0]}},
PORT - "port_no":{"v":[0-9]*},"is_border":[a-z]*

{THDR,FCN,CELLID,COMMENT}
{THDR,FCN,CELLID,PORT}
{THDR,FCN,CELLID,VMID,SENDER,COMMENT}
{THDR,FCN,COMMENT}

*/
#[derive(Debug, Clone, Serialize)]
pub struct TraceHeader {
    epoch: u64,
    thread_id: u64,
    event_id: Vec<u64>,
    trace_type: TraceType,
    module: &'static str,
    line_no: u32,
    function: &'static str,
    format: &'static str,
    repo: &'static str,
}
impl TraceHeader {
    pub fn new() -> TraceHeader {
        let thread_id = TraceHeader::parse(thread::current().id());
        let epoch = timestamp();
        TraceHeader { epoch,
            thread_id, event_id: vec![0], trace_type: TraceType::Trace,
            module: "", line_no: 0, function: "", format: "", repo: REPO }
    }
    pub fn next(&mut self, trace_type: TraceType) {
        let last = self.event_id.len() - 1;
        self.trace_type = trace_type;
        self.event_id[last] = self.event_id[last] + 1;
    }
    pub fn fork_trace(&mut self) -> TraceHeader {
        let last = self.event_id.len() - 1;
        self.event_id[last] = self.event_id[last] + 1;
        let mut event_id = self.event_id.clone();
        event_id.push(0);
        let thread_id = TraceHeader::parse(thread::current().id());
        TraceHeader { epoch: timestamp(),
            thread_id, event_id, trace_type: self.trace_type,
            module: self.module, line_no: self.line_no, function: self.function, format: self.format, repo: REPO }
    }
    pub fn update(&mut self, params: &TraceHeaderParams) {
        self.module   = params.get_module();
        self.line_no  = params.get_line_no();
        self.function = params.get_function();
        self.format   = params.get_format();
        self.epoch    = timestamp();
    }
    pub fn get_event_id(&self) -> Vec<u64> { self.event_id.clone() }
    pub fn parse(thread_id: ThreadId) -> u64 {
        let as_string = format!("{:?}", thread_id);
        let r: Vec<&str> = as_string.split('(').collect();
        let n_as_str: Vec<&str> = r[1].split(')').collect();
        n_as_str[0].parse().expect(&format!("Problem parsing ThreadId {:?}", thread_id))
    }
}
fn timestamp() -> u64 {
    let timespec = time::get_time();
    let t = timespec.sec as f64 + (timespec.nsec as f64/1000./1000./1000.);
    (t*1000.0*1000.0) as u64
}
pub fn sleep(n: usize) { // Sleep for n seconds
    let sleep_time = ::std::time::Duration::from_secs(4);
    thread::sleep(sleep_time);
}
pub struct TraceHeaderParams {
    pub module:   &'static str,
    pub function: &'static str,
    pub line_no:  u32,
    pub format:   &'static str
}
impl TraceHeaderParams {
    pub fn get_module(&self)   -> &'static str { self.module }
    pub fn get_function(&self) -> &'static str { self.function }
    pub fn get_line_no(&self)  -> u32          { self.line_no }
    pub fn get_format(&self)   -> &'static str { self.format }
}
impl fmt::Display for TraceHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thread id {}, Event id {:?}", self.thread_id, self.event_id) }
}
#[derive(Debug, Copy, Clone, Serialize)]
pub enum TraceType {
    Trace,
    Debug,
}
impl fmt::Display for TraceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Trace type {}", match self {
            &TraceType::Trace => "Trace",
            &TraceType::Debug => "Debug"
        })
    }
}
pub fn print_vec<T: fmt::Display>(vector: &Vec<T>) {
    for (count, v) in vector.iter().enumerate() { println!("{:3}: {}", count, v) }
}
pub fn write_err(caller: &str, e: Error) {
    use ::std::io::Write;
    let stderr = &mut ::std::io::stderr();
    let _ = writeln!(stderr, "*** {}: {}", caller, e);
    for cause in e.iter_chain() {
        println!("*** Caused by {}", cause);
    }
    let fail: &Fail = e.as_fail();
    if let Some(_) = fail.cause().and_then(|cause| cause.backtrace()) {
        let _ = writeln!(stderr, "---> Backtrace available: uncomment line in utility.rs containing --->");
        // let _ = writeln!(stderr, "Backtrace: {:?}", backtrace);
    }
}
pub fn string_to_object(string: &str) -> Result<Value, Error> {
    let v = serde_json::from_str(string)?;
    Ok(v)
}
// There are so many places in my code where it's more convenient
// to provide &str but I need String that I made the following
pub fn S<T: fmt::Display>(s: T) -> String { s.to_string() }
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum UtilityError {
    #[fail(display = "UtilityError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
//    #[fail(display = "UtilityError::Mask {}: Cell {} has no tenant mask", func_name, cell_id)]
//    Mask { cell_id: CellID, func_name: &'static str},
    #[fail(display = "UtilityError::PortNumber {}: Port number {:?} is larger than the maximum of {:?}", func_name, port_no, max)]
    PortNumber { port_no: PortNo, func_name: &'static str, max: PortNo },
    #[fail(display = "UtilityError::Serialize {}: Cannot serialize in append2file", func_name)]
    Serialize { func_name: &'static str},
    #[fail(display = "UtilityError::Unimplemented {}: {} is not implemented", func_name, feature)]
    Unimplemented { feature: String, func_name: &'static str }
}
