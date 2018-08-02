// This file contains hacks that represent functions of the DAL.
// which will be replaced by actual distributed storage algorithms.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Duration;

use futures::*;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::message::OwnedHeaders;

use kafka::producer::{Producer, Record, RequiredAcks};
use serde_json;
use serde_json::{Value};

use config::{KAFKA_SERVER, OUTPUT_FILE_NAME, SIMPLE_KAFKA};
use utility::{S, TraceHeader, TraceHeaderParams, TraceType};

const FOR_EVAL: bool = true;

static mut PRODUCER_RD: Option<FutureProducer> = None;

pub fn add_to_trace(producer: &mut Producer, trace_header: &mut TraceHeader, trace_type: TraceType,
                    trace_params: &TraceHeaderParams, trace_body: &Value, caller: &str) -> Result<(), Error> {
    let f = "add_to_trace";
    //let mut buf = String::with_capacity(2);
    let mut file_handle = match OpenOptions::new().append(true).open(OUTPUT_FILE_NAME) {
        Ok(f) => Ok(f),
        Err(_) => {
            println!("Writing output to {}", OUTPUT_FILE_NAME);
            File::create(OUTPUT_FILE_NAME)
        }
    }?;
    trace_header.next(trace_type);
    trace_header.update(trace_params);
    let trace_record = TraceRecord { header: trace_header, body: trace_body };
    let line = if FOR_EVAL {
        serde_json::to_string(&trace_record).context(DalError::Chain { func_name: "add_to_trace", comment: S(caller) })?
    } else {
        format!("{:?}", &trace_record)
    };
    file_handle.write(&(line.clone() + "\n").into_bytes()).context(DalError::Chain { func_name: "add_to_trace", comment: S("Write record") })?;
    if SIMPLE_KAFKA {
        match producer.send(&Record { topic: "CellAgent", partition: 0, key: (), value: line }) {
            Ok(_) => (),
            Err(e) => {
                println!("{}", e);
                return Err(DalError::Kafka { func_name: f, kafka_error: S(e) }.into())
            }
        }
    } else {
        unsafe {
            match PRODUCER_RD.clone() {
                Some(p) => p,
                None => {
                    PRODUCER_RD = Some(ClientConfig::new()
                        .set("bootstrap.servers", KAFKA_SERVER)
                        .set("message.timeout.ms", "5000")
                        .create()
                        .expect("Producer creation error"));
                    PRODUCER_RD.clone().unwrap()
                }
            }.send(
                FutureRecord::to("CellAgent")
                    .payload(&line)
                    .key(&format!("{:?}", trace_header.get_event_id())),
                0);
        }
    }
    Ok(())
}
pub fn make_kafka_producer(_requester: &str) -> Result<Producer, Error> {
    let f = "make_kafka_producer";
    match Producer::from_hosts(vec!(KAFKA_SERVER.to_owned()))
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(RequiredAcks::One)
            .create() {
        Ok(p) => Ok(p),
        Err(e) => {
            println!("Error creating Kafka producer");
            Err(DalError::Kafka { func_name: f, kafka_error: S(e) }.into())
        }
    }
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