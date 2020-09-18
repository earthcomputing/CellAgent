use serde::{Deserialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct TraceRecord {
    header: TraceHeader,
    pub body: Value
}
impl TraceRecord {
    pub fn header(&self) -> &TraceHeader { &self.header }
    pub fn body(&self) -> &Value { &self.body }
}

#[derive(Debug, Deserialize)]
pub struct TraceHeader {
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
impl TraceHeader {
    pub fn format(&self) -> &str { &self.format }
}