// This file contains hacks that represent functions of the DAL.
// which will be replaced by actual distributed storage algorithms.

use std::fs::{File, OpenOptions};
use std::io::Write;

use serde_json;
use serde_json::{Value};

use config::{OUTPUT_FILE_NAME};
use utility::{S, TraceHeader, TraceHeaderParams, TraceType};

const SCHEMA_VERSION: &'static str = "0.1";
const FOR_EVAL: bool = true;
pub fn add_to_trace(trace_header: &mut TraceHeader, trace_type: TraceType,
                    trace_params: &TraceHeaderParams, trace_body: &Value, caller: &str) -> Result<(), Error> {
    let mut file_handle = match OpenOptions::new().append(true).open(OUTPUT_FILE_NAME) {
        Ok(f) => Ok(f),
        Err(_) => {
            println!("Writing output to {}", OUTPUT_FILE_NAME);
            File::create(OUTPUT_FILE_NAME)
        }
    }?;
    let version = (S(json!({ "schema_version": SCHEMA_VERSION})) + "\n").into_bytes();
    file_handle.write(&version).context(DalError::Chain { func_name: "add_to_trace", comment: S("Write version") })?;;
    trace_header.next(trace_type);
    trace_header.update(trace_params);
    let trace_record = TraceRecord { header: trace_header, body: trace_body };
    let line = if FOR_EVAL {
        serde_json::to_string(&trace_record).context(DalError::Chain { func_name: "add_to_trace", comment: S(caller) })?
    } else {
        format!("{:?}", &trace_record)
    };
    file_handle.write(&(line + "\n").into_bytes()).context(DalError::Chain { func_name: "add_to_trace", comment: S("Write record") })?;
    Ok(())
}
#[derive(Debug, Clone, Serialize)]
struct TraceRecord<'a> {
    header: &'a TraceHeader,
    body: &'a Value
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DalError {
    #[fail(display = "DalError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}