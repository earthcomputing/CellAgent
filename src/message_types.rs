use std::sync::mpsc;
use routing_table_entry::{RoutingTableEntry};

use config::{PortNo, TableIndex};
use message::{MsgDirection, TcpMsgType};
use packet::{Packet};
use port::{PortStatus};
use uptree_spec::AllowedTree;
use utility::{Mask, PortNumber};

pub type TCP = (AllowedTree, TcpMsgType, MsgDirection, String);
// CellAgent to PacketEngine
pub enum CaToPePacket { Entry(RoutingTableEntry), Packet((TableIndex, Mask, Packet)), Tcp((PortNumber, TCP)) }
pub type CaToPe = mpsc::Sender<CaToPePacket>;
pub type PeFromCa = mpsc::Receiver<CaToPePacket>;
pub type CaPeError = mpsc::SendError<CaToPePacket>;
// PacketEngine to Port
pub enum PeToPortPacket { Packet((TableIndex, Packet)), Tcp(TCP) }
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;
pub type PePortError = mpsc::SendError<PeToPortPacket>;
// PacketEngine to Port, Port to Link
pub type PortToLinkPacket = (TableIndex, Packet);
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;
// Link to Port
pub enum LinkToPortPacket { Status(PortStatus), Packet((TableIndex, Packet)) }
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
pub enum PortToPePacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)), Tcp((PortNo, TCP)) }
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to CellAgent
pub enum PeToCaPacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)), Tcp((PortNo, TCP)) }
pub type PeToCa = mpsc::Sender<PeToCaPacket>;
pub type CaFromPe = mpsc::Receiver<PeToCaPacket>;
pub type PeCaError = mpsc::SendError<PeToCaPacket>;
// Port to Noc World
pub type PortToNocMsg = TCP;
pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;
pub type PortNocError = mpsc::SendError<PortToNocMsg>;
// Noc to Port
pub type NocToPortMsg = TCP;
pub type NocToPort = mpsc::Sender<NocToPortMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;
pub type NocPortError = mpsc::SendError<NocToPortMsg>;
// Outside to Noc
pub type OutsideNocMsg = String;
pub type OutsideToNoc = mpsc::Sender<OutsideNocMsg>;
pub type NocFromOutside = mpsc::Receiver<OutsideNocMsg>;
pub type OutsideNocError = mpsc::SendError<OutsideNocMsg>;
// Noc to Outside
pub type NocToOutsideMsg = String;
pub type NocToOutside = mpsc::Sender<NocToOutsideMsg>;
pub type OutsideFromNoc = mpsc::Receiver<NocToOutsideMsg>;
pub type NocOutsideError = mpsc::SendError<NocToOutsideMsg>;
// Cell agent to VM
pub type CaToVmMsg = String;
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = (AllowedTree, TcpMsgType, MsgDirection, String);
pub type VmToCa = mpsc::Sender<VmToCaMsg>;
pub type CaFromVm = mpsc::Receiver<VmToCaMsg>;
pub type VmCaError = mpsc::SendError<VmToCaMsg>;
// Vm to Tree
pub type VmToTreeMsg = String;
pub type VmToTree = mpsc::Sender<VmToTreeMsg>;
pub type TreeFromVm = mpsc::Receiver<VmToTreeMsg>;
pub type VmTreeError = mpsc::SendError<VmToTreeMsg>;
// Tree to Vm
pub type TreeToVmMsg = String;
pub type TreeToVm = mpsc::Sender<TreeToVmMsg>;
pub type VmFromTree = mpsc::Receiver<TreeToVmMsg>;
pub type TreeVmError = mpsc::SendError<TreeToVmMsg>;
// Vm to Container
pub type VmToContainerMsg = String;
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
// Container to VM
pub type ContainerToVmMsg = (AllowedTree, TcpMsgType, MsgDirection, String);
pub type ContainerToVm = mpsc::Sender<ContainerToVmMsg>;
pub type VmFromContainer = mpsc::Receiver<ContainerToVmMsg>;
pub type ContainerVmError = mpsc::SendError<ContainerToVmMsg>;
// Container to Tree
pub type ContainerToTreeMsg = String;
pub type ContainerToTree = mpsc::Sender<ContainerToTreeMsg>;
pub type TreeFromContainer = mpsc::Receiver<ContainerToTreeMsg>;
pub type ContainerTreeError = mpsc::SendError<ContainerToTreeMsg>;
// Tree to Container
pub type TreeToContainerMsg = String;
pub type TreeToContainer = mpsc::Sender<TreeToContainerMsg>;
pub type ContainerFromTree = mpsc::Receiver<TreeToContainerMsg>;
pub type TreeContainerError = mpsc::SendError<TreeToContainerMsg>;