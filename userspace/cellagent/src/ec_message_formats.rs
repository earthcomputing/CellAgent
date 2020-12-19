//use std::sync::mpsc;
use crossbeam::crossbeam_channel as mpsc;

use crate::app_message::{SenderMsgSeqNo};
use crate::app_message_formats::{ISAIT, SNAKE};
use crate::name::{OriginatorID, TreeID};
use crate::packet::{Packet};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{ByteArray, Mask, PortNo};
use crate::uuid_ec::Uuid;

type CATOCM = (TreeID, ISAIT, SNAKE, Mask, SenderMsgSeqNo, ByteArray);
type REROUTE = (PortNo, PortNo, NumberOfPackets);
type STATUS = (PortNo, bool, NumberOfPackets, PortStatus);
type TUNNELPORT = (PortNo, ByteArray);
type TUNNELUP = (OriginatorID, ByteArray);
pub type PACKET = Packet;
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
}
pub type CmToPe = mpsc::Sender<CmToPePacket>;
pub type PeFromCm = mpsc::Receiver<CmToPePacket>;
//pub type CmPeError = mpsc::SendError<CmToPePacket>;
// PacketEngine to Port
pub type PeToPortPacket = PACKET;
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;
//pub type PePortError = mpsc::SendError<PeToPortPacket>;
// PacketEngine to Port, Port to Link
pub type PortToLinkPacket = PACKET;
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
//pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;
// Link to Port
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacket {
    Status(PortStatus),
    Packet(PACKET),
}
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
//pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacket {
    Status((PortNo, bool, PortStatus)),
    Packet((PortNo, Packet))
}
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
//pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to Cmodel
#[derive(Debug, Clone, Serialize)]
pub enum PeToCmPacket {
    Status(STATUS),
    Packet((PortNo, Packet)),
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
