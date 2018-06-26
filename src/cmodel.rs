use std::fmt;
use std::thread::JoinHandle;
use message_types::{CaToCmPacket, CmToCa, CmFromCa, CmToPe, CmFromPe, PeToCmPacket, CmToCaPacket, CmToPePacket};
use name::{Name, CellID};
use utility::{TraceHeader, write_err};

#[derive(Debug, Clone)]
pub struct Cmodel {
    cell_id: CellID,
}
impl Cmodel {
    pub fn new(cell_id: &CellID) -> Cmodel {
        Cmodel { cell_id: cell_id.clone() }
    }
    pub fn initialize(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe, cm_from_pe: CmFromPe, cm_to_ca: CmToCa,
                      mut trace_header: TraceHeader) -> Result<(), Error> {
        self.listen_ca(cm_from_ca, cm_to_pe, &mut trace_header)?;
        self.listen_pe(cm_from_pe, cm_to_ca, &mut trace_header)?;
        Ok(())
    }
    fn listen_ca(&self, cm_from_ca: CmFromCa, cm_to_pe: CmToPe, outer_trace_header: &mut TraceHeader) -> Result<JoinHandle<()>, Error> {
        let cmodel = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        let join_handle = ::std::thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = cmodel.listen_ca_loop(&cm_from_ca, &cm_to_pe, inner_trace_header).map_err(|e| write_err("cmodel listen_ca", e.into()));
            //let _ = cmodel.listen_ca(cm_from_ca, cm_to_pe);
        });
        Ok(join_handle)
    }
    fn listen_pe(&self, cm_from_pe: CmFromPe, cm_to_ca: CmToCa, outer_trace_header: &mut TraceHeader) -> Result<JoinHandle<()>, Error> {
        let cmodel = self.clone();
        let mut outer_trace_header_clone = outer_trace_header.clone();
        let join_handle = ::std::thread::spawn( move || {
            let ref mut inner_trace_header = outer_trace_header_clone.fork_trace();
            let _ = cmodel.listen_pe_loop(&cm_from_pe, &cm_to_ca, inner_trace_header).map_err(|e| write_err("cmodel listen_pe", e.into()));;
            //let _ = cmodel.listen_pe(cm_from_pe, cm_to_ca);
        });
        Ok(join_handle)
    }
    fn listen_ca_loop(&self, cm_from_ca: &CmFromCa, cm_to_pe: &CmToPe, trace_header: &mut TraceHeader) -> Result<(), Error> {
        loop {
            match cm_from_ca.recv()? {
                CaToCmPacket::Entry(entry) => cm_to_pe.send(CmToPePacket::Entry(entry)),
                CaToCmPacket::Packet((index, user_mask, packet)) => cm_to_pe.send(CmToPePacket::Packet((index, user_mask, packet))),
                CaToCmPacket::Tcp((port_number, msg)) => cm_to_pe.send(CmToPePacket::Tcp((port_number, msg))),
                CaToCmPacket::Unblock => cm_to_pe.send(CmToPePacket::Unblock)
            }?;
        }
     }
    fn listen_pe_loop(&self, cm_from_pe: &CmFromPe, cm_to_ca: &CmToCa, trace_header: &mut TraceHeader) -> Result<(), Error> {
        loop {
            match cm_from_pe.recv()? {
                PeToCmPacket::Status((port_no,bool, PortStatus)) => cm_to_ca.send(CmToCaPacket::Status((port_no,bool, PortStatus))),
                PeToCmPacket::Packet((port_no, TableIndex, Packet)) => cm_to_ca.send(CmToCaPacket::Packet((port_no, TableIndex, Packet))),
                PeToCmPacket::Tcp((port_no, tcp_msg)) => cm_to_ca.send(CmToCaPacket::Tcp((port_no, tcp_msg)))
            }?;
        }
    }
}
impl fmt::Display for Cmodel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = format!("\nCmodel {}", self.cell_id.get_name());
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error};
#[derive(Debug, Fail)]
pub enum CmodelError {
    #[fail(display = "NameError::Chain {} {}", func_name, comment)]
	Chain { func_name: &'static str, comment: String },
}
