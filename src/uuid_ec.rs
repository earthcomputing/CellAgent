// A deterministic UUID to make debugging easier
use std::fmt;

use uuid;

const NORMAL:   u8 = 0b01000000;  // Used for all Name UUIDs, including TreeIDs used for normal packets
const AIT:      u8 = 0b00000100;
const TECK:     u8 = 0b00000011;
const TACK:     u8 = 0b00000010;
const TOCK:     u8 = 0b00000001;
const TICK:     u8 = 0b00000000;
const FORWARD:  u8 = 0b00000000;  // Denotes forward direction in time for AIT transfer
const REVERSE:  u8 = 0b10000000;  // Denotes time reversal for AIT transfer
#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uuid {
    uuid: uuid::Uuid
}
impl Uuid {
    pub fn new() -> Uuid {
        Uuid { uuid: uuid::Uuid::new_v4() }
    }
    pub fn new_tree_uuid() -> Uuid {
        let mut uuid = Uuid { uuid: uuid::Uuid::new_v4() };
        uuid.make_normal();
        uuid
    }
    pub fn new_ait() -> Uuid {
        let mut uuid = Uuid { uuid: uuid::Uuid::new_v4() };
        uuid.make_ait();
        uuid
    }
    fn get_code(&self) -> u8 {
        *self.uuid.as_bytes().get(0).unwrap()
    }
    fn set_code(&mut self, code: u8) {
        let mut bytes = *self.uuid.as_bytes();
        bytes[0] = code | (bytes[0] & REVERSE); // Make sure to keep direction when changing code
        self.uuid = uuid::Uuid::from_uuid_bytes(bytes);
    }
    pub fn get_ait_state(&self) -> AitState {
        match self.get_code() & !REVERSE {  // !REVERSE strips direction from test
            TICK   => AitState::Tick,
            TOCK   => AitState::Tock,
            TACK   => AitState::Tack,
            TECK   => AitState::Teck,
            AIT    => AitState::Ait,
            NORMAL => AitState::Normal,
            _ => {
                panic!("Uuid {} is in an invalid state", self)
            }
        }
    }
    pub fn get_direction(&self) -> TimeDirection {
        match self.get_code() & REVERSE {
            FORWARD => TimeDirection::Forward,
            REVERSE => TimeDirection::Reverse,
            _ => panic!("0xC0 & code is not 0 or 1")
        }
    }
    pub fn make_normal(&mut self) -> AitState {
        let mut bytes = *self.uuid.as_bytes();
        bytes[0] = NORMAL;
        self.uuid = uuid::Uuid::from_uuid_bytes(bytes);
        AitState::Normal
    }
    pub fn make_ait(&mut self) -> AitState {
        self.set_code(AIT);
        AitState::Ait
    }
    pub fn time_reverse(&mut self) {
        let code = self.get_code();
        self.set_code(code ^ REVERSE);
    }
    pub fn next(&mut self) -> Result<AitState, Error> {
        match self.get_direction() {
            TimeDirection::Forward => self.next_state(),
            TimeDirection::Reverse => self.previous_state()
        }
    }
    fn next_state(&mut self) -> Result<AitState, Error> {
        let f = "next_state";
        Ok(match self.get_code() & !REVERSE {
            TICK => { self.set_code(TOCK); AitState::Tock },
            TOCK => { self.set_code(TICK); AitState::Tick },
            TACK => { self.set_code(TOCK); AitState::Tock },
            TECK => { self.set_code(TACK); AitState::Tack },
            AIT  => { self.set_code(TACK); AitState::Ait  },
            NORMAL => AitState::Normal,
            _ => return Err(UuidError::Code { func_name: f, code: self.get_ait_state() }.into())
        })
    }
    fn previous_state(&mut self) -> Result<AitState, Error> {
        let f = "previous_stat";
        Ok(match self.get_code() & !REVERSE {
            TICK => { self.set_code(TOCK); AitState::Tock },
            TOCK => { self.set_code(TACK); AitState::Tack },
            TACK => { self.set_code(TECK); AitState::Teck },
            TECK => { self.set_code(AIT);  AitState::Teck },
            NORMAL => AitState::Normal,
            _ => return Err(UuidError::Code { func_name: f, code: self.get_ait_state() }.into())
        })
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} in time {}", self.get_direction(), self.uuid )
    }
}
impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.get_ait_state(), self.uuid)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AitState {
    Normal, Ait, Teck, Tack, Tock, Tick
}
impl fmt::Display for AitState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &AitState::Normal => "Normal",
            &AitState::Ait    => "AIT",
            &AitState::Teck   => "TECK",
            &AitState::Tack   => "TACK",
            &AitState::Tock   => "TOCK",
            &AitState::Tick   => "TICK",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeDirection {
    Forward, Reverse
}
impl fmt::Display for TimeDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &TimeDirection::Forward => "Forward",
            &TimeDirection::Reverse => "Reverse"
        };
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, Fail, ResultExt};
#[derive(Debug, Fail)]
pub enum UuidError {
    #[fail(display = "UuidError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "UuidError::Code: Can't do {} from state {}", func_name, code)]
    Code { func_name: &'static str, code: AitState }
}
