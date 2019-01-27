use std::{fmt};
use std::string::String;

use serde::{Deserialize, Deserializer, Serializer};

use crate::utility::S;

// Names are limited to 32 characters because that's the largest Rust has implemented for copying arrays
// If you want longer names, you must implement Debug, Copy, and Clone for each ID struct
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NameType {
    #[serde(serialize_with = "name_string")]
    #[serde(deserialize_with = "string_name")]
    name: [char; 32]
}
fn name_string<S: Serializer>(name: &[char; 32], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&str_from_chars(NameType { name: *name }))
}
fn string_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[char; 32], D::Error>
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    Ok(str_to_chars(s).name)
}
impl fmt::Display for NameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { str_from_chars(*self).fmt(f) }
}
pub fn str_to_chars(string: &str) -> NameType {
    let _f = "str_to_chars";
    if string.len() > 32 {
        // TODO: Handle error properly
        panic!(format!("String |{}| is longer than 32 characters {}", string, string.len()))
    }
    let padded = format!("{:\n<32}", string);
    let mut char_slice = ['\n'; 32];
    for (i, c) in padded.char_indices() {
        char_slice[i] = c; }
    NameType { name: char_slice }
}
pub fn str_from_chars(chars: NameType) -> String {
    let _f = "str_from_chars";
    let mut output = S("");
    for c in &chars.name {
        if c == &'\n' { break; }
        output.push(*c);
    }
    output
}
