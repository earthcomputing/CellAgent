//use std::sync::mpsc;
use crossbeam::crossbeam_channel as mpsc;

use crate::app_message::{SenderMsgSeqNo};
use crate::app_message_formats::{ISCONTROL, ISAIT, SNAKE};
use crate::name::{OriginatorID, TreeID};
use crate::packet::{Packet};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus, PortStatusOld};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{ActivityData, ByteArray, Mask, PortNo, OutbufType};
use crate::uuid_ec::Uuid;

type CATOCM = (TreeID, ISCONTROL, ISAIT, SNAKE, Mask, SenderMsgSeqNo, ByteArray);
type REROUTE = (PortNo, PortNo, NumberOfPackets);
type STATUS = (PortNo, bool, PortStatus); // bool = is_border
type STATUSOLD = (PortNo, bool, NumberOfPackets, PortStatusOld); // bool = is_border
type TUNNELPORT = (PortNo, ByteArray);
type TUNNELUP = (OriginatorID, ByteArray);
//pub type PePeError = mpsc::SendError<PeToPePacket>;
// CellAgent to Cmodel (index, tree_uuid, user_mask, direction, bytes)
#[derive(Debug, Clone, Serialize)]
pub enum CaToCmBytes {
    Bytes(CATOCM),
    Delete(Uuid),
    Entry(RoutingTableEntry),
    Reroute(REROUTE),
    Status(STATUSOLD),
    TunnelPort(TUNNELPORT),
    TunnelUp(TUNNELUP),
}
pub type CaToCm = mpsc::Sender<CaToCmBytes>;
pub type CmFromCa = mpsc::Receiver<CaToCmBytes>;
//pub type CaCmError = mpsc::SendError<CaToCmBytes>;
// Cmodel to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum CmToPePacket {
    Delete(Uuid),
    Entry(RoutingTableEntry),
    Packet((Mask, Packet)),
    Reroute(REROUTE),
    SnakeD((PortNo, Packet))
}
pub type CmToPe = mpsc::Sender<CmToPePacket>;
pub type PeFromCm = mpsc::Receiver<CmToPePacket>;
//pub type CmPeError = mpsc::SendError<CmToPePacket>;
// PacketEngine to Port
pub type PeToPortPacketOld = Packet;
pub type PeToPortOld = mpsc::Sender<PeToPortPacketOld>;
pub type PortFromPeOld = mpsc::Receiver<PeToPortPacketOld>;
#[derive(Debug, Clone, Serialize)]
pub enum PeToPortPacket {
    Activity(ActivityData),
    Packet((OutbufType, Packet)),
    Ready
}
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;

// Port to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacket {
    Activity((PortNo, ActivityData)),
    Increment((PortNo, OutbufType)),
    Packet((PortNo, Packet)),
    Status(STATUS)
}
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacketOld {
    Status((PortNo, bool, PortStatusOld)), // bool = is_border
    Packet((PortNo, Packet))
}
pub type PortToPeOld = mpsc::Sender<PortToPePacketOld>;
pub type PeFromPortOld = mpsc::Receiver<PortToPePacketOld>;
//pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to Cmodel
#[derive(Debug, Clone, Serialize)]
pub enum PeToCmPacketOld {
    Status(STATUSOLD),
    Packet((PortNo, Packet)),
    Snake((PortNo, usize, Packet))
}
pub type PeToCm = mpsc::Sender<PeToCmPacketOld>;
pub type CmFromPe = mpsc::Receiver<PeToCmPacketOld>;
//pub type PeCmError = mpsc::SendError<PeToCmPacket>;
// Cmodel to CellAgent
#[derive(Debug, Clone, Serialize)]
pub enum CmToCaBytesOld {
    Status(STATUSOLD),
    Bytes((PortNo, bool, Uuid, ByteArray)),
    TunnelPort(TUNNELPORT),
    TunnelUp(TUNNELUP),
}
pub type CmToCa = mpsc::Sender<CmToCaBytesOld>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytesOld>;
//pub type CmCaError = mpsc::SendError<CmToCaBytes>;
