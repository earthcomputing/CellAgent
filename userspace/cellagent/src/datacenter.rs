use actix_web::http::header::OccupiedEntry;
//use std::{sync::mpsc::channel};
use crossbeam::crossbeam_channel as mpsc;
use crossbeam::crossbeam_channel::unbounded as channel;
use std::{collections::{HashMap, hash_map::Entry}, fmt};

use crate::app_message_formats::{ApplicationNocMsg, NocToApplicationMsg};
use crate::blueprint::{Blueprint, Cell};
use crate::config::CONFIG;
use crate::dal::{add_to_trace};
use crate::noc::{DuplexNocPortChannel, DuplexNocApplicationChannel, Noc, NocToApplication, NocFromApplication, NocToPort, NocFromPort};
use crate::rack::{Rack};
use crate::simulated_border_port::{PortFromNoc, PortToNoc, DuplexPortNocChannel};
use crate::utility::{CellNo, PortNo, S, TraceHeaderParams, TraceType};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellBorderConnection {
    cell_no: CellNo,
    port_no: PortNo,
}
impl CellBorderConnection {
    pub fn cell_no(&self) -> CellNo { self.cell_no }
    pub fn port_no(&self) -> PortNo { self.port_no }
}
impl fmt::Display for CellBorderConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(cell: {}, port: {})", *self.cell_no, *self.port_no)
    }
}

pub type ApplicationToNoc = mpsc::Sender<ApplicationNocMsg>;
pub type ApplicationFromNoc = mpsc::Receiver<NocToApplicationMsg>;

#[derive(Clone, Debug)]
pub struct DuplexApplicationNocChannel {
    application_to_noc: ApplicationToNoc,
    application_from_noc: ApplicationFromNoc,
}
impl DuplexApplicationNocChannel {
    pub fn application_to_noc(&self) -> &ApplicationToNoc { &self.application_to_noc }
    pub fn application_from_noc(&self) -> &ApplicationFromNoc { &self.application_from_noc }
}
impl fmt::Display for DuplexApplicationNocChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Send and receive channels to talk to the Noc")
    }
}
#[derive(Debug)]
pub struct Datacenter {
    rack: Rack,
    duplex_application_noc_channel: DuplexApplicationNocChannel,
}
impl Datacenter {
    pub fn construct(blueprint: Blueprint) -> Result<Datacenter, Error> {
        let _f = "construct";
        println!("{}", blueprint);
        {// Reset web server state when restarting datacenter
            { 
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "reset" };
                let trace = json!({ "cell_id": {"name": "Datacenter"}, "blueprint": blueprint, "config": *CONFIG});
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let (application_to_noc, noc_from_application): (ApplicationToNoc, NocFromApplication) = channel();
        let (noc_to_application, application_from_noc): (NocToApplication, ApplicationFromNoc) = channel();
        let mut cell_border_connection_list = Vec::<CellBorderConnection>::new(); // This is not used, but analogous with edge case.
        let mut duplex_noc_port_channel_cell_port_map = HashMap::new();
        let mut duplex_port_noc_channel_cell_port_map = HashMap::new();
        let mut noc_border_port_map = HashMap::<CellNo, PortNo>::new();
        for border_cell in blueprint.get_border_cells() {
            let border_cell_no = border_cell.get_cell_no();
            let noc_port_channels = match duplex_noc_port_channel_cell_port_map
                .entry(border_cell_no) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(HashMap::new()),
                };
            let port_noc_channels = match duplex_port_noc_channel_cell_port_map
                .entry(border_cell_no) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(HashMap::new())
                };
            for &border_port_no in border_cell.get_border_ports() {
                if *border_port_no == 0 {
                    return Err(DatacenterError::BorderPort { func_name: _f, cell_no: border_cell_no }.into())
                }
                println! ("Assigning border cell {} to noc on port {}", border_cell_no, border_port_no);
                let (noc_to_port, port_from_noc): (NocToPort, PortFromNoc) = channel();
                let (port_to_noc, noc_from_port): (PortToNoc, NocFromPort) = channel();
                noc_port_channels.insert(border_port_no,DuplexNocPortChannel { noc_from_port, noc_to_port });
                port_noc_channels.insert(border_port_no, DuplexPortNocChannel{ port_from_noc, port_to_noc });
                noc_border_port_map.insert(border_cell_no, border_port_no);
                cell_border_connection_list.push(CellBorderConnection {
                    cell_no: border_cell_no,
                    port_no: border_port_no,
                });
            }
        }
        let (mut rack, _join_handles) = Rack::construct(&blueprint, duplex_port_noc_channel_cell_port_map).context(DatacenterError::Chain { func_name: _f, comment: S("Rack")})?;
        let (noc_border_cell_no, noc_border_cell) = rack.select_noc_border_cell()?;
        {
            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell" };
            let trace = json!({ "cell_id": {"name": "Rack"}, "cell_no": noc_border_cell_no});
            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
        }
        let noc_border_port_no = noc_border_port_map[&noc_border_cell_no];
        if CONFIG.replay {
            println!("Connecting NOC to border cell {} at port {} for replay", noc_border_cell_no, noc_border_port_no);
        } else {
            println!("Connecting NOC to border cell {} at port {}", noc_border_cell_no, noc_border_port_no);
        }
        noc_border_cell.listen_noc_and_ca(&noc_border_port_no)?; // Returns border cell, but it's not needed
        let mut noc = Noc::new(duplex_noc_port_channel_cell_port_map, 
            DuplexNocApplicationChannel {
                                            noc_to_application,
                                            noc_from_application,
                                        }).context(DatacenterError::Chain { func_name: _f, comment: S("Noc::new")})?;
        noc.initialize(&blueprint).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        println!("NOC created and initialized");
        return Ok(Datacenter {
            rack,
            duplex_application_noc_channel: DuplexApplicationNocChannel {
                application_to_noc,
                application_from_noc,
            },
        });
    }
    pub fn get_rack(&self) -> &Rack { &self.rack }
    pub fn get_rack_mut(&mut self) -> &mut Rack { &mut self.rack }
    pub fn get_application_to_noc(&self) -> &ApplicationToNoc { &self.duplex_application_noc_channel.application_to_noc }
    pub fn get_application_from_noc(&self) -> &ApplicationFromNoc { &self.duplex_application_noc_channel.application_from_noc }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "DatacenterError::BorderPort {} {}", func_name, cell_no)]
    BorderPort { func_name: &'static str, cell_no: CellNo }
}
