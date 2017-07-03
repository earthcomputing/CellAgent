use std::sync::mpsc;
use routing_table_entry::{RoutingTableEntry};
use config::{Json, PortNo, TableIndex};
use packet::{Packet};
use port::{PortStatus};
use utility::{Mask};
// CellAgent to PacketEngine
pub enum CaToPeMsg { Entry(RoutingTableEntry), Msg((TableIndex,Mask,Packet)) }
pub type CaToPe = mpsc::Sender<CaToPeMsg>;
pub type PeFromCa = mpsc::Receiver<CaToPeMsg>;
pub type CaPeError = mpsc::SendError<CaToPeMsg>;
// PacketEngine to Port
pub type PeToPort = mpsc::Sender<Packet>;
pub type PortFromPe = mpsc::Receiver<Packet>;
pub type PePortError = mpsc::SendError<Packet>;
// PacketEngine to Port, Port to Link
pub type PortToLink = mpsc::Sender<Packet>;
pub type LinkFromPort = mpsc::Receiver<Packet>;
pub type PortLinkError = mpsc::SendError<Packet>;
// Link to Port
pub enum LinkToPortMsg { Status(PortStatus),Msg(Packet) }
pub type LinkToPort = mpsc::Sender<LinkToPortMsg>;
pub type PortFromLink = mpsc::Receiver<LinkToPortMsg>;
pub type LinkPortError = mpsc::SendError<LinkToPortMsg>;
// Port to PacketEngine
pub enum PortToPeMsg { Status((PortNo, PortStatus)), Msg((PortNo, Packet)) }
pub type PortToPe = mpsc::Sender<PortToPeMsg>;
pub type PeFromPort = mpsc::Receiver<PortToPeMsg>;
pub type PortPeError = mpsc::SendError<PortToPeMsg>;
// PacketEngine to CellAgent
pub enum PeToCaMsg { Status(PortNo, PortStatus), Msg(PortNo, TableIndex, Packet) }
pub type PeToCa = mpsc::Sender<PeToCaMsg>;
pub type CaFromPe = mpsc::Receiver<PeToCaMsg>;
pub type PeCaError = mpsc::SendError<PeToCaMsg>;
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
