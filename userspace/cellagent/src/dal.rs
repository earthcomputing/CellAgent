// This file contains hacks that represent functions of the DAL.
// which will be replaced by actual distributed storage algorithms.
use std::{cell::RefCell,
          collections::{HashSet},
          fs::{File, OpenOptions},
          io::{BufReader, Lines, Write},
          sync::atomic::{AtomicBool}
};
#[cfg(feature="webserver")]
use {
    actix_rt::{System, SystemRunner},
    actix_web::client::{ClientBuilder},
    futures::{Future, future::lazy},
    std::sync::atomic::Ordering
};

//use futures::Future; // Needed for rdkafka
use lazy_static::lazy_static;
//use rdkafka::{config::ClientConfig, producer::{FutureProducer, FutureRecord}};
use serde_json;
use serde_json::{Value};

use crate::config::{CONFIG};
use crate::utility::{S, TraceHeader, TraceHeaderParams, TraceType, write_err};

const FOR_EVAL: bool = true;

// TODO: Integrate with log crate

#[cfg(feature="webserver")]
lazy_static! {
    static ref SERVER_URL: String = ::std::env::var("SERVER_URL").expect("Environment variable SERVER_URL not set");
}

lazy_static! {
    static ref SERVER_ERROR: AtomicBool = AtomicBool::new(false);
/*
    static ref PRODUCER_RD: FutureProducer = ClientConfig::new()
                        .set("bootstrap.servers", &CONFIG.kafka_server)
                        .set("message.timeout.ms", "5000")
                        .create()
                        .expect("Dal: Problem setting up Kafka");
*/
}

#[cfg(feature="webserver")]
thread_local!{ static SYSTEM: RefCell<SystemRunner> = RefCell::new(System::new("Tracer")); }
thread_local!{ static SKIP: RefCell<HashSet<String>> = RefCell::new(HashSet::new()); }
thread_local!{ static TRACE_HEADER: RefCell<TraceHeader> = RefCell::new(TraceHeader::new()) }

pub fn fork_trace_header() -> TraceHeader { TRACE_HEADER.with(|t| t.borrow_mut().fork_trace()) }
pub fn update_trace_header(child_trace_header: TraceHeader) { TRACE_HEADER.with(|t| *t.borrow_mut() = child_trace_header); }

pub fn add_to_trace(trace_type: TraceType, trace_params: &TraceHeaderParams,
                    trace_body: &Value, caller: &str) {
    let _f = "add_to_trace";
    let other = json!({"name": "Other"});
    let cell_id = trace_body
        .get("cell_id")
        .unwrap_or(&other)
        .get("name")
        .unwrap()
        .as_str()
        .unwrap();
    // Mac Finder replaces ":" with "/" which is obviously bad for filenames in the shell
    let cell_file_name = format!("{}{}-{}.json", CONFIG.output_dir_name, CONFIG.output_file_name, str::replace(cell_id, ":", "-"));
    let cell_file_handle = OpenOptions::new().append(true).open(cell_file_name.clone())
        .or_else(|_| { File::create(cell_file_name) }).map_err(|e| write_err(&format!("Dal: {}", caller), &e.into()));
    let output_file_name = format!("{}/{}.json", CONFIG.output_dir_name, CONFIG.output_file_name);
    let file_handle = OpenOptions::new().append(true).open(output_file_name.clone())
        .or_else(|_| { File::create(output_file_name) }).map_err(|e| write_err(&format!("Dal: {}", caller), &e.into()));
    TRACE_HEADER.with(|t| {
        t.borrow_mut().next(trace_type);
        t.borrow_mut().update(trace_params);
    });
    let trace_header = TRACE_HEADER.with(|t| t.borrow().clone());
    let trace_record = TraceRecord { header: &trace_header, body: trace_body };
    #[cfg(feature="webserver")]
    let _ = trace_it(&trace_record).map_err(|e| write_err(&format!("Dal: {}", caller), &e));
    let line = if FOR_EVAL {
        serde_json::to_string(&trace_record).map_err(|e| write_err(&format!("Dal: {}", caller), &e.into()))
    } else {
        Ok(format!("{:?}", &trace_record))
    };
    if let Ok(line) = line {
        if let Ok(mut c) = cell_file_handle {
            let _ = c.write_all(&(line.clone() + ",\n").into_bytes()).map_err(|e| write_err(&format!("Dal: {}", caller), &e.into()));
        };
        if let Ok(mut f) = file_handle {
            let _ = f.write_all(&(line.clone() + ",\n").into_bytes()).map_err(|e| write_err(&format!("Dal: {}", caller), &e.into()));
        }
    };
/*
    let _ = PRODUCER_RD.send(FutureRecord::to(&CONFIG.kafka_topic)
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
*/
}
pub fn get_cell_replay_lines(cell_name: &str) -> Result<Lines<BufReader<File>>, Error> {
    let _f = "get_cell_replay_lines";
    let dir_name = format!("{}-replay", &CONFIG.output_dir_name[..CONFIG.output_dir_name.len()-1]);
    let file_name = if CONFIG.replay {
        format!("{}/{}-{}.json", dir_name, CONFIG.output_file_name, cell_name)
    } else {
        S("/dev/null")
    };
    let cell_file_name = str::replace(&file_name, ":", "-");
    let cell_file_handle = OpenOptions::new().read(true).open(cell_file_name.clone()).context(DalError::Replay { func_name: _f, file_name: cell_file_name, cell_name: S(cell_name) })?;
    let reader = BufReader::new(cell_file_handle);
    Ok(reader.lines())
}
#[cfg(feature="webserver")]
fn trace_it(trace_record: &TraceRecord<'_>) -> Result<(), Error> {
    let _f = "trace_it";
    if SERVER_ERROR.load(Ordering::SeqCst) { return Ok(()); }
    let header = trace_record.header;
    let format = header.format();
    let server_url = format!("{}/{}", *SERVER_URL, format);
    let server_url_clone = server_url.clone(); // So I can print as part of en error response
    let value = serde_json::to_value(trace_record)?;
    SKIP.with(|skip| {
        let mut s = skip.borrow_mut();
        if !s.contains(format) {
            let client_builder = ClientBuilder::new();
            let client = client_builder.disable_timeout().finish();
            let _ = SYSTEM.with(|sys| {
                let mut system = sys.borrow_mut();
                system.block_on(lazy(|| {
                    client.post(server_url)
                        .header("User-Agent", "Actix-web")
                        .send_json(&value)
                        .map_err(|e| {
                            println!("\nError from server: url {} {:?}\n", server_url_clone, e);
                            SERVER_ERROR.swap(true, Ordering::SeqCst);
                        })
                        .and_then(|response| {
                            if !response.status().is_success() {
                                if response.status() != 404 {
                                    println!("Error {}: {:?}", server_url_clone, response);
                                }
                                s.insert(format.to_owned());
                            }
                            Ok(())
                        })
                }))
            });
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
use std::io::BufRead;

#[derive(Debug, Fail)]
pub enum DalError {
    #[fail(display = "DalError::Chain {}: {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "DalError::Kafka {}: Error {} producing trace record", func_name, kafka_error)]
    Kafka { func_name: &'static str, kafka_error: String },
    #[fail(display = "DalError::Replay {}: Error opening replay file {} on cell {}", func_name, file_name, cell_name)]
    Replay { func_name: &'static str, file_name: String, cell_name: String}
}