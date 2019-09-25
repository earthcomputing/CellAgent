use serde_json::Value;
use serde::{Deserialize};

#[derive(Deserialize)]
pub struct TraceRecord {
    header: Header,
    pub body: Value
}
impl TraceRecord {
    pub fn header(&self) -> &Header { &self.header }
    pub fn body(&self) -> &Value { &self.body }
}

#[derive(Deserialize)]
pub struct Header {
    starting_epoch: u64,
    epoch: u64,
    spawning_thread_id: usize,
    thread_id: usize,
    event_id: Vec<usize>,
    trace_type: String,
    module: String,
    line_no: usize,
    function: String,
    format: String,
    repo: String,
}