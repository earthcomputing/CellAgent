use failure::{Error, ResultExt};
use serde_json::Value;

// Structs to parse trace records
use crate::name::{CellID, TreeID};
use crate::routing_table_entry::RoutingTableEntry;
use crate::utility::{TraceType};

pub enum TraceFormat {
    EmptyFormat,
    CaNewFormat(CellID, TreeID, TreeID, TreeID),
    CaToCmEntryFormat(RoutingTableEntry),
} 

pub fn process_trace_record(mut record: String) -> Result<TraceFormat, Error> {
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

#[derive(Debug, Fail)]
pub enum ReplayError {
    #[fail(display = "ReplayError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
