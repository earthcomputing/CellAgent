use std::{fmt,
          collections::{HashSet, HashMap},
          ops::Deref,
          thread::ThreadId
};

use serde_json;
use serde_json::{Value};
use strum_macros::EnumIter;

use lazy_static::lazy_static;
use time;

use crate::config::{CONFIG, PAYLOAD_DEFAULT_ELEMENT, REPO,
                    CellQty, MASK_MAX, MaskValue, PortQty};
use crate::uuid_ec::Uuid;

pub const BASE_TENANT_MASK: Mask = Mask { mask: MaskValue(MASK_MAX) };     // All ports
pub const DEFAULT_USER_MASK: Mask = Mask { mask: MaskValue(MASK_MAX-1) };  // All ports except port 0
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Mask { mask: MaskValue }
impl Mask {
    pub fn new(port_number: PortNumber) -> Mask {
        let mask = MaskValue((1 as u16).rotate_left((*port_number.get_port_no()) as u32));
        Mask { mask }
    }
    pub fn port0() -> Mask { Mask { mask: MaskValue(1) } }
    pub fn empty() -> Mask { Mask { mask: MaskValue(0) } }
    pub fn all_but_zero(no_ports: PortQty) -> Mask {
        Mask { mask: MaskValue((2 as u16).pow((*no_ports) as u32)-2) }
    }
    pub fn _equal(self, other: Mask) -> bool { *self.mask == *other.mask }
    //pub fn get_as_value(&self) -> MaskValue { self.mask }
    pub fn or(self, mask: Mask) -> Mask {
        Mask { mask: MaskValue(*self.mask | *mask.mask) }
    }
    pub fn and(self, mask: Mask) -> Mask {
        Mask { mask: MaskValue(*self.mask & *mask.mask) }
    }
    pub fn not(self) -> Mask {
        Mask { mask: MaskValue(!*self.mask) }
    }
    pub fn all_but_port(self, port_number: PortNumber) -> Mask {
        let port_mask = Mask::new(port_number);
        self.and(port_mask.not())
    }
    pub fn make(port_numbers: &HashSet<PortNumber>) -> Mask {
        port_numbers
            .iter()
            .fold(Mask::empty(), |mask, port_number|
                mask.or(Mask::new(*port_number)) )
    }
    pub fn get_port_nos(self) -> Vec<PortNo> {
        (0..=*CONFIG.max_num_phys_ports_per_cell)
            .map(|i| PortNo(i).make_port_number(CONFIG.max_num_phys_ports_per_cell)
                .expect("Mask make_port_number cannont generate an error"))
            .map(|port_number| Mask::new(port_number))
            .enumerate()
            .filter(|(_, test_mask)| *test_mask.mask & *self.mask != 0)
            .map(|(i, _)| PortNo(i as u8))
            .collect::<Vec<_>>()
    }
}
impl Default for Mask { fn default() -> Self { Mask::empty() } }
impl fmt::Display for Mask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, " {:016.b}", *self.mask)
    }
}
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PortNo(pub u8);
impl PortNo {
    pub fn make_port_number(self, no_ports: PortQty) -> Result<PortNumber, Error> {
        Ok(PortNumber::new(self, no_ports)?)
    }
    pub fn as_usize(self) -> usize { self.0 as usize }
}
impl Default for PortNo { fn default() -> Self { PortNo(0) } }
impl Deref for PortNo { type Target = u8; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PortNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P:{}", self.0)
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum CellType {
    Border,
    Interior
}
impl Default for CellType {
    fn default() -> CellType { CellType::Interior }
}
impl fmt::Display for CellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            CellType::Border   => "Border",
            CellType::Interior => "Interior",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortNumber { port_no: PortNo }
impl PortNumber {
    pub fn new(no: PortNo, no_ports: PortQty) -> Result<PortNumber, Error> {
        if *no > *no_ports {
            Err(UtilityError::PortNumber{ port_no: no, func_name: "PortNumber::new", max: no_ports }.into())
        } else {
            Ok(PortNumber { port_no: (no as PortNo) })
        }
    }
    pub fn new0() -> PortNumber { PortNumber { port_no: (PortNo(0)) } }
    pub fn get_port_no(self) -> PortNo { self.port_no }
    pub fn as_usize(self) -> usize { *self.port_no as usize }
}
impl fmt::Display for PortNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", *self.port_no) }
}
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Path { port_number: PortNumber }
impl Path {
    pub fn new(port_no: PortNo, no_ports: PortQty) -> Result<Path, Error> {
        let port_number = port_no.make_port_number(no_ports).context(UtilityError::Chain { func_name: "Path::new", comment: S("")})?;
        Ok(Path { port_number })
    }
    pub fn new0() -> Path { Path { port_number: PortNumber::new0() } }
    pub fn get_port_number(self) -> PortNumber { self.port_number }
    pub fn get_port_no(self) -> PortNo { self.port_number.get_port_no() }
}
impl fmt::Display for Path {
    fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.port_number) }
}
// I could just use Vec, but this way I'm sure I don't do anything other than push, pop, and len
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack<T: Sized> {
    elements: Vec<T>
}
impl<T: Sized> Stack<T> {
    pub fn new() -> Stack<T> { Stack { elements: vec![] } }
    pub fn _push(&mut self, element: T) { self.elements.push(element); }
    pub fn _pop(&mut self) -> Option<T> { self.elements.pop() }
    pub fn iter(&self) -> core::slice::Iter<'_, T> { self.elements.iter() }
}
/*

THDR - "trace_header":{"thread_id":[0-9]*,"event_id":[[0-9]*]*,"trace_type":["Trace","Debug"]},
CELLID - "cell_id":{"name":"C:[0-9]*","uuid":{"uuid":\[[0-9]*,0\]}},
FCN - "module":"[^"]*","function":"[^"]*",
COMMENT - "comment":"[^"]*"
VMID - "vm_id":{"name":"VM:C:[0-9]*+vm[0-9]*","uuid":{"uuid":[[0-9]*,0]}},
SENDER - "sender_id":{"name":"Sender:C:[0-9]*+VM:C:[0-9]*+vm[0-9]*","uuid":{"uuid":[[0-9]*,0]}},
PORT - "port_no":{"v":[0-9]*},"is_border":[a-z]*

{THDR, FCN, CELLID, COMMENT}
{THDR, FCN, CELLID, PORT}
{THDR, FCN, CELLID, VMID, SENDER, COMMENT}
{THDR, FCN, COMMENT}

*/
lazy_static!{
    static ref STARTING_EPOCH: u64 = timestamp();
}
use std::thread;
#[derive(Debug, Clone, Serialize)]
pub struct TraceHeader {
    starting_epoch: u64,
    epoch: u64,
    spawning_thread_id: u64,
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
        TraceHeader { starting_epoch: *STARTING_EPOCH, epoch,
            thread_id, spawning_thread_id: thread_id,
            event_id: vec![0], trace_type: TraceType::Trace,
            module: "", line_no: 0, function: "", format: "", repo: REPO }
    }
    pub fn starting_epoch(&self) -> u64 { self.starting_epoch }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn spawning_thread_id(&self) -> u64 { self.spawning_thread_id }
    pub fn thread_id(&self) -> u64 { self.thread_id }
    pub fn event_id(&self) -> &Vec<u64> { &self.event_id }
    pub fn trace_type(&self) -> TraceType { self.trace_type }
    pub fn module(&self) -> &str { self.module }
    pub fn line_no(&self) -> u32 { self.line_no }
    pub fn function(&self) -> &str { self.function }
    pub fn format(&self) -> &str { self.format }
    pub fn repo(&self) -> &str { self.repo }

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
        TraceHeader { starting_epoch: *STARTING_EPOCH, epoch: timestamp(),
            thread_id, spawning_thread_id: thread_id,
            event_id, trace_type: self.trace_type,
            module: self.module, line_no: self.line_no, function: self.function, format: self.format, repo: REPO }
    }
    pub fn update(&mut self, params: &TraceHeaderParams) {
        self.module    = params.get_module();
        self.line_no   = params.get_line_no();
        self.function  = params.get_function();
        self.format    = params.get_format();
        self.epoch     = timestamp();
        self.thread_id = TraceHeader::parse(thread::current().id());
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
    let t = (timespec.sec as f64) + (timespec.nsec as f64/1000./1000./1000.);
    (t*1000.0*1000.0) as u64
}
pub fn sleep(n: u64) { // Sleep for n seconds
    let sleep_time = ::std::time::Duration::from_secs(n);
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Thread id {}, Event id {:?}", self.thread_id, self.event_id) }
}
#[derive(Debug, Copy, Clone, Serialize)]
pub enum TraceType {
    Trace,
    Debug,
}
impl fmt::Display for TraceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Trace type {}", match *self {
            TraceType::Trace => "Trace",
            TraceType::Debug => "Debug"
        })
    }
}
#[derive(Debug, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum CellConfig { Small, Medium, Large }
impl Default for CellConfig {
    fn default() -> CellConfig { CellConfig::Large }
}
impl fmt::Display for CellConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            CellConfig::Small  => "Small",
            CellConfig::Medium => "Medium",
            CellConfig::Large  => "Large"
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Default)]
pub struct CellInfo {   // Any data the cell agent wants to expose to applications
    external_id: Uuid   // An externally visible identifier so applications can talk about individual cells
}
impl CellInfo {
    pub fn new() -> CellInfo {
        Default::default()
    }
    pub fn get_external_id(&self) -> Uuid {
        return self.external_id;
    }
}
impl fmt::Display for CellInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "External ID {}", self.external_id)
    }
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ByteArray { bytes: Vec<u8> }
impl ByteArray {
    pub fn new(str_ref: &str) -> ByteArray {
        ByteArray { bytes: S(str_ref).into_bytes() }
    }
    pub fn new_from_bytes(bytes: &Vec<u8>) -> ByteArray {
        ByteArray { bytes: bytes.clone() }
    }
    pub fn get_bytes(&self) -> &Vec<u8> { &self.bytes }
    pub fn len(&self) -> usize { self.bytes.len() }
    pub fn to_string(&self) -> Result<String, Error> {
        let string = std::str::from_utf8(&self.bytes)?;
        let default_as_char = PAYLOAD_DEFAULT_ELEMENT as char;
        Ok(string.replace(default_as_char, ""))
    }
}
impl fmt::Display for ByteArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = std::str::from_utf8(&self.bytes).expect("ByteArray: Error converting bytes to str");
        write!(f, "{}", bytes)
    }
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, EnumIter)]
pub enum Quench { Simple, RootPort, MyPort }
impl fmt::Display for Quench {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Quench::Simple   => write!(f, "Simple"),
            Quench::RootPort => write!(f, "RootPort"),
            Quench::MyPort   => write!(f, "MyPort")
        }
    }
}
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellNo(pub usize);
impl Deref for CellNo { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for CellNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C:{}", self.0)
    }
}
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Edge(pub CellNo, pub CellNo);
impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E: ({}, {})", *self.0, *self.1)
    }
}

// Utility functions
pub fn is2e(i: usize, j: usize) -> Edge { Edge(CellNo(i), CellNo(j)) }
pub fn _print_vec<T: fmt::Display>(vector: &[T]) {
    for (count, v) in vector.iter().enumerate() { println!("{:3}: {}", count, v) }
}
pub fn _print_hash_set<T: fmt::Display + Eq + std::hash::Hash>(hashset: &HashSet<T>) {
    for v in hashset.iter() { println!("{}", v); }
}
pub fn print_hash_map<K: fmt::Display + Eq + std::hash::Hash, T: fmt::Display + Eq + std::hash::Hash>(hashmap: &HashMap<K, T>) {
    for (k, v) in hashmap.iter() { println!("{}: {}", k, v); }
}
pub fn new_hashset<T: Clone + Eq + std::hash::Hash>(values: &[T]) -> HashSet<T> {
    values.iter().cloned().collect::<HashSet<T>>()
}
pub fn vec_from_hashset<T: Clone + Eq + std::hash::Hash>(values: &HashSet<T>) -> Vec<T> {
    let mut vec = Vec::new();
    for item in values.into_iter() { vec.push(item.clone()); }
    vec
}
pub fn write_err(caller: &str, e: &Error) {
    use ::std::io::Write;
    let stderr = &mut ::std::io::stderr();
    let _ = writeln!(stderr, "*** {}: {}", caller, e);
    for cause in e.iter_chain() {
        println!("*** Caused by {}", cause);
    }
    let fail: &dyn Fail = e.as_fail();
    if fail.cause().and_then(|cause| cause.backtrace()).is_some() {
        let _ = writeln!(stderr, "---> Backtrace available: uncomment line in utility.rs containing --->");
        // let _ = writeln!(stderr, "Backtrace: {:?}", backtrace); // --->
    }
}
pub fn _string_to_object(string: &str) -> Result<Value, Error> {
    let _f = "_string_to_object";
    let v = serde_json::from_str(string).context(UtilityError::Chain { func_name: _f, comment: S("") })?;
    Ok(v)
}
// Used to get nice layouts in display - gives (row,col) for each cell
pub fn get_geometry(num_cells: CellQty) -> (usize, usize) {
    // Keep these definitions here so they don't get lost, actually defined in config file
    let _geometry = match num_cells {
        CellQty(3)  => vec![(0,0), (0,2), (1,1)],
        CellQty(4)  => vec![(0,0), (0,1), (1,0), (1,1)],
        CellQty(10) => vec![(0,0), (0,1), (0,2), (0,3), (0,4),
                            (1,0), (1,1), (1,2), (1,3), (1,4)],
        CellQty(47) => vec![(0,2), (0,4), (0,5), (0,7), (0,9), (0,10),
                            (1,0), (1,1), (1,2), (1,3), (1,4), (1,5), (1,7), (1,8), (1,9), (1,10),
                            (2,0), (2,1), (2,2), (2,3), (2,4), (2,6), (2,7), (2,8), (2,9),
                            (3,1), (3,2), (3,4), (3,5), (3,8), (3,9), (3,10),
                            (4,0), (4,1), (4,2), (4,3), (4,4), (4,6), (4,7), (4,8), (4,9),
                            (5,0), (5,1), (5,2), (5,3), (5,4), (5,6)],
        _ => vec![]
    };
    let max_x = CONFIG.geometry
        .iter()
        .max_by_key(|xy| xy.0)
        .map(|xy| xy.0 +1)
        .unwrap_or(0);
    let max_y = CONFIG.geometry
        .iter()
        .max_by_key(|xy| xy.1)
        .map(|xy| xy.1 + 1)
        .unwrap_or(0);
    if CONFIG.geometry.len() != *num_cells { panic!(format!("Topology has {} entries for {} cells", CONFIG.geometry.len(), *num_cells)) };
    (max_x, max_y)
}
pub fn _dbg_get_thread_id() -> u64 { TraceHeader::parse(thread::current().id()) }
// There are so many places in my code where it's more convenient
// to provide &str but I need String that I made the following
#[allow(non_snake_case)]
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
    PortNumber { port_no: PortNo, func_name: &'static str, max: PortQty },
//    #[fail(display = "UtilityError::Serialize {}: Cannot serialize in append2file", func_name)]
//    Serialize { func_name: &'static str},
    #[fail(display = "UtilityError::Unimplemented {}: {} is not implemented", func_name, feature)]
    Unimplemented { feature: String, func_name: &'static str }
}
