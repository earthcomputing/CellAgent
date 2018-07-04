use std::sync::mpsc;
use routing_table_entry::{RoutingTableEntry};

use config::{ByteArray, PortNo, TableIndex};
use message::{MsgDirection, TcpMsgType};
use name::TreeID;
use packet::{Packet};
use port::{PortStatus};
use uptree_spec::AllowedTree;
use utility::{Mask, PortNumber};
use uuid_fake::Uuid;

pub type TCP = (AllowedTree, TcpMsgType, MsgDirection, ByteArray);
pub type CATOCM = (TableIndex, TreeID, Mask, MsgDirection, bool, ByteArray);
// PacketEngine to PacketEngine to unblock
pub type PeToPePacket = String;
pub type PeToPe = mpsc::Sender<PeToPePacket>;
pub type PeFromPe = mpsc::Receiver<PeToPePacket>;
//pub type PePeError = mpsc::SendError<PeToPePacket>;
// CellAgent to Cmodel (index, tree_uuid, user_mask, direction, is_blocking, bytes)
pub enum CaToCmBytes { Entry(RoutingTableEntry), Bytes(CATOCM), Tcp((PortNumber, TCP)),  Unblock }
pub type CaToCm = mpsc::Sender<CaToCmBytes>;
pub type CmFromCa = mpsc::Receiver<CaToCmBytes>;
//pub type CaCmError = mpsc::SendError<CaToCmBytes>;
// Cmodel to PacketEngine
pub enum CmToPePacket { Entry(RoutingTableEntry), Packet((TableIndex, Mask, Packet)), Tcp((PortNumber, TCP)),  Unblock }
pub type CmToPe = mpsc::Sender<CmToPePacket>;
pub type PeFromCm = mpsc::Receiver<CmToPePacket>;
//pub type CmPeError = mpsc::SendError<CmToPePacket>;
// PacketEngine to Port
pub enum PeToPortPacket { Packet((TableIndex, Packet)), Tcp(TCP) }
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;
//pub type PePortError = mpsc::SendError<PeToPortPacket>;
// PacketEngine to Port, Port to Link
pub type PortToLinkPacket = (TableIndex, Packet);
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
//pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;
// Link to Port
pub enum LinkToPortPacket { Status(PortStatus), Packet((TableIndex, Packet)) }
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
//pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
pub enum PortToPePacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)), Tcp((PortNo, TCP)) }
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
//pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to Cmodel
pub enum PeToCmPacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)), Tcp((PortNo, TCP)) }
pub type PeToCm = mpsc::Sender<PeToCmPacket>;
pub type CmFromPe = mpsc::Receiver<PeToCmPacket>;
pub type PeCmError = mpsc::SendError<PeToCmPacket>;
// Cmodel to CellAgent
pub enum CmToCaPacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)), Tcp((PortNo, TCP)) }
pub type CmToCaX = mpsc::Sender<CmToCaPacket>;
pub type CaFromCmX = mpsc::Receiver<CmToCaPacket>;
pub type CmCaErrorX = mpsc::SendError<CmToCaPacket>;
// Cmodel to CellAgent
pub enum CmToCaBytes { Status((PortNo, bool, PortStatus)), Bytes((PortNo, TableIndex, Uuid, ByteArray)), Tcp((PortNo, TCP)) }
pub type CmToCa = mpsc::Sender<CmToCaBytes>;
pub type CaFromCm = mpsc::Receiver<CmToCaBytes>;
pub type CmCaError = mpsc::SendError<CmToCaBytes>;
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
pub type CaToVmMsg = ByteArray;
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
//pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = (AllowedTree, TcpMsgType, MsgDirection, ByteArray);
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
pub type VmToContainerMsg = ByteArray;
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
//pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
// Container to VM
pub type ContainerToVmMsg = (AllowedTree, TcpMsgType, MsgDirection, ByteArray);
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