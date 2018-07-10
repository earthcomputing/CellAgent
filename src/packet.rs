use std::fmt;
use std::mem;
use std::collections::HashMap;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};
use std::str;

use rand;
use serde;
use serde_json;
//use uuid::Uuid;

use config::{PACKET_MIN, PACKET_MAX, PAYLOAD_DEFAULT_ELEMENT, 
	ByteArray, MsgID, PacketNo};
use message::{Message, MsgDirection, MsgType, TypePlusMsg};
use name::{Name, TreeID};
use utility::S;
use uuid_fake::Uuid;
 
//const LARGEST_MSG: usize = std::u32::MAX as usize;
const PAYLOAD_MIN: usize = PACKET_MAX - PACKET_HEADER_SIZE;
const PAYLOAD_MAX: usize = PACKET_MAX - PACKET_HEADER_SIZE;

pub type PacketAssemblers = HashMap<MsgID, PacketAssembler>;

static PACKET_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
#[derive(Debug, Copy)]
pub struct Packet {
	header: PacketHeader, 
	payload: Payload,
	packet_count: usize
}
#[deny(unused_must_use)]
impl Packet {
	fn new(msg_id: MsgID, tree_id: &TreeID, size: PacketNo, direction: MsgDirection,
           is_last_packet: bool, is_blocking: bool, data_bytes: Vec<u8>) -> Packet {
        let header = PacketHeader::new(tree_id);
        let payload = Payload::new(msg_id, size, direction, is_last_packet, is_blocking, data_bytes);
		Packet { header, payload, packet_count: Packet::get_next_count() }
	}
    pub fn get_uuid(&self) -> Uuid { self.header.get_uuid() }
	pub fn get_next_count() -> usize { PACKET_COUNT.fetch_add(1, Ordering::SeqCst) } 
	pub fn get_count(&self) -> usize { self.packet_count }  // For debugging
	pub fn get_header(&self) -> PacketHeader { self.header }
	pub fn get_payload(&self) -> Payload { self.payload }
	pub fn get_tree_uuid(&self) -> Uuid { self.header.get_uuid() }
    pub fn is_blocking(&self) -> bool { self.payload.is_blocking() }
    pub fn is_last_packet(&self) -> bool { self.payload.is_last_packet() }
    pub fn get_bytes(&self) -> Vec<u8> { self.payload.bytes.iter().cloned().collect() }
    pub fn get_msg_id(&self) -> MsgID { self.payload.get_msg_id() }
    pub fn get_size(&self) -> PacketNo { self.payload.get_size() }
    pub fn is_leafcast(&self) -> bool {
        match self.payload.get_direction() {
            MsgDirection::Leafward => true,
            _ => false
        }
    }
    pub fn is_rootcast(&self) -> bool { !self.is_leafcast() }
    fn set_direction(&mut self, direction: MsgDirection) { self.payload.set_direction(direction) }
    // Debug hack to get tree_id out of packets.  Assumes msg is one packet
    pub fn get_tree_id(self) -> TreeID {
        let msg = MsgType::get_msg(&vec![self]).unwrap();
        msg.get_tree_id().unwrap().clone()
    }
    //	pub fn get_payload_bytes(&self) -> Vec<u8> { self.get_payload().get_bytes() }
    //	pub fn get_payload_size(&self) -> usize { self.payload.get_no_bytes() }
}
impl fmt::Display for Packet {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let bytes = self.payload.get_bytes();
		let len = if self.payload.is_last_packet() {
			*self.payload.get_size() as usize
		} else {
			bytes.len()
		};
		let s = format!("Packet {}: Header: {}, Payload: {:?}", self.packet_count, self.header, str::from_utf8(&bytes[0..len]));
		write!(f, "{}", s)
	} 	
}
impl Clone for Packet {
	fn clone(&self) -> Packet { *self }
}
const PACKET_HEADER_SIZE: usize = 16; // Last value is padding
#[derive(Debug, Copy, Clone, Serialize)]
pub struct PacketHeader {
    uuid: Uuid,     // Tree identifier 16 bytes
}
impl PacketHeader {
	pub fn new(tree_id: &TreeID) -> PacketHeader {
		// Assertion fails if I forgot to change PACKET_HEADER_SIZE when I changed PacketHeader struct
        assert_eq!(PACKET_HEADER_SIZE, mem::size_of::<PacketHeader>());
        PacketHeader { uuid: tree_id.get_uuid() }
	}
	fn get_uuid(&self) -> Uuid { self.uuid }
//	pub fn set_uuid(&mut self, new_uuid: Uuid) { self.uuid = new_uuid; }
}
impl fmt::Display for PacketHeader { 
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut uuid = self.uuid.to_string();
		uuid.truncate(8);
		let s = &format!(", UUID {}", self.uuid );
		write!(f, "{}", s)
	}
}
#[derive(Copy)]
pub struct Payload {
    msg_id: MsgID,	// Unique identifier of this message
    size: PacketNo,	// Number of packets remaining in message if not last packet
                    // Number of bytes in last packet if last packet, 0 => Error
    is_last: bool,
    is_blocking: bool,
    direction: MsgDirection,
	bytes: [u8; PAYLOAD_MAX],
}
impl Payload {
	pub fn new(msg_id: MsgID, size: PacketNo, direction: MsgDirection,
               is_last: bool, is_blocking: bool, data_bytes: Vec<u8>) -> Payload {
		let no_data_bytes = data_bytes.len();
		let mut bytes: [u8; PAYLOAD_MAX] = [0; PAYLOAD_MAX];
        for i in 0..data_bytes.len() { bytes[i] = data_bytes[i]; }
		Payload { msg_id, size, is_last, is_blocking, direction, bytes }
	}
	fn get_bytes(&self) -> Vec<u8> { self.bytes.iter().cloned().collect() }
    fn get_msg_id(&self) -> MsgID { self.msg_id }
    fn get_size(&self) -> PacketNo { self.size }
    fn get_direction(&self) -> MsgDirection { self.direction }
    fn is_leafcast(&self) -> bool {
        match self.direction {
            MsgDirection::Leafward => true,
            _ => false
        }
    }
    fn is_rootcast(&self) -> bool { !self.is_leafcast() }
    fn is_last_packet(&self) -> bool { self.is_last }
    fn is_blocking(&self) -> bool { self.is_blocking }

    fn set_direction(&mut self, direction: MsgDirection) { self.direction = direction }
}
impl fmt::Display for Payload {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = format!("Message ID {}", *self.msg_id);
        if self.is_rootcast() { s = s + " Rootward"; }
        else                  { s = s + " Leafward"; }
        if self.is_last_packet() { s = s + ", Last packet"; }
        else                     { s = s + ", Not last packet"; }
        s = s + &format!(", Size {}", *self.size);
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
pub struct Serializer {}
impl Serializer {
	pub fn serialize<M>(msg: &M) -> Result<Box<Vec<u8>>, Error>
			where M: Message + serde::Serialize {		
		let msg_type = msg.get_header().get_msg_type();
		let serialized_msg = serde_json::to_string(msg).context(PacketError::Chain { func_name: "serialize", comment: S("msg")})?;
		let msg_obj = TypePlusMsg::new(msg_type, serialized_msg);
		let serialized = serde_json::to_string(&msg_obj).context(PacketError::Chain { func_name: "serialize", comment: S("msg_obj")})?;
		let msg_bytes = serialized.clone().into_bytes();
		Ok(Box::new(msg_bytes))
	}	
}
pub struct Packetizer {}
impl Packetizer {
	pub fn packetize(tree_id: &TreeID, msg_bytes: &ByteArray, direction: MsgDirection, is_blocking: bool)
			-> Vec<Packet> {
		let payload_size = Packetizer::packet_payload_size(msg_bytes.len());
		let num_packets = (msg_bytes.len() + payload_size - 1)/ payload_size; // Poor man's ceiling
		let last_packet_size = msg_bytes.len() - (num_packets-1)*payload_size;
		let msg_id = MsgID(rand::random()); // Can't use hash in case two cells send the same message
		let mut packets = Vec::new();
		for i in 0..num_packets {
			let (size, is_last_packet) = if i == (num_packets-1) {
				(last_packet_size, true)
			} else {
				(num_packets - i, false)
			};
			// Not a very Rusty way to put bytes into payload
			let mut packet_bytes = vec![PAYLOAD_DEFAULT_ELEMENT; payload_size];
			for j in 0..payload_size {
				if i*payload_size + j == msg_bytes.len() { break; }
				packet_bytes[j] = msg_bytes[i*payload_size + j];
			}
			let packet = Packet::new(msg_id, tree_id, PacketNo(size as u16), direction,
                                     is_last_packet, is_blocking, packet_bytes);
			//println!("Packet: packet {} for msg {}", packet.get_packet_count(), msg.get_count());
			packets.push(packet); 
		}
		packets
	}
	pub fn unpacketize(packets: &Vec<Packet>) -> Result<ByteArray, Error> {
		let mut all_bytes = Vec::new();
		for packet in packets {
			let is_last_packet = packet.is_last_packet();
			let last_packet_size = *packet.get_size() as usize;
			let payload = packet.get_payload();
			let mut bytes = packet.get_bytes();
			if is_last_packet {
				bytes.truncate(last_packet_size)
			};
			all_bytes.extend_from_slice(&bytes);
		}
		Ok(ByteArray(all_bytes))
		//Ok(str::from_utf8(&all_bytes).context(PacketError::Chain { func_name: "unpacketize", comment: S("")})?.to_string())
	}
	fn packet_payload_size(len: usize) -> usize {
		match len-1 { 
			0...PACKET_MIN           => PAYLOAD_MIN,
			PAYLOAD_MIN...PAYLOAD_MAX => len,
			_                         => PAYLOAD_MAX
		}		
	}
}
#[derive(Debug, Clone)]
pub struct PacketAssembler {
	msg_id: MsgID,
	packets: Vec<Packet>,
}
impl PacketAssembler {
	pub fn new(msg_id: MsgID) -> PacketAssembler {
		PacketAssembler { msg_id, packets: Vec::new() }
	}
	pub fn create(msg_id: MsgID, packets: &Vec<Packet>) -> PacketAssembler {
		PacketAssembler { msg_id, packets: packets.clone() }
	}
/*
	pub fn get_msg_id(&self) -> MsgID { self.msg_id }
	pub fn get_packets(&self) -> &Vec<Packet> { &self.packets }
	pub fn get_tree_uuid(&self) -> Option<Uuid> { 
		if let Some(packet) = self.packets.get(0) {
			Some(packet.get_header().get_tree_uuid())
		} else {
			None 
		}
	}
*/
	pub fn add(&mut self, packet: Packet) -> (bool, &Vec<Packet>) { 
		self.packets.push(packet); 
		(packet.is_last_packet(), &self.packets)
	}
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PacketError {
	#[fail(display = "PacketError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
}
