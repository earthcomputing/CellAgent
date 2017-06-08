use std::fmt;
use std::mem;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use rand;
use std::str;
use std::str::Utf8Error;
use std::hash::{Hash};
use serde;
use serde_json;
use config::{MSG_HEADER_DELIMITER, PACKET_MIN, PACKET_MAX, PAYLOAD_DEFAULT_ELEMENT, 
	PacketElement, PacketNo, TableIndex, Uniquifier};
use message::{Message, DiscoverMsg, DiscoverDMsg, MsgDirection, MsgType};
 
//const LARGEST_MSG: usize = std::u32::MAX as usize;
const PAYLOAD_MIN: usize = PACKET_MAX - PACKET_HEADER_SIZE;
const PAYLOAD_MAX: usize = PACKET_MAX - PACKET_HEADER_SIZE;

static PACKET_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
#[derive(Debug, Copy)]
pub struct Packet {
	header: PacketHeader, 
	payload: Payload,
	packet_count: usize
}
#[deny(unused_must_use)]
impl Packet {
	fn new(header: PacketHeader, payload: Payload) -> Packet {
		Packet { header: header, payload: payload, packet_count: Packet::get_next_count() }
	}
	pub fn get_next_count() -> usize { PACKET_COUNT.fetch_add(1, Ordering::SeqCst) } 
	pub fn get_count(&self) -> usize { self.packet_count }
	pub fn get_header(&self) -> PacketHeader { self.header }
	pub fn get_payload(&self) -> Payload { self.payload }
//	pub fn get_payload_bytes(&self) -> Vec<u8> { self.get_payload().get_bytes() }
//	pub fn get_payload_size(&self) -> usize { self.payload.get_no_bytes() }
}
impl fmt::Display for Packet {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Header: {}, Payload: {}", self.header, self.payload);
		write!(f, "{}", s)
	} 	
}
impl Clone for Packet {
	fn clone(&self) -> Packet { *self }
}
const PACKET_HEADER_SIZE: usize = 8 + 2 + 4 + 1 + 9; // Last value is padding
#[derive(Debug, Copy, Clone)]
pub struct PacketHeader {
	uniquifier: u64,	// Unique identifier of this message
	size: u16,			// Number of packets remaining in message if not last packet
						// Number of bytes in last packet if last packet, 0 => Error
	other_index: u32,	// Routing table index on receiving cell
	flags: u8,    		// Various flags
						// xxxx xxx0 => rootcast
						// xxxx xxx1 => leafcast
						// xxxx xx0x => Not last packet
						// xxxx xx1x => Last packet
						// xx00 xxxx => EC Protocol to VirtualMachine
						// xx01 xxxx => Legacy Protocol to VirtualMachine
}
#[deny(unused_must_use)]
impl PacketHeader {
	pub fn new(uniquifier: Uniquifier, size: PacketNo, other_index: TableIndex, direction: MsgDirection,
			is_last_packet: bool) -> PacketHeader {
		// Assertion fails if I forgot to change PACKET_HEADER_SIZE when I changed PacketHeader struct
		assert_eq!(PACKET_HEADER_SIZE, mem::size_of::<PacketHeader>());
		let flags = if is_last_packet { 2 } else { 0 };
		let mut ph = PacketHeader { uniquifier: uniquifier, size: size, 
			other_index: other_index, flags: flags };
		ph.set_direction(direction);
		ph
	}
	pub fn get_uniquifier(&self) -> Uniquifier { self.uniquifier }
//	fn set_uniquifier(&mut self, uniquifier: Uniquifier) { self.uniquifier = uniquifier; }
	pub fn get_size(&self) -> PacketNo { self.size }
	pub fn is_leafcast(&self) -> bool { (self.flags & 1) == 1 }
	pub fn is_rootcast(&self) -> bool { !self.is_leafcast() }
	pub fn is_last_packet(&self) -> bool { (self.flags & 2) == 2 }
	fn set_direction(&mut self, direction: MsgDirection) { 
		match direction {
			MsgDirection::Leafward => self.flags = self.flags | 1,
			MsgDirection::Rootward => self.flags = self.flags & 254
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
		if self.is_last_packet() { s = s + ", Last packet"; }
		else                     { s = s + ", Not last packet"; }
		s = s + &format!(", Size {}", self.size);
		write!(f, "{}", s) 
	}
}
#[derive(Copy)]
pub struct Payload {
	bytes: [PacketElement; PAYLOAD_MAX],
}
#[deny(unused_must_use)]
impl Payload {
	pub fn new(data_bytes: Vec<PacketElement>) -> Payload {
		let no_data_bytes = data_bytes.len();
		let mut bytes = [0; PAYLOAD_MAX];
		for i in 0..no_data_bytes { bytes[i] = data_bytes[i]; }
		Payload { bytes: bytes }
	}
	fn get_bytes(&self) -> Vec<PacketElement> { self.bytes.iter().cloned().collect() }
//	fn get_no_bytes(&self) -> usize { self.get_bytes().len() }
//	fn get_msg_bytes(&self) -> Vec<PacketElement> {
//		let total_bytes = self.get_bytes();
//		total_bytes[0..self.no_data_bytes as usize].iter().cloned().collect()
//	}
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
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = &format!("{:?}", &self.bytes[0..10]); 
		write!(f, "{}", s)
	}
}
pub struct Packetizer {}
#[deny(unused_must_use)]
impl Packetizer {
	pub fn packetize<M>(msg: &M, other_index: TableIndex) -> Result<Vec<Box<Packet>>, PacketizerError>
			where M: Message + Hash + serde::Serialize {
		let msg_type = msg.get_header().get_msg_type();
		let serialized_msg_type = serde_json::to_string(&msg_type)?;
		let serialized = serde_json::to_string(&msg)?;
		let mut msg_bytes = serialized_msg_type.clone().into_bytes();
		msg_bytes.push(MSG_HEADER_DELIMITER as u8);
		msg_bytes.append(&mut serialized.into_bytes());
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
				(num_packets - i, false)
			};
			let packet_header = PacketHeader::new(unique_id, size as u16, other_index, direction, is_last_packet);
			// Not a very Rusty way to put bytes into payload
			let mut packet_bytes = vec![PAYLOAD_DEFAULT_ELEMENT; payload_size];
			for j in 0..payload_size {
				if i*payload_size + j == msg_bytes.len() { break; }
				packet_bytes[j] = msg_bytes[i*payload_size + j];
			}
			let payload = Payload::new(packet_bytes);
			let packet = Box::new(Packet::new(packet_header, payload));
			//println!("Packet: packet {} for msg {}", packet.get_packet_count(), msg.get_count());
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
			let payload = packet.get_payload(); 
			all_bytes.extend_from_slice(payload.get_bytes().as_slice());
			if is_last_packet { all_bytes.truncate(last_packet_size as usize); }
		}
		let serialized = str::from_utf8(&all_bytes)?;
		let mut split = serialized.splitn(2, PAYLOAD_DEFAULT_ELEMENT as char);
		let deserialized;
		if let Some(serialized_msg_type) = split.next() {
			let msg_type = serde_json::from_str(serialized_msg_type)?;
			if let Some(serialized_msg) = split.next() {
				deserialized = match msg_type {
					MsgType::Discover  => Packetizer::make_discover(serialized_msg)?,
					MsgType::DiscoverD => Packetizer::make_discoverd(serialized_msg)?,
				};
			} else {
				return Err(PacketizerError::Unpacketize(UnpacketizeError::new(serialized)))
			}
		} else {
			return Err(PacketizerError::Unpacketize(UnpacketizeError::new(serialized)))			
		}
		Ok(deserialized)
	}
	fn make_discover(serialized: &str) -> Result<Box<Message>, PacketizerError>{
		let msg: DiscoverMsg = serde_json::from_str(&serialized)?;
		Ok(Box::new(msg))
	}
	fn make_discoverd(serialized: &str) -> Result<Box<Message>, PacketizerError>{
		let msg: DiscoverDMsg = serde_json::from_str(&serialized)?;
		Ok(Box::new(msg))
	}
	fn packet_payload_size(len: usize) -> usize {
		match len-1 { 
			0...PACKET_MIN           => PAYLOAD_MIN,
			PAYLOAD_MIN...PAYLOAD_MAX => len,
			_                         => PAYLOAD_MAX
		}		
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
			PacketizerError::Serde(ref err) => write!(f, "Packetizer Serde Error caused by {}", err),
		}
	}
}
#[derive(Debug)]
pub struct SizeError { msg: String }
impl SizeError { 
//	pub fn new(size: usize) -> SizeError {
//		SizeError { msg: format!("{} is not a valid packet size", size) }
//	}
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
#[deny(unused_must_use)]
impl UnpacketizeError { 
	pub fn new(serialized: &str) -> UnpacketizeError {
		UnpacketizeError { msg: format!("Cannot deserialize {}", serialized) }
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
