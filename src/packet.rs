use std;
use std::fmt;
use std::mem;
use std::str;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde;
use serde_json;
use config::{PACKET_SMALL, PACKET_MEDIUM, PACKET_LARGE};
use message::Message;

const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
const PAYLOAD_SMALL:  usize = PACKET_SMALL  - PACKET_HEADER_SIZE;
const PAYLOAD_MEDIUM: usize = PACKET_MEDIUM - PACKET_HEADER_SIZE;
const PAYLOAD_LARGE:  usize = PACKET_LARGE  - PACKET_HEADER_SIZE;
const LARGEST_MSG: usize = std::u32::MAX as usize;

type PacketElement = u8;
pub trait Packet {
	fn get_header(&self) -> PacketHeader;
	fn set_header(&mut self, header: PacketHeader);
	fn get_payload(&self) -> Vec<PacketElement>;
	fn set_payload(&mut self, payload: &[u8]) -> Result<(), PacketizerError>;
	fn get_packet_payload_size(&self) -> usize;
	fn get_size(&self) -> usize;
	fn stringify(&self) -> String {
		format!("Payload: {:?}", self.get_payload())
	}
}
pub struct Packetizer {}
impl Packetizer {
	pub fn packetize<M>(msg: &M, other_index: u32) -> Result<Vec<Box<Packet>>, PacketizerError>
			where M: Message + Hash + serde::Serialize {
		let serialized = try!(serde_json::to_string(&msg));
		let bytes = serialized.into_bytes();
		msg.get_header().set_msg_size(bytes.len());
		// Redo after putting msg_size into message header
		let serialized = try!(serde_json::to_string(&msg));
		let bytes = serialized.into_bytes();
		let packet = try!(Packetizer::packet_type(bytes.len()));
		let payload_size = packet.get_packet_payload_size();
		let num_packets = (bytes.len() + payload_size - 1)/ payload_size; // Poor man's ceiling
		let unique_id = Packetizer::hash(&msg);
		let direction = msg.is_rootward();
		let mut packet_header = packet.get_header();
		packet_header.set_uniquifier(unique_id);
		packet_header.set_index(other_index);
		packet_header.set_direction(direction);
		let mut packets = Vec::new();
		for i in 0..num_packets {
			packet_header.set_count(num_packets - i);
			let mut packet_bytes = vec![PAYLOAD_DEFAULT_ELEMENT; payload_size];
			for j in 0..payload_size {
				if i*payload_size + j == bytes.len() { break; }
				packet_bytes[j] = bytes[i*payload_size + j];
			}
			let (mut small, mut medium, mut large) = (PacketSmall::new(), PacketMedium::new(), 
					PacketLarge::new());
			match payload_size {
				PAYLOAD_SMALL => {
					small.set_header(packet_header);
					try!(small.set_payload(&bytes));
					packets.push(Box::new(small) as Box<Packet>);
				},
				PAYLOAD_MEDIUM => {
					medium.set_header(packet_header);
					try!(medium.set_payload(&bytes));
					packets.push(Box::new(medium) as Box<Packet>);
				},
				PAYLOAD_LARGE => {
					large.set_header(packet_header);
					try!(large.set_payload(&bytes));
					packets.push(Box::new(large) as Box<Packet>);
				}
				_ => return Err(PacketizerError::Size(SizeError::new(payload_size)))
			}
		}
		Ok(packets)
	}
	fn packet_type(len: usize) -> Result<Box<Packet>, PacketizerError> {
		match len-1 { 
			0...PAYLOAD_SMALL              => Ok(Box::new(PacketSmall::new())),
			PAYLOAD_SMALL...PAYLOAD_MEDIUM => Ok(Box::new(PacketMedium::new())),
			PAYLOAD_MEDIUM...LARGEST_MSG   => Ok(Box::new(PacketLarge::new())),
			_ => Err(PacketizerError::Size(SizeError::new(len)))
		}		
	}
	fn hash<T: Hash>(t: &T) -> u64 {
	    let mut s = DefaultHasher::new();
	    t.hash(&mut s);
	    s.finish()
	}
}
const PACKET_HEADER_SIZE: usize = 8 + 4 + 4 + 1 + 7; // Last value is padding
#[derive(Copy, Clone)]
pub struct PacketHeader {
	uniquifier: u64,	// Unique identifier of this message
	count: u32,			// Number of packets remaining for this message including this one
	index: u32,			// Routing table index on receiving cell
	is_rootcast: bool,	// Rootcast or Leafcast
}
impl PacketHeader {
	pub fn new() -> PacketHeader {
		// Assertion fails if I forgot to change const when I changed struct
		assert_eq!(PACKET_HEADER_SIZE, mem::size_of::<PacketHeader>());
		PacketHeader { uniquifier: 0, count: 0, index: 0, is_rootcast: false }
	}
	pub fn get_uniquifier(&self) -> u64 { self.uniquifier }
	fn set_uniquifier(&mut self, uniquifier: u64) { self.uniquifier = uniquifier; }
	pub fn get_count(&self) -> u32 { self.count }
	fn set_count(&mut self, count: usize) { self.count = count as u32; }
	pub fn is_root_cast(&self) -> bool { self.is_rootcast }
	pub fn is_leaf_cast(&self) -> bool { self.is_rootcast }
	fn set_direction(&mut self, direction: bool) { self.is_rootcast = direction; }
	pub fn get_index(&self) -> u32 { self.index }
	pub fn set_index(&mut self, index: u32) { self.index = index; }
	pub fn stringify(&self) -> String {
		let mut s = format!("Table Index {}: ", self.index);
		if self.is_rootcast { s = s + "Rootward"; }
		else                { s = s + "Leafward"; }
		s
	}
}
#[derive(Copy)]
pub struct PacketSmall {
	header: PacketHeader,
	payload: [u8; PAYLOAD_SMALL],
}
impl PacketSmall {
	pub fn new() -> PacketSmall {
		let header = PacketHeader::new();
		PacketSmall { header: header, payload: [0; PAYLOAD_SMALL] }
	}
}
impl Packet for PacketSmall {
	fn get_header(&self) -> PacketHeader { self.header }
	fn set_header(&mut self, header: PacketHeader) { self.header = header; }
	fn get_payload(&self) -> Vec<PacketElement> { self.payload.iter().cloned().collect() }
	fn set_payload(&mut self, payload: &[u8]) -> Result<(), PacketizerError> { 
		if payload.len() > PAYLOAD_SMALL { 
			return Err(PacketizerError::Size(SizeError::new(payload.len())))
		}
		self.payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_SMALL];
		for i in payload.iter() { self.payload[*i as usize] = payload[*i as usize]; } 
		Ok(())
	}
	fn get_size(&self) -> usize { PACKET_SMALL }
	fn get_packet_payload_size(&self) -> usize { PAYLOAD_SMALL }
}
impl Clone for PacketSmall {
	fn clone(&self) -> PacketSmall { *self }
}
impl fmt::Debug for PacketSmall { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl fmt::Display for PacketSmall { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
#[derive(Copy)]
pub struct PacketMedium {
	header: PacketHeader,
	payload: [u8; PAYLOAD_MEDIUM],
}
impl PacketMedium {
	pub fn new() -> PacketMedium {
		let header = PacketHeader::new(); 
		PacketMedium { header: header, payload: [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_MEDIUM] }
	}
}
impl Packet for PacketMedium {
	fn get_header(&self) -> PacketHeader { self.header }
	fn set_header(&mut self, header: PacketHeader) { self.header = header; }
	fn get_payload(&self) -> Vec<PacketElement> {
		self.payload.iter().cloned().collect()
	}
	fn set_payload(&mut self, payload: &[u8]) -> Result<(), PacketizerError> { 
		if payload.len() > PAYLOAD_MEDIUM { 
			return Err(PacketizerError::Size(SizeError::new(payload.len())))
		}
		self.payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_MEDIUM];
		for i in payload.iter() { self.payload[*i as usize] = payload[*i as usize]; } 
		Ok(())
	}
	fn get_size(&self) -> usize { PACKET_MEDIUM }
	fn get_packet_payload_size(&self) -> usize { PAYLOAD_MEDIUM }
}
impl Clone for PacketMedium {
	fn clone(&self) -> PacketMedium { *self }
}
impl fmt::Debug for PacketMedium { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl fmt::Display for PacketMedium { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
#[derive(Copy)]
pub struct PacketLarge {
	header: PacketHeader,
	payload: [u8; PAYLOAD_LARGE],
}
impl PacketLarge {
	pub fn new() -> PacketLarge {
		let header = PacketHeader::new();
		PacketLarge { header: header, payload: [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_LARGE] }
	}
}
impl Packet for PacketLarge {
	fn get_header(&self) -> PacketHeader { self.header }
	fn set_header(&mut self, header: PacketHeader) { self.header = header; }
	fn get_payload(&self) -> Vec<PacketElement> {
		self.payload.iter().cloned().collect()
	}
	fn set_payload(&mut self, payload: &[u8]) -> Result<(), PacketizerError> { 
		if payload.len() > PAYLOAD_LARGE { 
			return Err(PacketizerError::Size(SizeError::new(payload.len())))
		}
		self.payload = [PAYLOAD_DEFAULT_ELEMENT; PAYLOAD_LARGE];
		Ok(())
	}
	fn get_size(&self) -> usize { PACKET_LARGE }	
	fn get_packet_payload_size(&self) -> usize { PAYLOAD_LARGE }
}
impl Clone for PacketLarge {
	fn clone(&self) -> PacketLarge { *self }
}
impl fmt::Debug for PacketLarge { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
impl fmt::Display for PacketLarge { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.stringify()) }
}
// Errors
use std::error::Error;
#[derive(Debug)]
pub enum PacketizerError {
	Size(SizeError),
	Serde(serde_json::Error)
}
impl Error for PacketizerError {
	fn description(&self) -> &str {
		match *self {
			PacketizerError::Size(ref err) => err.description(),
			PacketizerError::Serde(ref err) => err.description(),
		}
	}
	fn cause(&self) -> Option<&Error> {
		match *self {
			PacketizerError::Size(ref err) => Some(err),
			PacketizerError::Serde(ref err) => Some(err),
		}
	}
}
impl fmt::Display for PacketizerError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			PacketizerError::Size(ref err) => write!(f, "Packetizer Size Error caused by {}", err),
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
impl From<serde_json::Error> for PacketizerError{
	fn from(err: serde_json::Error) -> PacketizerError { PacketizerError::Serde(err) }
}
