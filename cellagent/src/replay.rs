use failure::{Error};
use serde_json::Value;
use std::{fmt, fmt::Write};

// Structs to parse trace records
use crate::ec_message::{Message, MsgHeader, MsgType};
use crate::name::{CellID, TreeID};
use crate::routing_table_entry::RoutingTableEntry;
use crate::utility::{ByteArray, PortNo, TraceType};
use crate::uuid_ec::Uuid;

#[derive(Debug)]
pub enum TraceFormat {
    EmptyFormat,
    CaNewFormat(CellID, TreeID, TreeID, TreeID),
    CaToCmEntryFormat(RoutingTableEntry),
    CaFromCmBytes(PortNo, bool, Uuid, ByteArray)
}

pub fn process_trace_record(mut record: String) -> Result<TraceFormat, Error> {
    let _f = "process_trace_record";
    record.pop(); // Remove trailing comma
    let trace: TraceRecord = serde_json::from_str(&record)?;
    let format = trace.header.format;
    let trace_format = match format.as_str() {
        "ca_new" => {
            let ca_new: CaNew = serde_json::from_value(trace.body)?;
            TraceFormat::CaNewFormat(ca_new.cell_id, ca_new.my_tree_id,
                               ca_new.connected_tree_id, ca_new.control_tree_id)
        },
        "ca_to_cm_entry" => {
            let ca_to_cm_entry: CaToCmEntry = serde_json::from_value(trace.body)?;
            TraceFormat::CaToCmEntryFormat(ca_to_cm_entry.entry)
        },
        "ca_from_cm_bytes" => {
            let m2a: CaFromCmBytes = match serde_json::from_value(trace.body) {
                Ok(m) => m,
                Err(e) => {
                    println!("Replay {} error {}", format, e);
                    return Err(e.into());
                }
            };
            TraceFormat::CaFromCmBytes(m2a.port, m2a.is_ait, m2a.uuid, m2a.bytes)
        }
        _ => TraceFormat::EmptyFormat
    };
    Ok(trace_format)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceHeader {
    starting_epoch: u64,
    epoch: u64,
    spawning_thread_id: u64,
    thread_id: u64,
    event_id: Vec<u64>,
    trace_type: TraceType,
    module: String,
    line_no: u32,
    function: String,
    format: String,
    repo: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecord {
    header: TraceHeader,
    body: Value
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecordCaNew {
    header: TraceHeader,
    body: CaNew
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaNew {
    cell_id: CellID,
    connected_tree_id: TreeID,
    control_tree_id: TreeID,
    my_tree_id: TreeID
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecordCaToCmEntry {
    header: TraceHeader,
    body: CaToCmEntry
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaToCmEntry {
    entry: RoutingTableEntry
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecordCaFromCmBytes {
    header: TraceHeader,
    body: CaFromCmBytes
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaFromCmBytes {
    is_ait: bool,
    port: PortNo,
    uuid: Uuid,
    bytes: ByteArray
}
impl fmt::Display for CaFromCmBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = "CaFromCmBytes".to_string();
        write!(s, "is_ait {} ", self.is_ait)?;
        write!(s, "port_no {}", self.port)?;
        write!(s, "tree_uuid {}", self.uuid)?;
        let msg = serde_json::to_string(&self.bytes).expect("Replay: serde problem");
        write!(s, "msg {}", msg)?;
        write!(f, "{}", s)
    
    }
}
#[derive(Debug, Fail)]
pub enum ReplayError {
    #[fail(display = "ReplayError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
