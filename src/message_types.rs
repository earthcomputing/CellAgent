use std::sync::mpsc;

use crate::config::{ByteArray, PortNo};
use crate::message::{MsgDirection, TcpMsgType};
use crate::name::TreeID;
use crate::packet::{Packet};
use crate::port::{PortStatus};
use crate::routing_table_entry::{RoutingTableEntry};
use crate::uptree_spec::AllowedTree;
use crate::utility::{Mask, PortNumber};
use crate::uuid_ec::Uuid;

type ISAIT = bool;
type ISBLOCKING = bool;
type CATOCM = (TreeID, ISAIT, Mask, ISBLOCKING, ByteArray);
pub type PACKET = Packet;
pub type TCP = (ISAIT, AllowedTree, TcpMsgType, MsgDirection, ByteArray);
// PacketEngine to PacketEngine to unblock
pub type PeToPePacket = String;
pub type PeToPe = mpsc::Sender<PeToPePacket>;
pub type PeFromPe = mpsc::Receiver<PeToPePacket>;
//pub type PePeError = mpsc::SendError<PeToPePacket>;
// CellAgent to Cmodel (index, tree_uuid, user_mask, direction, is_blocking, bytes)
#[derive(Debug, Clone, Serialize)]
pub enum CaToCmBytes { Entry(RoutingTableEntry), Bytes(CATOCM), Tcp((PortNumber, TCP)),  Unblock }
pub type CaToCm = mpsc::Sender<CaToCmBytes>;
pub type CmFromCa = mpsc::Receiver<CaToCmBytes>;
//pub type CaCmError = mpsc::SendError<CaToCmBytes>;
// Cmodel to PacketEngine
#[derive(Debug, Clone, Serialize)]
pub enum CmToPePacket { Entry(RoutingTableEntry), Packet((Mask, Packet)), Tcp((PortNumber, TCP)),  Unblock }
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
pub enum PeToCmPacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, Packet)), Tcp((PortNo, TCP)) }
pub type PeToCm = mpsc::Sender<PeToCmPacket>;
pub type CmFromPe = mpsc::Receiver<PeToCmPacket>;
//pub type PeCmError = mpsc::SendError<PeToCmPacket>;
// Cmodel to CellAgent
#[derive(Debug, Clone, Serialize)]
pub enum CmToCaBytes { Status((PortNo, bool, PortStatus)), Bytes((PortNo, bool, Uuid, ByteArray)), Tcp((PortNo, TCP)) }
pub type CmToCa = mpsc::Sender<CmToCaBytes>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytes>;
//pub type CmCaError = mpsc::SendError<CmToCaBytes>;
// Port to Noc World
pub type PortToNocMsg = TCP;
pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;
//pub type PortNocError = mpsc::SendError<PortToNocMsg>;
// Noc to Port
pub type NocToPortMsg = TCP;
pub type NocToPort = mpsc::Sender<NocToPortMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;
//pub type NocPortError = mpsc::SendError<NocToPortMsg>;
// Outside to Noc
pub type OutsideNocMsg = String;
pub type OutsideToNoc = mpsc::Sender<OutsideNocMsg>;
pub type NocFromOutside = mpsc::Receiver<OutsideNocMsg>;
//pub type OutsideNocError = mpsc::SendError<OutsideNocMsg>;
// Noc to Outside
pub type NocToOutsideMsg = String;
pub type NocToOutside = mpsc::Sender<NocToOutsideMsg>;
pub type OutsideFromNoc = mpsc::Receiver<NocToOutsideMsg>;
//pub type NocOutsideError = mpsc::SendError<NocToOutsideMsg>;
// Cell agent to VM
pub type CaToVmMsg = (ISAIT, ByteArray);
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
//pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = (ISAIT, AllowedTree, TcpMsgType, MsgDirection, ByteArray);
pub type VmToCa = mpsc::Sender<VmToCaMsg>;
pub type CaFromVm = mpsc::Receiver<VmToCaMsg>;
//pub type VmCaError = mpsc::SendError<VmToCaMsg>;
// Vm to Tree
//pub type VmToTreeMsg = String;
//pub type VmToTree = mpsc::Sender<VmToTreeMsg>;
//pub type TreeFromVm = mpsc::Receiver<VmToTreeMsg>;
//pub type VmTreeError = mpsc::SendError<VmToTreeMsg>;
// Tree to Vm
//pub type TreeToVmMsg = String;
//pub type TreeToVm = mpsc::Sender<TreeToVmMsg>;
//pub type VmFromTree = mpsc::Receiver<TreeToVmMsg>;
//pub type TreeVmError = mpsc::SendError<TreeToVmMsg>;
// Vm to Container
pub type VmToContainerMsg = (ISAIT, ByteArray);
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
//pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
// Container to VM
pub type ContainerToVmMsg = (ISAIT, AllowedTree, TcpMsgType, MsgDirection, ByteArray);
pub type ContainerToVm = mpsc::Sender<ContainerToVmMsg>;
pub type VmFromContainer = mpsc::Receiver<ContainerToVmMsg>;
//pub type ContainerVmError = mpsc::SendError<ContainerToVmMsg>;
// Container to Tree
//pub type ContainerToTreeMsg = String;
//pub type ContainerToTree = mpsc::Sender<ContainerToTreeMsg>;
//pub type TreeFromContainer = mpsc::Receiver<ContainerToTreeMsg>;
//pub type ContainerTreeError = mpsc::SendError<ContainerToTreeMsg>;
// Tree to Container
//pub type TreeToContainerMsg = String;
//pub type TreeToContainer = mpsc::Sender<TreeToContainerMsg>;
//pub type ContainerFromTree = mpsc::Receiver<TreeToContainerMsg>;
//pub type TreeContainerError = mpsc::SendError<TreeToContainerMsg>;
