use std::{fmt,
          collections::HashMap,
          cmp::min,
          sync::atomic::{AtomicUsize, Ordering},
          str};

use rand;
use serde;
use serde_json;

use crate::app_message::{SenderMsgSeqNo, get_next_count};
use crate::config::{PACKET_MIN, PACKET_MAX, PAYLOAD_DEFAULT_ELEMENT,
    ByteArray, PacketNo};
use crate::ec_message::{Message, MsgType};
use crate::name::{PortTreeID, Name};
use crate::utility::{S, Stack};
use crate::uuid_ec::{Uuid, AitState};
 
//const LARGEST_MSG: usize = std::u32::MAX as usize;
const PACKET_HEADER_SIZE: usize = 16; // PacketHeader / Uuid
const PAYLOAD_MIN: usize = PACKET_MAX - PACKET_HEADER_SIZE;
const PAYLOAD_MAX: usize = PACKET_MAX - PACKET_HEADER_SIZE;

pub type PacketAssemblers = HashMap<SenderMsgSeqNo, PacketAssembler>;

static PACKET_COUNT: AtomicUsize = AtomicUsize::new(0);
#[derive(Debug, Clone, Serialize)]
pub struct Packet {
    header: PacketHeader,
    payload: Payload,
    packet_count: usize
}
impl Packet {
    fn new(sender_msg_seq_no: SenderMsgSeqNo, uuid: &Uuid, size: PacketNo,
           is_last_packet: bool, data_bytes: Vec<u8>) -> Packet {
        let header = PacketHeader::new(uuid);
        let payload = Payload::new(sender_msg_seq_no, size, is_last_packet, data_bytes);
        Packet { header, payload, packet_count: Packet::get_next_count() }
    }
    pub fn _make(header: PacketHeader, payload: Payload, packet_count: usize) -> Packet {
        Packet { header, payload, packet_count }
    }
    pub fn make_entl_packet() -> Packet {
        let mut uuid = Uuid::new();
        uuid.make_entl();
        Packet::new(get_next_count(), &uuid, PacketNo(1),
                    false, vec![])
    }
    
    pub fn get_next_count() -> usize { PACKET_COUNT.fetch_add(1, Ordering::SeqCst) }

    pub fn _get_header(&self) -> PacketHeader { self.header }
    pub fn _get_payload(&self) -> &Payload { &self.payload }
    pub fn get_count(&self) -> usize { self.packet_count }
    
    // Used for trace records
    pub fn to_string(&self) -> Result<String, Error> {
        let bytes = self.get_bytes();
        let string = std::str::from_utf8(&bytes)?.to_owned();
        let default_as_char = PAYLOAD_DEFAULT_ELEMENT as char;
        Ok(string.replace(default_as_char, ""))
    }

    // PacketHeader (delegate)
    pub fn get_tree_uuid(&self) -> Uuid { self.header.get_uuid() }

    // Payload (delegate)
    pub fn is_last_packet(&self) -> bool { self.payload.is_last_packet() }
    pub fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.payload.get_sender_msg_seq_no() }
    pub fn get_size(&self) -> PacketNo { self.payload.get_size() }
    pub fn get_bytes(&self) -> Vec<u8> { self.payload.bytes.iter().cloned().collect() }
    // pub fn get_payload_bytes(&self) -> Vec<u8> { self.get_payload().get_bytes() }
    // pub fn get_payload_size(&self) -> usize { self.payload.get_no_bytes() }

    // UUID Magic
    pub fn make_ait_send(&mut self) { self.header.make_ait_send() }
    pub fn make_ait_reply(&mut self) { self.header.make_ait_reply() }
    pub fn make_tock(&mut self) { self.header.make_tock() }
    pub fn is_ait(&self) -> bool { self.is_ait_recv() || self.is_ait_recv() }
    pub fn is_ait_send(&self) -> bool { self.header.get_uuid().is_ait_send() }
    pub fn is_ait_recv(&self) -> bool { self.header.get_uuid().is_ait_recv() }
    pub fn _is_entl(&self) -> bool { self.header.get_uuid()._is_entl() }
    pub fn get_ait_state(&self) -> AitState { self.get_tree_uuid().get_ait_state() }
    pub fn time_reverse(&mut self) { self.header.get_uuid().time_reverse(); }
    pub fn next_ait_state(&mut self) -> Result<AitState, Error> {
        let mut uuid = self.header.get_uuid();
        uuid.next()?;
        self.header = PacketHeader::new(&uuid);
        Ok(uuid.get_ait_state())
    }
    // Wrapping and unwrapping following failover
    pub fn _wrap(&mut self, rw_port_tree_id: PortTreeID) {
        self.payload.wrapped_header._push(self.header);
        self.header = PacketHeader::new(&rw_port_tree_id.get_uuid());
    }
    pub fn _unwrap(&mut self) -> bool {
        if let Some(wrapped_header) = self.payload.wrapped_header._pop(){
            self.header = wrapped_header;
            true
        } else {
            false
        }
    }
    // Payload (Deep Packet Inspection)
    // Debug hack to get tree name out of packets.  Assumes msg is one packet
    pub fn get_port_tree_id(&self) -> PortTreeID {
        let msg = MsgType::get_msg(&vec![self.clone()]).unwrap();
        msg.get_port_tree_id()
    }
}
impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
#[derive(Debug, Copy, Clone, Serialize)]
pub struct PacketHeader {
    uuid: Uuid,     // Tree identifier 16 bytes
}
impl PacketHeader {
    pub fn new(uuid: &Uuid) -> PacketHeader {
        PacketHeader { uuid: *uuid }
    }
    fn get_uuid(&self) -> Uuid { self.uuid }
    fn make_ait_send(&mut self) { self.uuid.make_ait_send(); }
    fn make_ait_reply(&mut self) { self.uuid.make_ait_reply(); }
    fn make_tock(&mut self) { self.uuid.make_tock(); }
}
impl fmt::Display for PacketHeader { 
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut uuid = self.uuid.to_string();
        uuid.truncate(8);
        let s = &format!(", UUID {}", self.uuid );
        write!(f, "{}", s)
    }
}

#[derive(Clone)]
pub struct Payload {
    sender_msg_seq_no: SenderMsgSeqNo,  // Unique identifier of this message
    size: PacketNo, // Number of packets remaining in message if not last packet
                    // Number of bytes in last packet if last packet, 0 => Error
    is_last: bool,
    bytes: [u8; PAYLOAD_MAX],
    wrapped_header: Stack<PacketHeader>,
}
impl Payload {
    pub fn new(sender_msg_seq_no: SenderMsgSeqNo, size: PacketNo,
               is_last: bool, data_bytes: Vec<u8>) -> Payload {
        let mut bytes = [0 as u8; PAYLOAD_MAX];
        // Next line recommended by clippy, but I think the loop is clearer
        //bytes[..min(data_bytes.len(), PAYLOAD_MAX)].clone_from_slice(&data_bytes[..min(data_bytes.len(), PAYLOAD_MAX)]);
        for i in 0..min(data_bytes.len(), PAYLOAD_MAX) { bytes[i] = data_bytes[i]; }
        Payload { sender_msg_seq_no, size, is_last, bytes, wrapped_header: Stack::new() }
    }
    fn get_bytes(&self) -> Vec<u8> { self.bytes.iter().cloned().collect() }
    fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
    fn get_size(&self) -> PacketNo { self.size }
    fn is_last_packet(&self) -> bool { self.is_last }
    fn _get_wrapped_header(&self) -> &Stack<PacketHeader> { &self.wrapped_header }
}
impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("Sender Msg Seq No {}", *self.sender_msg_seq_no);
        if self.is_last_packet() { s = s + ", Last packet"; }
        else                     { s = s + ", Not last packet"; }
        s = s + &format!(", Size {}", *self.size);
        s = s + &format!(", Wrapped headers: ");
        for w in self.wrapped_header.iter() {
            s = s + &format!("{}", w);
        }
        s = s + &format!("{:?}", &self.bytes[0..10]);
        write!(f, "{}", s)
    }
}
impl Serialize for Payload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer,
    {
        let body = self.bytes.to_hex();
        let mut state = serializer.serialize_struct("Payload", 5)?;
        state.serialize_field("sender_msg_seq_no", &self.sender_msg_seq_no)?;
        state.serialize_field("size", &self.size)?;
        state.serialize_field("is_last", &self.is_last)?;
        state.serialize_field("bytes", &body)?;
        state.end()
    }
}
impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &format!("{:?}", &self.bytes[0..10]);
        write!(f, "{}", s)
    }
}
pub struct Serializer {}
impl Serializer {
    pub fn serialize<M>(msg: &M) -> Result<String, Error>
            where M: Message + serde::Serialize {
        let serialized = serde_json::to_string(msg as &dyn Message).context(PacketError::Chain { func_name: "serialize", comment: S("msg")})?;
        Ok(serialized)
    }
}
pub struct Packetizer {}
impl Packetizer {
    pub fn packetize(uuid: &Uuid, msg: &ByteArray)
            -> Vec<Packet> {
        let msg_bytes = msg.get_bytes();
        let mtu = Packetizer::packet_payload_size(msg_bytes.len());
        let num_packets = (msg_bytes.len() + mtu - 1)/ mtu; // Poor man's ceiling
        let frag = msg_bytes.len() - (num_packets - 1) * mtu;
        let sender_msg_seq_no = SenderMsgSeqNo(rand::random()); // Can't use hash in case two cells send the same message
        let mut packets = Vec::new();
        for i in 0..num_packets {
            let (size, is_last_packet) = if i == (num_packets-1) {
                (frag, true)
            } else {
                (num_packets - i, false)
            };
            // Not a very Rusty way to put bytes into payload
            let mut packet_bytes = vec![PAYLOAD_DEFAULT_ELEMENT; mtu];
            for j in 0..mtu {
                if i*mtu + j == msg_bytes.len() { break; }
                packet_bytes[j] = msg_bytes[i*mtu + j];
            }
            let packet = Packet::new(sender_msg_seq_no, uuid, PacketNo(size as u16),
                                     is_last_packet, packet_bytes);
            //println!("Packet: packet {} for msg {}", packet.get_packet_count(), msg.get_count());
            packets.push(packet);
        }
        packets
    }
    pub fn unpacketize(packets: &Vec<Packet>) -> Result<ByteArray, Error> {
        let mut msg = Vec::new();
        for packet in packets.iter() {
            let mut bytes = packet.get_bytes();
            let frag = *packet.get_size() as usize;
            let is_last_packet = packet.is_last_packet();
            if is_last_packet { bytes.truncate(frag) };
            msg.extend_from_slice(&bytes);
        }
        let msg = std::str::from_utf8(&msg)?;
        Ok(ByteArray::new(msg))
        //Ok(str::from_utf8(&msg).context(PacketError::Chain { func_name: "unpacketize", comment: S("")})?.to_string())
    }
    fn packet_payload_size(len: usize) -> usize {
        match len-1 {
            0..=PACKET_MIN            => PAYLOAD_MIN,
            PAYLOAD_MIN..=PAYLOAD_MAX => len,
            _                         => PAYLOAD_MAX
        }
    }
}
#[derive(Debug, Clone)]
pub struct PacketAssembler {
    sender_msg_seq_no: SenderMsgSeqNo,
    packets: Vec<Packet>,
}
impl PacketAssembler {
    pub fn new(sender_msg_seq_no: SenderMsgSeqNo) -> PacketAssembler {
        PacketAssembler { sender_msg_seq_no, packets: Vec::new() }
    }
//    pub fn create(sender_msg_seq_no: SenderMsgSeqNo, packets: &Vec<Packet>) -> PacketAssembler {
//        PacketAssembler { sender_msg_seq_no, packets: packets.clone() }
//    }
/*
    pub fn get_sender_msg_seq_no(&self) -> SenderMsgSeqNo { self.sender_msg_seq_no }
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
        let is_last = packet.is_last_packet(); // Because I move packet on next line
        self.packets.push(packet);
        (is_last, &self.packets)
    }
}
pub trait ToHex {
    fn to_hex(&self) -> String;
}
impl ToHex for [u8] {
    fn to_hex(&self) -> String {
        format!("{:02x?}", self)
            .split(", ")
            .collect::<Vec<_>>()
            .join("")
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim_end_matches("00").to_string()
    }
}
use serde::ser::{Serialize, SerializeStruct};
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PacketError {
    #[fail(display = "PacketError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
