//use std::sync::mpsc;
use crossbeam::crossbeam_channel as mpsc;

use crate::app_message::{SenderMsgSeqNo};
use crate::app_message_formats::{ISAIT, SNAKE};
use crate::name::{OriginatorID, TreeID};
use crate::packet::{Packet};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{ActivityData, ByteArray, Mask, PortNo, OutbufType};
use crate::uuid_ec::Uuid;

type CATOCM = (TreeID, ISAIT, SNAKE, Mask, SenderMsgSeqNo, ByteArray);
type REROUTE = (PortNo, PortNo, NumberOfPackets);
type STATUS = (PortNo, bool, NumberOfPackets, PortStatus);
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
    Status(STATUS),
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
pub type PeToPort = mpsc::Sender<PeToPortPacketOld>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacketOld>;
#[derive(Debug, Clone, Serialize)]
pub enum PeToPortPacket {
    Activity(ActivityData),
    Packet((OutbufType, Packet)),
    Ready
}
//pub type PePortError = mpsc::SendError<PeToPortPacket>;
// Port to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacket {
    Activity((PortNo, ActivityData)),
    Increment((PortNo, OutbufType)),
    Packet((PortNo, Packet)),
    Status((PortNo, bool, PortStatus)) // bool = is_border
}
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacketOld {
    Status((PortNo, bool, PortStatus)), // bool = is_border
    Packet((PortNo, Packet))
}
pub type PortToPe = mpsc::Sender<PortToPePacketOld>;
pub type PeFromPort = mpsc::Receiver<PortToPePacketOld>;
//pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to Cmodel
#[derive(Debug, Clone, Serialize)]
pub enum PeToCmPacket {
    Status(STATUS),
    Packet((PortNo, Packet)),
    Snake((PortNo, usize, Packet))
}
pub type PeToCm = mpsc::Sender<PeToCmPacket>;
pub type CmFromPe = mpsc::Receiver<PeToCmPacket>;
//pub type PeCmError = mpsc::SendError<PeToCmPacket>;
// Cmodel to CellAgent
#[derive(Debug, Clone, Serialize)]
pub enum CmToCaBytes {
    Status(STATUS),
    Bytes((PortNo, bool, Uuid, ByteArray)),
    TunnelPort(TUNNELPORT),
    TunnelUp(TUNNELUP),
}
pub type CmToCa = mpsc::Sender<CmToCaBytes>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytes>;
//pub type CmCaError = mpsc::SendError<CmToCaBytes>;
