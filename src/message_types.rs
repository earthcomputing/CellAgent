use std::sync::mpsc;
use routing_table_entry::{RoutingTableEntry};
use config::{Json, PortNo, TableIndex};
use packet::{Packet};
use port::{PortStatus};
use utility::{Mask};
// CellAgent to PacketEngine
pub enum CaToPePacket { Entry(RoutingTableEntry), Packet((TableIndex,Mask,Packet)) }
pub type CaToPe = mpsc::Sender<CaToPePacket>;
pub type PeFromCa = mpsc::Receiver<CaToPePacket>;
pub type CaPeError = mpsc::SendError<CaToPePacket>;
// PacketEngine to Port
pub type PeToPort = mpsc::Sender<Packet>;
pub type PortFromPe = mpsc::Receiver<Packet>;
pub type PePortError = mpsc::SendError<Packet>;
// PacketEngine to Port, Port to Link
pub type PortToLink = mpsc::Sender<Packet>;
pub type LinkFromPort = mpsc::Receiver<Packet>;
pub type PortLinkError = mpsc::SendError<Packet>;
// Link to Port
pub enum LinkToPortPacket { Status(PortStatus),Packet(Packet) }
pub type LinkToPort = mpsc::Sender<LinkToPortPacket>;
pub type PortFromLink = mpsc::Receiver<LinkToPortPacket>;
pub type LinkPortError = mpsc::SendError<LinkToPortPacket>;
// Port to PacketEngine
pub enum PortToPePacket { Status((PortNo, PortStatus)), Packet((PortNo, Packet)) }
pub type PortToPe = mpsc::Sender<PortToPePacket>;
pub type PeFromPort = mpsc::Receiver<PortToPePacket>;
pub type PortPeError = mpsc::SendError<PortToPePacket>;
// PacketEngine to CellAgent
pub enum PeToCaPacket { Status(PortNo, PortStatus), Packet(PortNo, TableIndex, Packet) }
pub type PeToCa = mpsc::Sender<PeToCaPacket>;
pub type CaFromPe = mpsc::Receiver<PeToCaPacket>;
pub type PeCaError = mpsc::SendError<PeToCaPacket>;
// Port to Outside World
pub type PortToOutsideMsg = Json;
pub type PortToOutside = mpsc::Sender<PortToOutsideMsg>;
pub type OutsideFromPort = mpsc::Receiver<PortToOutsideMsg>;
pub type PortOutsideError = mpsc::SendError<PortToOutsideMsg>;
// Outside World to Port
pub type OutsideToPortMsg = Json;
pub type OutsideToPort = mpsc::Sender<OutsideToPortMsg>;
pub type PortFromOutside = mpsc::Receiver<OutsideToPortMsg>;
pub type OutsidePortError = mpsc::SendError<OutsideToPortMsg>;
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
