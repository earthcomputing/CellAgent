//use std::sync::mpsc;
use crossbeam::crossbeam_channel as mpsc;

use crate::app_message::{SenderMsgSeqNo};
use crate::app_message_formats::{ISCONTROL, ISAIT, SNAKE};
use crate::name::{OriginatorID, TreeID};
use crate::packet::{Packet};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::utility::{ByteArray, Mask, PortNo};
#[cfg(feature = "api-new")]
use crate::utility::{ActivityData, OutbufType};
use crate::uuid_ec::Uuid;

type CATOCM = (TreeID, ISCONTROL, ISAIT, SNAKE, Mask, SenderMsgSeqNo, ByteArray);
#[cfg(feature = "api-old")]
type REROUTE = (PortNo, PortNo, NumberOfPackets);
#[cfg(feature = "api-new")]
type REROUTE = (PortNo, PortNo);
#[cfg(feature = "api-old")]
pub type STATUS = (PortNo, bool, PortStatus, NumberOfPackets); // bool = is_border
#[cfg(feature = "api-new")]
type STATUS = (PortNo, bool, PortStatus); // bool = is_border
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
#[derive(Debug, Clone, Serialize)]
#[cfg(feature = "api-old")]
pub enum PeToPortPacket {
    Packet(Packet),
}
#[cfg(feature = "api-new")]
pub enum PeToPortPacket {
    Packet((Outbuf, Packet)),
    Activity(ActivityData),
    Ready
}
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;

// Port to PacketEngine
#[derive(Debug, Clone, Serialize)]
#[cfg(feature = "api-new")]
pub enum PortToPePacket {
    Packet((PortNo, Packet)),
    Status(STATUS),
    Activity((PortNo, ActivityData)),
    Increment((PortNo, OutbufType)),
}
#[cfg(feature = "api-old")]
pub enum PortToPePacket {
    Packet((PortNo, Packet)),
    Status(STATUS)
}
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
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
#[derive(Debug, Clone, Serialize)]
pub enum PeToCmPacketOld {
    Status(STATUS),
    Packet((PortNo, Packet)),
    Snake((PortNo, usize, Packet))
}
pub type PeToCmOld = mpsc::Sender<PeToCmPacketOld>;
pub type CmFromPeOld = mpsc::Receiver<PeToCmPacketOld>;
//pub type PeCmError = mpsc::SendError<PeToCmPacket>;
#[derive(Debug, Clone, Serialize)]
pub enum CmToCaBytes {
    Status(STATUS),
    Bytes((PortNo, bool, Uuid, ByteArray)),
    TunnelPort(TUNNELPORT),
    TunnelUp(TUNNELUP),
}
pub type CmToCa = mpsc::Sender<CmToCaBytes>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytes>;
