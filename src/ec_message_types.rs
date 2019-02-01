use std::sync::mpsc;

use crate::config::{ByteArray, PortNo};
use crate::name::{TreeID};
use crate::packet::{Packet};
use crate::packet_engine::NumberOfPackets;
use crate::port::{PortStatus};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::tcp_message_types::{ISAIT, TCP};
use crate::utility::{Mask, PortNumber};
use crate::uuid_ec::Uuid;

type ISBLOCKING = bool;
type CATOCM = (TreeID, ISAIT, Mask, ISBLOCKING, ByteArray);
type REROUTE = (PortNo, PortNo, NumberOfPackets);
pub type PACKET = Packet;
// PacketEngine to PacketEngine to unblock
pub type PeToPePacket = String;
pub type PeToPe = mpsc::Sender<PeToPePacket>;
pub type PeFromPe = mpsc::Receiver<PeToPePacket>;
//pub type PePeError = mpsc::SendError<PeToPePacket>;
// CellAgent to Cmodel (index, tree_uuid, user_mask, direction, is_blocking, bytes)
#[derive(Debug, Clone, Serialize)]
pub enum CaToCmBytes { Entry(RoutingTableEntry), Bytes(CATOCM), Tcp((PortNumber, TCP)),
    Reroute(REROUTE), Unblock }
pub type CaToCm = mpsc::Sender<CaToCmBytes>;
pub type CmFromCa = mpsc::Receiver<CaToCmBytes>;
//pub type CaCmError = mpsc::SendError<CaToCmBytes>;
// Cmodel to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum CmToPePacket { Entry(RoutingTableEntry), Packet((Mask, Packet)), Tcp((PortNumber, TCP)),
    Reroute(REROUTE),  Unblock }
pub type CmToPe = mpsc::Sender<CmToPePacket>;
pub type PeFromCm = mpsc::Receiver<CmToPePacket>;
//pub type CmPeError = mpsc::SendError<CmToPePacket>;
// PacketEngine to Port
#[derive(Debug, Clone, Serialize)]
pub enum PeToPortPacket { Packet((PACKET)), Tcp(TCP) }
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;
//pub type PePortError = mpsc::SendError<PeToPortPacket>;
// PacketEngine to Port, Port to Link
pub type PortToLinkPacket = (PACKET);
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
//pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;
// Link to Port
#[derive(Debug, Clone, Serialize)]
pub enum LinkToPortPacket { Status(PortStatus), Packet((PACKET)) }
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
//pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum PortToPePacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, Packet)), Tcp((PortNo, TCP)) }
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
//pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to Cmodel
#[derive(Debug, Clone, Serialize)]
pub enum PeToCmPacket { Status((PortNo, bool, NumberOfPackets, PortStatus)), Packet((PortNo, Packet)), Tcp((PortNo, TCP)) }
pub type PeToCm = mpsc::Sender<PeToCmPacket>;
pub type CmFromPe = mpsc::Receiver<PeToCmPacket>;
//pub type PeCmError = mpsc::SendError<PeToCmPacket>;
// Cmodel to CellAgent
#[derive(Debug, Clone, Serialize)]
pub enum CmToCaBytes { Status((PortNo, bool, NumberOfPackets, PortStatus)), Bytes((PortNo, bool, Uuid, ByteArray)), Tcp((PortNo, TCP)) }
pub type CmToCa = mpsc::Sender<CmToCaBytes>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytes>;
//pub type CmCaError = mpsc::SendError<CmToCaBytes>;
