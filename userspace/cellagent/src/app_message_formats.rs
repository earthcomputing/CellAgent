/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
//use std::sync::mpsc;
use crossbeam::crossbeam_channel as mpsc;

use crate::port::PortStatusOld;
use crate::utility::{ByteArray, PortNo};
pub type SNAKE = bool;
pub type ISAIT = bool;
pub type ISCONTROL = bool;
pub type APP = ByteArray;
// Port to Noc World
pub type NocToPortMsg = APP;
pub type PortToNocMsg = APP;
//pub type PortNocError = mpsc::SendError<PortToNocMsg>;
// Application to Noc
pub type ApplicationNocMsg = String;
//pub type ApplicationNocError = mpsc::SendError<ApplicationNocMsg>;
// Noc to Application
pub type NocToApplicationMsg = String;
//pub type NocApplicationError = mpsc::SendError<NocToApplicationMsg>;
// Boundary Port to Ca
#[derive(Debug, Clone, Serialize)]
pub enum PortToCaMsg { Status(PortNo, PortStatusOld), AppMsg(PortNo, APP) }
pub type PortToCa = mpsc::Sender<PortToCaMsg>;
pub type CaFromPort = mpsc::Receiver<PortToCaMsg>;
//pub type PortCaError = mpsc::SendError<PortToCaMsg>;
// Ca to Boundary Port
pub type CaToPortMsg = APP;
pub type CaToPort = mpsc::Sender<CaToPortMsg>;
pub type PortFromCa = mpsc::Receiver<CaToPortMsg>;
//pub type CaToPortError = mpsc::SendError<CaToPortMsg>;
// Cell agent to VM
pub type CaToVmMsg = APP;
pub type CaToVm = mpsc::Sender<CaToVmMsg>;
pub type VmFromCa = mpsc::Receiver<CaToVmMsg>;
//pub type CaVmError = mpsc::SendError<CaToVmMsg>;
// VM to Cell agent
pub type VmToCaMsg = APP;
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
pub type VmToContainerMsg = APP;
pub type VmToContainer = mpsc::Sender<VmToContainerMsg>;
pub type ContainerFromVm = mpsc::Receiver<VmToContainerMsg>;
//pub type VmContainerError = mpsc::SendError<VmToContainerMsg>;
// Container to VM
pub type ContainerToVmMsg = APP;
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
