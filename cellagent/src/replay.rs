use failure::{Error};
use serde_json::Value;
use std::{fmt, fmt::Write};

// Structs to parse trace records
use crate::ec_message::{MsgType};
use crate::name::{CellID, TreeID};
use crate::packet_engine::NumberOfPackets;
use crate::port::PortStatus;
use crate::routing_table_entry::RoutingTableEntry;
use crate::utility::{ByteArray, PortNo, TraceType};
use crate::uuid_ec::Uuid;

#[derive(Debug)]
pub enum TraceFormat {
    EmptyFormat,
    CaNewFormat(CellID, TreeID, TreeID, TreeID),
    CaToCmEntryFormat(RoutingTableEntry),
    CaFromCmBytesMsg(PortNo, bool, Uuid, ByteArray),
    CaFromCmBytesStatus(PortNo, bool, NumberOfPackets, PortStatus),
    CaToNoc(PortNo, ByteArray),
}
impl fmt::Display for TraceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TraceFormat::EmptyFormat => "Empty",
            TraceFormat::CaNewFormat(_, _, _, _) => "CaNew",
            TraceFormat::CaToCmEntryFormat(_) => "CaToCmEntry",
            TraceFormat::CaFromCmBytesMsg(_, _, _, _) => "CaFromCmBytesMsg",
            TraceFormat::CaFromCmBytesStatus(_, _, _, _) => "CaFromCmBytesStatus",
            TraceFormat::CaToNoc(_, _) => "CaToNoc"
        };
        write!(f, "{}", s)
    }
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
                                     ca_new.control_tree_id, ca_new.connected_tree_id)
        },
        "ca_to_cm_entry" => {
            let ca_to_cm_entry: CaToCmEntry = serde_json::from_value(trace.body)?;
            TraceFormat::CaToCmEntryFormat(ca_to_cm_entry.entry)
        },
        "ca_from_cm_bytes" => {
            let m2a: CaFromCmBytesMsg = match serde_json::from_value(trace.body) {
                Ok(m) => m,
                Err(e) => {
                    println!("Replay {} error {}", format, e);
                    return Err(e.into());
                }
            };
            TraceFormat::CaFromCmBytesMsg(m2a.port, m2a.is_ait, m2a.uuid, m2a.bytes)
        }
        "ca_from_cm_status" => {
            let m2a: CaFromCmBytesStatus = match serde_json::from_value(trace.body) {
                Ok(m) => m,
                Err(e) => {
                    println!("Replay {} error {}", format, e);
                    return Err(e.into());
                }
            };
            TraceFormat::CaFromCmBytesStatus(m2a.port, m2a.is_border, m2a.no_packets, m2a.status)
        }
        "ca_to_noc_tree_name" => {
            let a2n: CaToNoc = match serde_json::from_value(trace.body) {
                Ok(m) => m,
                Err(e) => {
                    println!("Replay {} error {}", format, e);
                    return Err(e.into());
                }
            };
            TraceFormat::CaToNoc(a2n.noc_port, a2n.bytes)
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
struct TraceRecordCaFromCmBytesMsg {
    header: TraceHeader,
    body: CaFromCmBytesMsg
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaFromCmBytesMsg {
    is_ait: bool,
    port: PortNo,
    uuid: Uuid,
    bytes: ByteArray
}
impl fmt::Display for CaFromCmBytesMsg {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecordCaFromCmBytesStatus {
    header: TraceHeader,
    body: CaFromCmBytesStatus
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaFromCmBytesStatus {
    port: PortNo,
    is_border: bool,
    no_packets: NumberOfPackets,
    status: PortStatus
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceRecordCaToNoc {
    header: TraceHeader,
    body: CaToNoc
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaToNoc {
    noc_port: PortNo,
    bytes: ByteArray
}
#[derive(Debug, Fail)]
pub enum ReplayError {
    #[fail(display = "ReplayError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
