use std::sync::mpsc;
use routing_table_entry::{RoutingTableEntry};

use config::{PortNo, TableIndex};
use packet::{Packet};
use port::{PortStatus};
use utility::{Mask};

// CellAgent to PacketEngine
pub enum CaToPePacket { Entry(RoutingTableEntry), Packet((TableIndex,Mask,Packet)) }
pub type CaToPe = mpsc::Sender<CaToPePacket>;
pub type PeFromCa = mpsc::Receiver<CaToPePacket>;
pub type CaPeError = mpsc::SendError<CaToPePacket>;
// PacketEngine to Port
pub type PeToPortPacket = ((TableIndex, Packet));
pub type PeToPort = mpsc::Sender<PeToPortPacket>;
pub type PortFromPe = mpsc::Receiver<PeToPortPacket>;
pub type PePortError = mpsc::SendError<PeToPortPacket>;
// PacketEngine to Port, Port to Link
pub type PortToLinkPacket = ((TableIndex, Packet));
pub type PortToLink = mpsc::Sender<PortToLinkPacket>;
pub type LinkFromPort = mpsc::Receiver<PortToLinkPacket>;
pub type PortLinkError = mpsc::SendError<PortToLinkPacket>;
// Link to Port
pub enum LinkToPortPacket { Status(PortStatus),Packet((TableIndex, Packet)) }
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
pub enum PortToPePacket { Status((PortNo, bool, PortStatus)), Packet((PortNo, TableIndex, Packet)) }
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to CellAgent
pub enum PeToCaPacket { Status(PortNo, bool, PortStatus), Packet(PortNo, Packet) }
pub type PeToCa = mpsc::Sender<PeToCaPacket>;
pub type CaFromPe = mpsc::Receiver<PeToCaPacket>;
pub type PeCaError = mpsc::SendError<PeToCaPacket>;
// Port to Noc World
pub type PortToNocMsg = Packet;
pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;
pub type PortNocError = mpsc::SendError<PortToNocMsg>;
// Noc World to Port
pub type NocToPortMsg = Packet;
pub type NocToPort = mpsc::Sender<NocToPortMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;
pub type NocPortError = mpsc::SendError<NocToPortMsg>;
// Outside to Noc
pub type OutsideNocMsg = String;
pub type OutsideToNoc = mpsc::Sender<OutsideNocMsg>;
pub type NocFromOutside = mpsc::Receiver<OutsideNocMsg>;
pub type OutsideNocError = mpsc::SendError<OutsideNocMsg>;
// Cell agent to VM
pub type CaToVmMsg = String;
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = String;
pub type VmToCa = mpsc::Sender<VmToCaMsg>;
pub type CaFromVm = mpsc::Receiver<VmToCaMsg>;
pub type VmCaError = mpsc::SendError<VmToCaMsg>;
// Vm to Container
pub type VmToContainerMsg = String;
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
// Container to VM
pub type ContainerToVmMsg = String;
pub type ContainerToVm = mpsc::Sender<ContainerToVmMsg>;
pub type VmFromContainer = mpsc::Receiver<ContainerToVmMsg>;
pub type ContainerVmError = mpsc::SendError<ContainerToVmMsg>;
