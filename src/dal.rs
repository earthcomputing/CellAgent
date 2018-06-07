// This file contains hacks that represent functions of the DAL.
// which will be replaced by actual distributed storage algorithms.

use std::fs::{File, OpenOptions};
use std::io::Write;

use serde_json;
use serde_json::Value;

use config::{OUTPUT_FILE_NAME};
use utility::S;

pub fn add_to_trace(obj: &Value, comment: &str) -> Result<(), Error> {
    let mut file_handle = match OpenOptions::new().append(true).open(OUTPUT_FILE_NAME) {
        Ok(f) => Ok(f),
        Err(_) => {
            println!("Writing output to {}", OUTPUT_FILE_NAME);
            File::create(OUTPUT_FILE_NAME)
        }
    }?;
    let line = serde_json::to_string(obj).context(DalError::Chain { func_name: "add_to_trace", comment: S(comment) })?;
    file_handle.write(&(line + "\n").into_bytes()).context(DalError::Chain { func_name: "add_to_trace", comment: S("Write") })?;
    Ok(())
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DalError {
    #[fail(display = "DalError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}