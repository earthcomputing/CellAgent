// A deterministic UUID to make debugging easier
use std::fmt;

use utility::PortNumber;
use uuid;

const NORMAL:   u8 = 0b01000000;  // Used for all Name UUIDs, including TreeIDs used for normal packets
const AIT:      u8 = 0b00000100;
const TECK:     u8 = 0b00000011;
const TACK:     u8 = 0b00000010;
const TOCK:     u8 = 0b00000001;
const TICK:     u8 = 0b00000000;
const FORWARD:  u8 = 0b00000000;  // Denotes forward direction in time for AIT transfer
const REVERSE:  u8 = 0b10000000;  // Denotes time reversal for AIT transfer

const AIT_BYTE: usize = 0;
const PORT_NO_BYTE: usize = 1;

type Bytes = [u8; 16];
#[derive(Copy, Clone, Hash, Eq, Serialize, Deserialize)]
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
    fn get_bytes(&self) -> Bytes { *self.uuid.as_bytes() }
    fn set_bytes(&mut self, bytes: Bytes) { self.uuid = uuid::Uuid::from_uuid_bytes(bytes); }
    fn mask_special_bytes(&self) -> Bytes {
        let mut bytes = self.clone().get_bytes();
        bytes[AIT_BYTE] = 0;
        bytes[PORT_NO_BYTE] = 0;
        bytes
    }
    fn get_code(&self) -> u8 {
        *self.uuid.as_bytes().get(AIT_BYTE).unwrap()
    }
    fn set_code(&mut self, code: u8) {
        let mut bytes = self.get_bytes();
        bytes[AIT_BYTE] = code | (bytes[AIT_BYTE] & REVERSE); // Make sure to keep direction when changing code
        self.set_bytes(bytes);
    }
    pub fn is_ait(&self) -> bool {
        self.get_ait_state() == AitState::Ait
    }
    pub fn get_ait_state(&self) -> AitState {
        let f = "get_ait_state";
        match self.get_code() {
            TICK => AitState::Tick,
            TOCK => AitState::Tock,
            TACK => AitState::Tack,
            TECK => AitState::Teck,
            AIT  => AitState::Ait,
            _    => AitState::Normal, // Bad uuid codes are treated as normal
        }
    }
    pub fn get_direction(&self) -> TimeDirection {
        match self.get_code() & REVERSE {
            FORWARD => TimeDirection::Forward,
            REVERSE => TimeDirection::Reverse,
            _ => panic!("0xC0 & code is not 0 or 1")
        }
    }
    fn is_forward(&self) -> bool {
        match self.get_direction() {
            TimeDirection::Forward => true,
            TimeDirection::Reverse => false
        }
    }
    fn is_reverse(&self) -> bool { !self.is_forward() }
    pub fn make_normal(&mut self) -> AitState {
        let mut bytes = self.get_bytes();
        bytes[AIT_BYTE] = NORMAL;
        bytes[PORT_NO_BYTE] = 0; // Used to code root port number in TreeID
        self.set_bytes(bytes);
        AitState::Normal
    }
    pub fn make_ait(&mut self) -> AitState {
        self.set_code(AIT);
        AitState::Ait
    }
    pub fn add_port_no(&mut self, port_number: &PortNumber) {
        let port_no = port_number.get_port_no();
        let mut bytes = self.get_bytes();
        bytes[PORT_NO_BYTE] = *port_no;
        self.set_bytes(bytes);
    }
    pub fn remove_port_no(&mut self) {
        let mut bytes = self.get_bytes();
        bytes[PORT_NO_BYTE] = 0;
        self.set_bytes(bytes);
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
            AIT  => { self.set_code(TECK); AitState::Teck },
            NORMAL => AitState::Normal,
            _ => return Err(UuidError::AitState { func_name: f, ait_state: self.get_ait_state() }.into())
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
            _ => return Err(UuidError::AitState { func_name: f, ait_state: self.get_ait_state() }.into())
        })
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_ait() { write!(f, "{} in time {}", self.get_direction(), self.uuid ) }
        else             { write!(f, "{}", self.uuid) }
    }
}
impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.get_ait_state(), self.uuid)
    }
}
impl PartialEq for Uuid {
    fn eq(&self, other: &Uuid) -> bool {
        self.uuid == other.uuid //self.mask_special_bytes() == other.mask_special_bytes()
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
use failure::{Error,};
#[derive(Debug, Fail)]
pub enum UuidError {
    #[fail(display = "UuidError::Chain {}: {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "UuidError::AitState: Can't do {} from state {}", func_name, ait_state)]
    AitState { func_name: &'static str, ait_state: AitState },
    #[fail(display = "UuidError::Code {}: {} is an invalid UUID code", func_name, code)]
    Code { func_name: &'static str, code: u8 }
}
