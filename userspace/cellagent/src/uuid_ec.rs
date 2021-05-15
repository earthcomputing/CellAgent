/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
// A deterministic UUID to make debugging easier
use std::fmt;

use uuid;

use crate::utility::{PortNo, PortNumber};
// First 4 bits for flags, such as Snake and time direction
const NORMAL:   u8 = 0b0000_0000;  // Used for all Name UUIDs, including TreeIDs and for normal packets
const TICK:     u8 = 0b0000_0001;
const TOCK:     u8 = 0b0000_0010;
const TECK:     u8 = 0b0000_0011;
const TACK:     u8 = 0b0000_0100;
const TUCK:     u8 = 0b0000_0101;
const TYCK:     u8 = 0b0000_0110;
const INIT:     u8 = 0b0000_1111;  // Used to break symmetry when a link starts
const SNAKED:   u8 = 0b0000_1110;  // Snake ack
const AITD:     u8 = 0b0000_1011;  // AIT packet delivered or not (ACK/NACK depending on time reversal)
const AIT:      u8 = 0b0000_1001;  // Sent AIT packet
const FORWARD:  u8 = 0b0000_0000;  // Denotes forward direction in time for AIT transfer
const REVERSE:  u8 = 0b1000_0000;  // Denotes time reversal for AIT transfer
const SNAKE:    u8 = 0b0100_0000;  // Packets that won't get lost on node failure
const CTRL:     u8 = 0b0010_0000;  // Control packets

const AIT_BYTE: usize = 0;
const PORT_NO_BYTE: usize = 1;

type Bytes = [u8; 16];
#[repr(C)]
#[derive(Copy, Clone, Default, Hash, Eq, PartialOrd, Serialize, Deserialize)]
pub struct Uuid {
    uuid: uuid::Uuid
}
impl Uuid {
    pub fn new() -> Uuid {
        let mut uuid = Uuid { uuid: uuid::Uuid::new_v4() };
        uuid.make_normal();
        uuid
    }
    pub fn _new_ait() -> Uuid {
        let mut uuid = Uuid { uuid: uuid::Uuid::new_v4() };
        uuid.make_ait();
        uuid
    }
    fn get_bytes(&self) -> Bytes { *self.uuid.as_bytes() }
    fn set_bytes(&mut self, bytes: Bytes) { self.uuid = uuid::Uuid::from_bytes(bytes); }
    fn mask_ait_byte(&self) -> Bytes {
        let mut bytes = self.clone().get_bytes();
        bytes[AIT_BYTE] = 0;
        bytes
    }
    fn get_code(&self) -> u8 {
        self.uuid.as_bytes()[AIT_BYTE] & 0b0000_1111
    }
    fn set_code(&mut self, code: u8) -> AitState {
        let mut bytes = self.get_bytes();
        bytes[AIT_BYTE] = (bytes[AIT_BYTE] & 0b1111_0000) ^ code; // Keep flags when setting code
        self.set_bytes(bytes);
        self.get_ait_state()
    }
    fn get_flags(&self) -> u8 {
        self.uuid.as_bytes()[AIT_BYTE] & 0b1111_0000
    }
    fn clear_flags(&mut self) {
        let mut bytes = self.get_bytes();
        bytes[AIT_BYTE] = bytes[AIT_BYTE] & 0b0000_1111;
        self.set_bytes(bytes);
    }
    pub fn is_entl(&self) -> bool {
        let ait_state = self.get_ait_state();
        (ait_state == AitState::Tick) ||
        (ait_state == AitState::Tock)
    }
    pub fn is_ait(&self) -> bool {
        self.is_ait_send() || self.is_ait_recv()
    }
    pub fn is_ait_send(&self) -> bool {
        self.get_ait_state() == AitState::Ait
    }
    pub fn is_ait_recv(&self) -> bool {
        self.get_ait_state() == AitState::Ait
    }
    pub fn is_init(&self) -> bool {
        self.get_ait_state() == AitState::Init
    }
    pub fn is_snake(&self) -> bool {
        (self.get_flags() & SNAKE) != 0
    }
    pub fn is_snaked(&self) -> bool {
        self.get_ait_state() == AitState::SnakeD
    }
    pub fn is_control(&self) -> bool {
        (self.get_flags() & CTRL) != 0
    }
    pub fn get_ait_state(&self) -> AitState {
        let _f = "get_ait_state"; 
        match self.get_code() & 0b0000_1111 {
            TICK => AitState::Tick,
            TOCK => AitState::Tock,
            TYCK => AitState::Tyck,
            TUCK => AitState::Tuck,
            TACK => AitState::Tack,
            TECK => AitState::Teck,
            AITD => AitState::AitD,
            AIT =>  AitState::Ait,
            INIT => AitState::Init,
            SNAKED => AitState::SnakeD,
            _    => AitState::Normal, // Bad uuid codes are treated as normal
        }
    }
    pub fn get_direction(&self) -> TimeDirection {
        match self.get_code() & REVERSE {
            FORWARD => TimeDirection::Forward,
            REVERSE => TimeDirection::Reverse,
            _ => panic!("REVERSE & code is not 0 or 1")
        }
    }
    fn _is_forward(&self) -> bool {
        match self.get_direction() {
            TimeDirection::Forward => true,
            TimeDirection::Reverse => false
        }
    }
    fn _is_reverse(&self) -> bool { !self._is_forward() }
    pub fn for_lookup(&self) -> Uuid {
        let bytes = self.mask_ait_byte();
        Uuid { uuid: uuid::Uuid::from_bytes(bytes) }
    }
    pub fn make_normal(&mut self) -> AitState {
        let mut bytes = self.get_bytes();
        bytes[AIT_BYTE] = NORMAL;
        bytes[PORT_NO_BYTE] = 0; // Used to code root port number in TreeID
        self.set_bytes(bytes);
        AitState::Normal
    }
    pub fn make_init(&mut self) -> AitState {
        self.set_code(INIT);
        AitState::Init
    }
    pub fn make_ait(&mut self) -> AitState {
        self.set_code(AIT);
        AitState::Ait
    }
    pub fn make_snake(&mut self) -> AitState {
        let code = self.get_code();
        let new_code = code ^ SNAKE;
        self.set_code(new_code);
        self.get_ait_state()
    }
    pub fn make_snaked(&mut self) -> AitState {
        self.set_code(SNAKED);
        AitState::SnakeD
    }
    pub fn make_control(&mut self) -> AitState {
        let code = self.get_code();
        let new_code = code ^ CTRL;
        self.set_code(new_code);
        self.get_ait_state()
    }
    // Tell sender if transfer succeeded or not
    pub fn make_aitd(&mut self) -> AitState {
        self.set_code(AITD);
        AitState::AitD
    }
    pub fn make_tick(&mut self) -> AitState {
        self.clear_flags();
        self.set_code(TICK);
        AitState::Tick
    }
    pub fn make_tock(&mut self) -> AitState {
        self.clear_flags();
        self.set_code(TOCK);
        AitState::Tock
    }
    pub fn set_port_number(&mut self, port_number: PortNumber) {
        let port_no = port_number.get_port_no();
        self.set_port_no(port_no);
    }
    pub fn set_port_no(&mut self, port_no: PortNo) {
        let mut bytes = self.get_bytes();
        bytes[PORT_NO_BYTE] = *port_no;
        self.set_bytes(bytes);
    }
    pub fn get_port_no(&self) -> PortNo {
        let bytes = self.get_bytes();
        PortNo(bytes[PORT_NO_BYTE])
    }
    pub fn _has_port_no(&self) -> bool {
        let bytes = self.get_bytes();
        bytes[PORT_NO_BYTE] != 0
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
        let _f = "next_state";
        Ok(match self.get_code() & !REVERSE {
            TICK => { self.set_code(TOCK) },
            TOCK => { self.set_code(TICK) },
            TYCK => { self.set_code(AITD) },
            TUCK => { self.set_code(TYCK) },
            TACK => { self.set_code(TUCK) },
            TECK => { self.set_code(TACK) },
            AIT  => { self.set_code(TECK) },
            NORMAL => AitState::Normal,
            _ => return {
                Err(UuidError::AitState { func_name: _f, ait_state: self.get_ait_state() }.into())
            }
        })
    }
    fn previous_state(&mut self) -> Result<AitState, Error> {
        let _f = "previous_stat";
        Ok(match self.get_code() & !REVERSE {
            TYCK => { self.set_code(TUCK) },
            TUCK => { self.set_code(TACK) },
            TACK => { self.set_code(TECK) },
            TECK => { self.set_code(AIT)  },
            NORMAL => AitState::Normal,
            _ => return Err(UuidError::AitState { func_name: _f, ait_state: self.get_ait_state() }.into())
        })
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snake = if self.is_snake() { "Snake "} else { "" };
        if self.is_ait_send() { write!(f, "{}{} in time {}", snake, self.get_direction(), self.uuid ) }
        else             { write!(f, "{}", self.uuid) }
    }
}
impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    Normal, Init, SnakeD, AitD,
    Ait, Teck, Tack, Tuck, Tyck, Tock, Tick
}
impl fmt::Display for AitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            AitState::Normal => "Normal",
            AitState::Init   => "Entl",
            AitState::SnakeD => "SnakeD",
            AitState::Ait    => "Ait",
            AitState::AitD   => "AitD",
            AitState::Teck   => "TECK",
            AitState::Tack   => "TACK",
            AitState::Tuck   => "TUCK",
            AitState::Tyck   => "TYCK",
            AitState::Tock   => "TOCK",
            AitState::Tick   => "TICK",
        };
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeDirection {
    Forward, Reverse
}
impl fmt::Display for TimeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TimeDirection::Forward => "Forward",
            TimeDirection::Reverse => "Reverse"
        };
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error,};
#[derive(Debug, Fail)]
pub enum UuidError {
//    #[fail(display = "UuidError::Chain {}: {}", func_name, comment)]
//    Chain { func_name: &'static str, comment: String },
    #[fail(display = "UuidError::AitState: Can't do {} from state {}", func_name, ait_state)]
    AitState { func_name: &'static str, ait_state: AitState },
//    #[fail(display = "UuidError::Code {}: {} is an invalid UUID code", func_name, code)]
//    Code { func_name: &'static str, code: u8 }
}
