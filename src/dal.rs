// This file contains hacks that represent functions of the DAL.
// which will be replaced by actual distributed storage algorithms.
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::Write;

use lazy_static::lazy_static;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json;
use serde_json::{Value};
use futures::Future;

use crate::config::{KAFKA_SERVER, KAFKA_TOPIC, OUTPUT_FILE_NAME};
use crate::utility::{S, TraceHeader, TraceHeaderParams, TraceType};

thread_local!(static TRACE_HEADER: RefCell<TraceHeader> = RefCell::new(TraceHeader::new()));
pub fn fork_trace_header() -> TraceHeader { TRACE_HEADER.with(|t| t.borrow_mut().fork_trace()) }
pub fn update_trace_header(child_trace_header: TraceHeader) { TRACE_HEADER.with(|t| *t.borrow_mut() = child_trace_header); }

const FOR_EVAL: bool = true;

lazy_static! {
    static ref PRODUCER_RD: FutureProducer = ClientConfig::new()
                        .set("bootstrap.servers", KAFKA_SERVER)
                        .set("message.timeout.ms", "5000")
                        .create()
                        .expect("Dal: Problem setting up Kafka");
}

pub fn add_to_trace(trace_type: TraceType, trace_params: &TraceHeaderParams,
                    trace_body: &Value, caller: &str) -> Result<(), Error> {
    let _f = "add_to_trace";
    //let mut buf = String::with_capacity(2);
    let mut file_handle = OpenOptions::new().append(true).open(OUTPUT_FILE_NAME)
        .or_else(|_| {
            println!("Writing output to {}", OUTPUT_FILE_NAME);
            File::create(OUTPUT_FILE_NAME)
        })?;
    TRACE_HEADER.with(|t| {
        t.borrow_mut().next(trace_type);
        t.borrow_mut().update(trace_params);
    });
    let trace_header = TRACE_HEADER.with(|t| t.borrow().clone());
    let trace_record = TraceRecord { header: &trace_header, body: trace_body };
    let line = if FOR_EVAL {
        serde_json::to_string(&trace_record).context(DalError::Chain { func_name: "add_to_trace", comment: S(caller) })?
    } else {
        format!("{:?}", &trace_record)
    };
    file_handle.write(&(line.clone() + "\n").into_bytes()).context(DalError::Chain { func_name: "add_to_trace", comment: S("Write record") })?;
    let _ = PRODUCER_RD.send(FutureRecord::to(KAFKA_TOPIC)
            .payload(&line)
            .key(&format!("{:?}", trace_header.get_event_id())),
        0)
        .then(|result| -> Result<(), Error> {
            match result {
                Ok(Ok(_)) => Ok(()),
                Ok(Err((e, _))) => Err(DalError::Kafka { func_name: _f, kafka_error: S(e) }.into()),
                Err(e) => Err(DalError::Kafka { func_name: _f, kafka_error: S(e) }.into())
            }
        });
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
    #[fail(display = "DalError::Kafka {}: Error {} producing trace record", func_name, kafka_error)]
    Kafka { func_name: &'static str, kafka_error: String }
}