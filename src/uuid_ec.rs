// A deterministic UUID to make debugging easier
use std::fmt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use uuid;

/*
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
*/

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uuid {
    codes: u8,          // In case we want to encode stuff in the UUID
    uuid: uuid::Uuid
}
impl Uuid {
    pub fn new_v4(_value: &str) -> Uuid {
        Uuid { codes: 0, uuid: uuid::Uuid::new_v4() }
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.uuid) }
}
