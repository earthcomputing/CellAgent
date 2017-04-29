use std;
use std::fmt;
use std::mem;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use rand;
use std::str;
use std::str::Utf8Error;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde;
use serde_json;
use config::{PACKET_MIN, PACKET_MAX, PAYLOAD_DEFAULT_ELEMENT, PacketElement, PacketNo, TableIndex, Uniquifier};
use message::{Message, DiscoverMsg, DiscoverDMsg, MsgDirection};

const LARGEST_MSG: usize = std::u32::MAX as usize;
const PAYLOAD_MIN: usize = PACKET_MAX - PACKET_HEADER_SIZE;
const PAYLOAD_MAX: usize = PACKET_MAX - PACKET_HEADER_SIZE;

static packet_count: AtomicUsize = ATOMIC_USIZE_INIT;
pub fn get_next_count() -> usize { packet_count.fetch_add(1, Ordering::SeqCst) } 
#[derive(Copy)]
pub struct Packet {
	header: PacketHeader, 
	payload: Payload,
}
impl Packet {
	fn new(header: PacketHeader, payload: Payload) -> Packet {
		Packet { header: header, payload: payload }
	}
	pub fn get_header(&self) -> PacketHeader { self.header }
	pub fn get_payload(&self) -> Payload { self.payload }
	pub fn get_payload_bytes(&self) -> Vec<u8> { self.get_payload().get_bytes() }
	pub fn get_payload_size(&self) -> usize { self.payload.get_no_bytes() }
}
impl fmt::Display for Packet {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Header: {}, Payload: {}", self.header, self.payload);
		write!(f, "{}", s)
	} 	
}
impl Clone for Packet {
	fn clone(&self) -> Packet { *self }
}
impl fmt::Debug for Packet { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt(f) }
}
#[derive(Copy)]
struct Payload {
	no_data_bytes: u16,
	bytes: [PacketElement; PAYLOAD_MAX],
}
impl Payload {
	pub fn new(data_bytes: Vec<PacketElement>) -> Payload {
		let no_data_bytes = data_bytes.len();
		let mut bytes = [0; PAYLOAD_MAX];
		for i in 0..no_data_bytes { bytes[i] = data_bytes[i]; }
		Payload { no_data_bytes : no_data_bytes as u16, bytes: bytes }
	}
	fn get_bytes(&self) -> Vec<PacketElement> { self.bytes.iter().cloned().collect() }
	fn get_no_bytes(&self) -> usize { self.get_bytes().len() }
	fn get_msg_bytes(&self) -> Vec<PacketElement> {
		let total_bytes = self.get_bytes();
		total_bytes[0..self.no_data_bytes as usize].iter().cloned().collect()
	}
}
impl fmt::Display for Payload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = &format!("{:?}", &self.bytes[0..10]); 
		write!(f, "{}", s)
	} 	
}
impl Clone for Payload {
	fn clone(&self) -> Payload { *self }
}
impl fmt::Debug for Payload { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt(f) }
}
pub struct Packetizer {}
impl Packetizer {
	pub fn packetize<M>(msg: &M, flags: [bool;4]) -> Result<Vec<Box<Packet>>, PacketizerError>
			where M: Message + Hash + serde::Serialize {
		let serialized = try!(serde_json::to_string(&msg));
		let msg_bytes = serialized.clone().into_bytes();
		let payload_size = Packetizer::packet_payload_size(msg_bytes.len());
		let num_packets = (msg_bytes.len() + payload_size - 1)/ payload_size; // Poor man's ceiling
		let last_packet_size = msg_bytes.len() - (num_packets-1)*payload_size;
		let unique_id = rand::random(); // Can't use hash in case two cells send the same message
		let direction = msg.get_header().get_direction();
		let mut packets = Vec::new();
		for i in 0..num_packets {
			let (size, is_last_packet) = if i == (num_packets-1) {
				(last_packet_size, true)
			} else {
				(num_packets, false)
			};
			let packet_header = PacketHeader::new(unique_id, size as u16, 0, direction, is_last_packet);
			// Not a very Rusty way to put bytes into payload
			let mut packet_bytes = vec![PAYLOAD_DEFAULT_ELEMENT; payload_size];
			for j in 0..payload_size {
				if i*payload_size + j == msg_bytes.len() { break; }
				packet_bytes[j] = msg_bytes[i*payload_size + j];
			}
			let payload = Payload::new(packet_bytes);
			let packet = Box::new(Packet::new(packet_header, payload));
			packets.push(packet);
		}
		Ok(packets)
	}
	pub fn unpacketize(packets: &Vec<Box<Packet>>) -> Result<Box<Message>, PacketizerError> {
		let mut all_bytes = Vec::new();
		for packet in packets {
			let header = packet.get_header();
			let is_last_packet = header.is_last_packet();
			let last_packet_size = header.get_size();
			let mut payload = packet.get_payload(); 
			if is_last_packet { payload.get_bytes().truncate(last_packet_size as usize); }
			all_bytes.extend_from_slice(payload.get_bytes().as_slice());
		}
		let serialized = try!(str::from_utf8(&all_bytes));
		let deserialized: DiscoverMsg = try!(serde_json::from_str(&serialized));
		Ok(Box::new(deserialized))
	}
	fn packet_payload_size(len: usize) -> usize {
		match len-1 { 
			0...PACKET_MIN           => PAYLOAD_MIN,
			PAYLOAD_MIN...PAYLOAD_MAX => len,
			_                         => PAYLOAD_MAX
		}		
	}
	fn hash<T: Hash>(t: &T) -> u64 {
	    let mut s = DefaultHasher::new();
	    t.hash(&mut s);
	    s.finish()
	}
}
const PACKET_HEADER_SIZE: usize = 8 + 2 + 4 + 1 + 9; // Last value is padding
#[derive(Debug, Copy, Clone)]
pub struct PacketHeader {
	uniquifier: u64,	// Unique identifier of this message
	size: u16,			// Number of packets in message if not last packet, 0 => stream
						// Number of bytes in last packet if last packet, 0 => Error
	other_index: u32,	// Routing table index on receiving cell
	flags: u8,    		// Various flags
						// xxxx xxx0 => rootcast
						// xxxx xxx1 => leafcast
						// xxxx xx0x => Not last packet
						// xxxx xx1x => Last packet
						// xx00 xxxx => EC Protocol to CellAgent
						// xx01 xxxx => EC Protocol to VirtualMachine
						// xx10 xxxx => Legacy Protocol to VirtualMachine
}
impl PacketHeader {
	pub fn new(uniquifier: Uniquifier, size: PacketNo, other_index: TableIndex, direction: MsgDirection,
			is_last_packet: bool) -> PacketHeader {
		// Assertion fails if I forgot to change PACKET_HEADER_SIZE when I changed PacketHeader struct
		assert_eq!(PACKET_HEADER_SIZE, mem::size_of::<PacketHeader>());
		let mut flags = match direction {
			MsgDirection::Rootward => 0,
			MsgDirection::Leafward => 1
		};
		flags = if is_last_packet { flags | 2 } else { flags };
		PacketHeader { uniquifier: uniquifier, size: size, 
			other_index: other_index, flags: flags }
	}
	pub fn get_uniquifier(&self) -> Uniquifier { self.uniquifier }
	fn set_uniquifier(&mut self, uniquifier: Uniquifier) { self.uniquifier = uniquifier; }
	pub fn get_size(&self) -> PacketNo { self.size }
	pub fn is_rootcast(&self) -> bool { (self.flags & 1) == 0 }
	pub fn is_leafcast(&self) -> bool { !self.is_rootcast() }
	pub fn is_last_packet(&self) -> bool { (self.flags & 2) == 2 }
	fn set_direction(&mut self, direction: MsgDirection) { 
		match direction {
			MsgDirection::Leafward => self.flags = self.flags & 00000001,
			MsgDirection::Rootward => self.flags = self.flags & 11111110
		}
	}
	pub fn get_other_index(&self) -> u32 { self.other_index }
	pub fn set_other_index(&mut self, other_index: TableIndex) { self.other_index = other_index; }
}
impl fmt::Display for PacketHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Table Index {} ", self.other_index);
		if self.is_rootcast() { s = s + "Rootward"; }
		else                  { s = s + "Leafward"; }
		if self.is_last_packet() { s = s + " Last packet"; }
		else                     { s = s + " Not last packet"; }
		write!(f, "{}", s) 
	}
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PacketizerError {
	Size(SizeError),
	Utf8(str::Utf8Error),
	Unpacketize(UnpacketizeError),
	Serde(serde_json::Error)
}
impl Error for PacketizerError {
	fn description(&self) -> &str {
		match *self {
			PacketizerError::Size(ref err) => err.description(),
			PacketizerError::Utf8(ref err) => err.description(),
			PacketizerError::Unpacketize(ref err) => err.description(),
			PacketizerError::Serde(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketizerError::Size(ref err) => Some(err),
			PacketizerError::Utf8(ref err) => Some(err),
			PacketizerError::Unpacketize(ref err) => Some(err),
			PacketizerError::Serde(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketizerError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketizerError::Size(ref err) => write!(f, "Packetizer Size Error caused by {}", err),
			PacketizerError::Utf8(ref err) => write!(f, "Packetizer Utf8 Error caused by {}", err),
			PacketizerError::Unpacketize(ref err) => write!(f, "Packetizer Unpacketize Error caused by {}", err),
			PacketizerError::Serde(ref err) => write!(f, "Packetizer Serialization Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError { 
	pub fn new(size: usize) -> SizeError {
		SizeError { msg: format!("{} is not a valid packet size", size) }
	}
}
impl Error for SizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for SizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<SizeError> for PacketizerError {
	fn from(err: SizeError) -> PacketizerError { PacketizerError::Size(err) }
}
#[derive(Debug)]
pub struct UnpacketizeError { msg: String }
impl UnpacketizeError { 
	pub fn new(supplied: usize, required: usize) -> UnpacketizeError {
		if supplied == 0 {
			UnpacketizeError { msg: format!("Zero bytes supplied") }
		} else {
			UnpacketizeError { msg: format!("Only {} bytes of {} required", supplied, required) }
		}
	}
}
impl Error for UnpacketizeError {
	fn description(&self) -> &str { &self.msg }
	fn cause(&self) -> Option<&Error> { None }
}
impl fmt::Display for UnpacketizeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.msg)
	}
}
impl From<UnpacketizeError> for PacketizerError {
	fn from(err: UnpacketizeError) -> PacketizerError { PacketizerError::Unpacketize(err) }
}
impl From<serde_json::Error> for PacketizerError{
	fn from(err: serde_json::Error) -> PacketizerError { PacketizerError::Serde(err) }
}
impl From<Utf8Error> for PacketizerError{
	fn from(err: Utf8Error) -> PacketizerError { PacketizerError::Utf8(err) }
}
