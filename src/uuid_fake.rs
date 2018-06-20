// A deterministic UUID to make debugging easier
use std::fmt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::Value;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uuid {
    uuid: (u64, u64)
}
impl Uuid {
    pub fn new_v4(value: &str) -> Uuid {
        let mut s = DefaultHasher::new();
        value.hash(&mut s);
        s.finish();
        Uuid { uuid: (s.finish(), 0) }
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:08.x}", self.uuid.0)
    }
}
impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{:08.x}", self.uuid.0) }
}