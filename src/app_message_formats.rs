use std::sync::mpsc;

use crate::app_message::{AppMsgType, AppMsgDirection};
use crate::config::{ByteArray};
use crate::uptree_spec::AllowedTree;

pub type ISAIT = bool;
pub type APP = (ISAIT, AllowedTree, AppMsgType, AppMsgDirection, ByteArray);
// Port to Noc World
pub type PortToNocMsg = APP;
pub type PortToNoc = mpsc::Sender<PortToNocMsg>;
pub type NocFromPort = mpsc::Receiver<PortToNocMsg>;
//pub type PortNocError = mpsc::SendError<PortToNocMsg>;
// Noc to Port
pub type NocToPortMsg = APP;
pub type NocToPort = mpsc::Sender<NocToPortMsg>;
pub type PortFromNoc = mpsc::Receiver<NocToPortMsg>;
//pub type NocPortError = mpsc::SendError<NocToPortMsg>;
// Application to Noc
pub type ApplicationNocMsg = String;
pub type ApplicationToNoc = mpsc::Sender<ApplicationNocMsg>;
pub type NocFromApplication = mpsc::Receiver<ApplicationNocMsg>;
//pub type ApplicationNocError = mpsc::SendError<ApplicationNocMsg>;
// Noc to Application
pub type NocToApplicationMsg = String;
pub type NocToApplication = mpsc::Sender<NocToApplicationMsg>;
pub type ApplicationFromNoc = mpsc::Receiver<NocToApplicationMsg>;
//pub type NocApplicationError = mpsc::SendError<NocToApplicationMsg>;

// Cell agent to VM
pub type CaToVmMsg = (ISAIT, ByteArray);
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
//pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = (ISAIT, AllowedTree, AppMsgType, AppMsgDirection, ByteArray);
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
pub type ContainerToVmMsg = (ISAIT, AllowedTree, AppMsgType, AppMsgDirection, ByteArray);
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
