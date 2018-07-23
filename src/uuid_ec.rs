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
    pub fn get_state(&self) -> PacketState {
        match self.get_code() & !REVERSE {  // !REVERSE strips direction from test
            TICK   => PacketState::Tick,
            TOCK   => PacketState::Tock,
            TACK   => PacketState::Tack,
            TECK   => PacketState::Teck,
            AIT    => PacketState::Ait,
            NORMAL => PacketState::Normal,
            _ => panic!("Uuid {} is in an invalid state", self)
        }
    }
    pub fn get_direction(&self) -> TimeDirection {
        match self.get_code() & REVERSE {
            FORWARD => TimeDirection::Forward,
            REVERSE => TimeDirection::Reverse,
            _ => panic!("0xC0 & code is not 0 or 1")
        }
    }
    pub fn make_normal(&mut self) -> PacketState {
        self.set_code(NORMAL);
        PacketState::Normal
    }
    pub fn make_ait(&mut self) -> PacketState {
        self.set_code(AIT);
        PacketState::Ait
    }
    pub fn time_reverse(&mut self) {
        let code = self.get_code();
        self.set_code(code ^ REVERSE);
    }
/*
    pub fn next(&mut self) -> Result<PacketState, Error> {
        match self.get_direction() {
            TimeDirection::Forward => self.next_state(),
            TimeDirection::Reverse => self.previous_state()
        }
    }
    fn next_state(&mut self) -> Result<PacketState, Error> {
        let f = "next_state";
        Ok(match self.code & 0x7F {
            0 => { self.code = 1; PacketState::Tock },
            1 => { self.code = 0; PacketState::Tick },
            2 => { self.code = 1; PacketState::Tack },
            3 => { self.code = 2; PacketState::Teck },
            4 => { self.code = 3; PacketState::Ait  },
            _ => return Err(UuidError::Code { func_name: f, code: self.get_state() }.into())
        })
    }
    fn previous_state(&mut self) -> Result<PacketState, Error> {
        let f = "previous_stat";
        Ok(match self.code & 0x7f {
            0 => { self.code = 1; PacketState::Tock },
            1 => { self.code = 2; PacketState::Tick },
            2 => { self.code = 3; PacketState::Tack },
            3 => { self.code = 4; PacketState::Teck },
            _ => return Err(UuidError::Code { func_name: f, code: self.get_state() }.into())
        })
    }
*/
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} in time {} {}", self.get_direction(), self.get_state(), self.uuid )
    }
}
impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.get_state(), self.uuid)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PacketState {
    Normal, Ait, Teck, Tack, Tock, Tick
}
impl fmt::Display for PacketState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &PacketState::Normal => "Normal",
            &PacketState::Ait    => "AIT",
            &PacketState::Teck   => "TECK",
            &PacketState::Tack   => "TACK",
            &PacketState::Tock   => "TOCK",
            &PacketState::Tick   => "TICK",
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
    Code { func_name: &'static str, code: PacketState }
}
