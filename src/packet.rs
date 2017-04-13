use std;
use std::fmt;
use std::mem;
use rand;
use std::str;
use std::str::Utf8Error;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde;
use serde_json;
use config::{PACKET_SMALL, PACKET_MEDIUM, PACKET_LARGE, PacketNo, TableIndex, Uniquifier};
use message::{Message, DiscoverMsg, DiscoverDMsg, MsgDirection};

const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
const PAYLOAD_SMALL:  usize = PACKET_SMALL  - PACKET_HEADER_SIZE;
const PAYLOAD_MEDIUM: usize = PACKET_MEDIUM - PACKET_HEADER_SIZE;
const PAYLOAD_LARGE:  usize = PACKET_LARGE  - PACKET_HEADER_SIZE;
const LARGEST_MSG: usize = std::u32::MAX as usize;

type PacketElement = u8; // Packets are made up of bytes
#[derive(Copy)]
pub enum Packet {
	Small  { header: PacketHeader, payload: [PacketElement; PAYLOAD_SMALL]  },
	Medium { header: PacketHeader, payload: [PacketElement; PAYLOAD_MEDIUM] },
	Large  { header: PacketHeader, payload: [PacketElement; PAYLOAD_LARGE]  }
}
impl Packet {
	pub fn get_header(&self) -> PacketHeader {
		match *self {
			Packet::Small  { header, payload } => header,
			Packet::Medium { header, payload } => header,
			Packet::Large  { header, payload } => header,
		}
	}
	pub fn get_payload(&self) -> Vec<PacketElement> {
		match *self {
			Packet::Small  { header, payload } => payload.iter().cloned().collect(),
			Packet::Medium { header, payload } => payload.iter().cloned().collect(),
			Packet::Large  { header, payload } => payload.iter().cloned().collect(),
		}
	}
	pub fn get_payload_bytes(&self) -> Vec<u8> { self.get_payload().iter().cloned().collect() }
	pub fn get_size(&self) -> usize {
		match *self {
			Packet::Small  {header, payload} => PACKET_SMALL, 
			Packet::Medium {header, payload} => PACKET_MEDIUM,
			Packet::Large  {header, payload} => PACKET_LARGE
		}
	}
	pub fn get_payload_size(&self) -> usize { 
		match *self {
			Packet::Small  {header, payload} => PAYLOAD_SMALL, 
			Packet::Medium {header, payload} => PAYLOAD_MEDIUM,
			Packet::Large  {header, payload} => PAYLOAD_LARGE
		}
	}
	pub fn stringify(&self) -> String {
		let mut s = format!("Header: {}, Payload: ", self.get_header());
		let payload = self.get_payload();
		s = s + &format!("{:?}", &payload[0..10]); 
		s
	}
}
impl fmt::Display for Packet {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("Header: {}, Payload: ", self.get_header());
		let payload = self.get_payload();
		s = s + &format!("{:?}", &payload[0..10]); 
		write!(f, "{}", s)
	} 	
}
impl Clone for Packet {
	fn clone(&self) -> Packet { *self }
}
impl fmt::Debug for Packet { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt(f) }
}
pub struct Packetizer {}
impl Packetizer {
	pub fn packetize<M>(msg: &M, flags: [bool;4]) -> Result<Vec<Box<Packet>>, PacketizerError>
			where M: Message + Hash + serde::Serialize {
		let serialized = try!(serde_json::to_string(&msg));
		let bytes = serialized.clone().into_bytes();
		let payload_size = try!(Packetizer::packet_payload_size(bytes.len()));
		let num_packets = (bytes.len() + payload_size - 1)/ payload_size; // Poor man's ceiling
		let last_packet_size = bytes.len() - (num_packets-1)*payload_size;
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
				if i*payload_size + j == bytes.len() { break; }
				packet_bytes[j] = bytes[i*payload_size + j];
			}
			match payload_size {
				PAYLOAD_SMALL => {
					if bytes.len() > PAYLOAD_SMALL { 
						return Err(PacketizerError::Size(SizeError::new(bytes.len())))
					}
					let mut payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_SMALL];	
					// Not a very Rusty way to put bytes into payload
					for i in 0..bytes.len() { payload[i as usize] = packet_bytes[i as usize]; }
					let small = Packet::Small { header: packet_header, payload: payload }; 
					packets.push(Box::new(small) as Box<Packet>);
				},
				PAYLOAD_MEDIUM => {
					if bytes.len() > PAYLOAD_MEDIUM { 
						return Err(PacketizerError::Size(SizeError::new(bytes.len())))
					}
					let mut payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_MEDIUM];				
					// Not a very Rusty way to put bytes into payload
					for i in 0..bytes.len() { payload[i as usize] = packet_bytes[i as usize]; }
					let small = Packet::Medium { header: packet_header, payload: payload }; 
					packets.push(Box::new(small) as Box<Packet>);
				},
				PAYLOAD_LARGE => {
					if bytes.len() > PAYLOAD_LARGE { 
						return Err(PacketizerError::Size(SizeError::new(bytes.len())))
					}
					let mut payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_LARGE];				
					// Not a very Rusty way to put bytes into payload
					for i in 0..bytes.len() { payload[i as usize] = packet_bytes[i as usize]; }
					let small = Packet::Large { header: packet_header, payload: payload }; 
					packets.push(Box::new(small) as Box<Packet>);
				}
				_ => return Err(PacketizerError::Size(SizeError::new(payload_size)))
			}
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
			if is_last_packet { payload.truncate(last_packet_size as usize); }
			all_bytes.extend_from_slice(payload.as_slice());
		}
		let serialized = try!(str::from_utf8(&all_bytes));
		let deserialized: DiscoverMsg = try!(serde_json::from_str(&serialized));
		Ok(Box::new(deserialized))
	}
	fn packet_payload_size(len: usize) -> Result<usize, PacketizerError> {
		match len-1 { 
			0...PAYLOAD_SMALL              => Ok(PAYLOAD_SMALL),
			PAYLOAD_SMALL...PAYLOAD_MEDIUM => Ok(PAYLOAD_MEDIUM),
			PAYLOAD_MEDIUM...LARGEST_MSG   => Ok(PAYLOAD_LARGE),
			_ => Err(PacketizerError::Size(SizeError::new(len)))
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
