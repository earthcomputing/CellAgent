use either::Either;
use multi_mut::HashMapMultiMut;
use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          iter::FromIterator,
          marker::{PhantomData},
          //sync::mpsc::channel,
          thread, thread::{JoinHandle}};
use crossbeam::crossbeam_channel::unbounded as channel;

use crate::app_message_formats::{PortFromCa};
use crate::blueprint::{Blueprint, Cell, };
use crate::config::{CONFIG, CellQty, LinkQty};
use crate::dal::{add_to_trace, fork_trace_header, get_cell_replay_lines, update_trace_header};
use crate::ec_message_formats::{PortFromPe};
use crate::link::{Link, DuplexLinkPortChannel, LinkFromPorts, LinkToPorts };
use crate::nalcell::{NalCell};
use crate::name::{CellID, LinkID};
use crate::port::{PortSeed, CommonPortLike, InteriorPortLike, BorderPortLike};
use crate::replay::{process_trace_record, TraceFormat};
use crate::simulated_border_port::{NocFromPort, NocToPort, PortFromNoc, PortToNoc, SimulatedBorderPortFactory, SimulatedBorderPort, DuplexPortNocChannel};
use crate::simulated_internal_port::{LinkFromPort, LinkToPort, PortFromLink, PortToLink, SimulatedInteriorPortFactory, SimulatedInteriorPort, DuplexPortLinkChannel};
use crate::utility::{CellNo, CellConfig, PortNo, Edge, S, TraceHeaderParams, TraceType};

#[derive(Clone, Debug)]
pub struct DuplexLinkEndChannel {
    pub link_to_port: LinkToPort,
    pub link_from_port: LinkFromPort,
}

#[derive(Clone, Debug)]
pub struct DuplexLinkEndChannels {
    pub left: DuplexLinkEndChannel,
    pub rite: DuplexLinkEndChannel,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellInteriorConnection {
    pub cell_no: CellNo,
    pub port_no: PortNo,
}
impl fmt::Display for CellInteriorConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CC: (cell: {}, port: {})", *self.cell_no, *self.port_no)
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct EdgeConnection {
    pub left: CellInteriorConnection,
    pub rite: CellInteriorConnection,
}
impl fmt::Display for EdgeConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EC: (left: {}, rite: {})", self.left, self.rite)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rack {
    cells: HashMap<CellNo, NalCell::<SimulatedInteriorPortFactory, SimulatedInteriorPort, SimulatedBorderPortFactory, SimulatedBorderPort>>,
    links: HashMap<EdgeConnection, Link>,
}
impl Rack {
    pub fn new() -> Rack { Default::default() }
    pub fn initialize(&mut self, blueprint: &Blueprint, duplex_port_noc_channel_cell_port_map: HashMap::<CellNo, HashMap::<PortNo, DuplexPortNocChannel>>) -> Result<Vec<JoinHandle<()>>, Error> {
        let _f = "initialize";
        let num_cells = blueprint.get_ncells();
        let edge_list = blueprint.get_edge_list();
        let mut edge_connection_list = Vec::<EdgeConnection>::new();
        if *num_cells < 1  { return Err(RackError::Cells{ num_cells, func_name: _f }.into()); }
        if edge_list.len() < *num_cells - 1 { return Err(RackError::Edges { nlinks: LinkQty(edge_list.len()), func_name: _f }.into() ); }
        let mut link_handles = Vec::new();
        let mut duplex_port_link_channel_cell_port_map = HashMap::<CellNo, HashMap::<PortNo, DuplexPortLinkChannel>>::new();
        let mut duplex_link_port_channel_cell_port_map = HashMap::<CellNo, HashMap::<PortNo, DuplexLinkPortChannel>>::new();
        let mut dest_cell_port_map = HashMap::<CellNo, HashMap::<PortNo, CellNo>>::new(); // This isn't needed yet, but may be
        let mut duplex_link_end_channel_map = HashMap::<CellInteriorConnection, DuplexLinkEndChannel>::new();
        for edge in edge_list {
            println! ("Connecting edge {}", edge);
            let mut connect_port  = |cell_no, dest_cell_no, side_name| {
                for interior_port_no in match blueprint.get_cell(cell_no).context(RackError::Chain { func_name: _f, comment: S("lookup cell")}) {
                    Ok(cell) => match cell {
                        Either::Left(interior_cell) => Ok(interior_cell.get_interior_ports()),
                        Either::Right(border_cell) => Ok(border_cell.get_interior_ports()),
                    },
                    Err(e) => Err(RackError::Chain { func_name: _f, comment: S("lookup cell")}),
                }? {
                    if !(**interior_port_no == 0) && (!duplex_port_link_channel_cell_port_map.contains_key(&cell_no) || !duplex_port_link_channel_cell_port_map[&cell_no].contains_key(&interior_port_no)) {
                        println! ("Connecting interior port {} for {} of edge {}", interior_port_no, side_name, edge);
                        let (link_to_port, port_from_link): (LinkToPort, PortFromLink) = channel();
                        let (port_to_link, link_from_port): (PortToLink, LinkFromPort) = channel();
                        if duplex_port_link_channel_cell_port_map.contains_key(&cell_no) {
                            duplex_port_link_channel_cell_port_map.get_mut(&cell_no).unwrap().insert(
                                *interior_port_no,
                                DuplexPortLinkChannel {
                                    port_from_link: port_from_link.clone(),
                                    port_to_link: port_to_link.clone(),
                                },
                            );
                            duplex_link_port_channel_cell_port_map.get_mut(&cell_no).unwrap().insert(
                                *interior_port_no,
                                DuplexLinkPortChannel {
                                    link_from_port: link_from_port.clone(),
                                    link_to_port: link_to_port.clone(),
                                },
                            );
                            dest_cell_port_map.get_mut(&cell_no).unwrap().insert(
                                *interior_port_no,
                                dest_cell_no,
                            );
                        } else {
                            let mut duplex_port_link_channel_port_map = HashMap::<PortNo, DuplexPortLinkChannel>::new();
                            duplex_port_link_channel_port_map.insert(
                                *interior_port_no,
                                DuplexPortLinkChannel {
                                    port_from_link: port_from_link.clone(),
                                    port_to_link: port_to_link.clone(),
                                },
                            );
                            duplex_port_link_channel_cell_port_map.insert(
                                cell_no,
                                duplex_port_link_channel_port_map,
                            );
                            let mut duplex_link_port_channel_port_map = HashMap::<PortNo, DuplexLinkPortChannel>::new();
                            duplex_link_port_channel_port_map.insert(
                                *interior_port_no,
                                DuplexLinkPortChannel {
                                    link_from_port: link_from_port.clone(),
                                    link_to_port: link_to_port.clone(),
                                },
                            );
                            duplex_link_port_channel_cell_port_map.insert(
                                cell_no,
                                duplex_link_port_channel_port_map,
                            );
                            let mut dest_port_map = HashMap::<PortNo, CellNo>::new();
                            dest_port_map.insert(
                                *interior_port_no,
                                dest_cell_no,
                            );
                            dest_cell_port_map.insert(
                                cell_no,
                                dest_port_map,
                            );
                        }
                        return Ok(interior_port_no);
                    }
                }
                return Err(RackError::NoPortAvailable { edge: *edge, side_name: side_name, func_name: _f, comment: "no port available for edge", });
            };
            let left_port_no = connect_port(edge.0, edge.1, "left")?;
            let rite_port_no = connect_port(edge.1, edge.0, "rite")?;
            let edge_connection: EdgeConnection = EdgeConnection {
                left: CellInteriorConnection {
                    cell_no: edge.0,
                    port_no: *left_port_no,
                },
                rite: CellInteriorConnection {
                    cell_no: edge.1,
                    port_no: *rite_port_no,
                },
            };
            edge_connection_list.push(edge_connection);
            let left_duplex_link_port_channel_port_map = &duplex_link_port_channel_cell_port_map[&edge.0];
            let left_duplex_link_port_channel = &left_duplex_link_port_channel_port_map[&left_port_no];
            duplex_link_end_channel_map.insert(
                edge_connection.left,
                DuplexLinkEndChannel {
                    link_to_port: left_duplex_link_port_channel.link_to_port.clone(),
                    link_from_port: left_duplex_link_port_channel.link_from_port.clone(),
                },
            );
            let rite_duplex_link_port_channel_port_map = &duplex_link_port_channel_cell_port_map[&edge.1];
            let rite_duplex_link_port_channel = &rite_duplex_link_port_channel_port_map[&rite_port_no];
            duplex_link_end_channel_map.insert(
                edge_connection.rite,
                DuplexLinkEndChannel {
                    link_to_port: rite_duplex_link_port_channel.link_to_port.clone(),
                    link_from_port: rite_duplex_link_port_channel.link_from_port.clone(),
                },
            );
        }
        for border_cell in blueprint.get_border_cells() {
            let cell_no = border_cell.get_cell_no();
            let border_ports = border_cell.get_border_ports();
            println! ("Constructing port factories for border cell {}", cell_no);
            let mut simulated_port_factories = HashMap::<PortNo, Either<SimulatedInteriorPortFactory, SimulatedBorderPortFactory>>::new();
            for border_port_no in border_ports {
                println!("Creating border port factory for port no {} for border cell {}", border_port_no, cell_no);
                simulated_port_factories.insert(*border_port_no, Either::Right(SimulatedBorderPortFactory::new(
                    PortSeed::new(),
                    blueprint.clone(),
                    duplex_port_noc_channel_cell_port_map.clone(),
                    cell_no,
                    *border_port_no,
                    PhantomData,
                )));
            }
            for interior_port_no in border_cell.get_interior_ports() {
                println!("Creating interior port factory for port no {} for border cell {}", interior_port_no, cell_no);
                simulated_port_factories.insert(
                    *interior_port_no,
                    Either::Left(SimulatedInteriorPortFactory::new(
                        PortSeed::new(),
                        blueprint.clone(),
                        duplex_port_link_channel_cell_port_map.clone(),
                        cell_no,
                        *interior_port_no,
                        PhantomData,
                    )),
                );
            }
            let (nal_cell, _join_handle) = match NalCell::<SimulatedInteriorPortFactory, SimulatedInteriorPort, SimulatedBorderPortFactory, SimulatedBorderPort>::new(
                &border_cell.get_name(),
                border_cell.get_num_phys_ports(),
                &HashSet::from_iter(border_ports.clone()),
                CellConfig::Large,
                simulated_port_factories,
            ) {
                Ok(t) => t,
                Err(e) => {
                    println!("Rack: {} error from nalcell {}", _f, e);
                    return Err(RackError::Chain { func_name: _f, comment: S("Border cell") }.into() );
                }
            };
            println!("Created NAL Cell for border cell {}", cell_no);
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc || CONFIG.trace_options.visualize { // Needed for visualization
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "border_cell_start" };
                    let cell_id = nal_cell.get_id();
                    let trace = json!({ "cell_id": cell_id, "cell_number": cell_no,
                                         "border_ports": border_ports, "location":  CONFIG.geometry.get(*cell_no)});
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.cells.insert(cell_no, nal_cell);
        }
        for interior_cell in blueprint.get_interior_cells() {
            let cell_no = interior_cell.get_cell_no();
            println! ("Connecting interior cell {}", cell_no);
            let mut simulated_port_factories = HashMap::<PortNo, Either<SimulatedInteriorPortFactory, SimulatedBorderPortFactory>>::new();
            for interior_port_no in interior_cell.get_interior_ports() {
                println!("Creating interior port factory for port no {} for interior cell {}", interior_port_no, cell_no);
                simulated_port_factories.insert(
                    *interior_port_no,
                    Either::Left(SimulatedInteriorPortFactory::new(
                        PortSeed::new(
                        ),
                        blueprint.clone(),
                        duplex_port_link_channel_cell_port_map.clone(),
                        cell_no,
                        *interior_port_no,
                        PhantomData,
                    )),
                );
            }
            let (nal_cell, _join_handle) = match NalCell::<SimulatedInteriorPortFactory, SimulatedInteriorPort, SimulatedBorderPortFactory, SimulatedBorderPort>::new(
                &interior_cell.get_name(),
                interior_cell.get_num_phys_ports(),
                &HashSet::new(),
                CellConfig::Large,
                simulated_port_factories,
            )
            {
                Ok(t) => t,
                Err(e) => {
                    println!("Rack: {} error from nalcell {}", _f, e);
                    return Err(RackError::Chain { func_name: _f, comment: S("Interior cell") }.into());
                }
            };
            println!("Created NAL Cell for interior cell {}", cell_no);
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc || CONFIG.trace_options.visualize { // Needed for visualization
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "interior_cell_start" };
                    let cell_id = nal_cell.get_id();
                    let trace = json!({ "cell_id": cell_id, "cell_number": cell_no, "location": CONFIG.geometry.get(*cell_no as usize) });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            self.cells.insert(cell_no, nal_cell);
        }
        println!("Created all simulated cells");
        for edge_connection in edge_connection_list {
            println!("Hello edge connection {}", edge_connection);
            let (left_cell, rite_cell) = self.cells
                .get_pair_mut(&edge_connection.left.cell_no, &edge_connection.rite.cell_no)
                .unwrap();
            let left_cell_id: CellID = left_cell.get_id(); // For Trace
            let left_from_pe = left_cell.get_port_from_pe_or_ca(&edge_connection.left.port_no).left().expect("Unexpected border port");
            let left_port = left_cell.get_port(&edge_connection.left.port_no).left().expect("Shold have been an interior port");
            let rite_cell_id: CellID = rite_cell.get_id(); // For Trace
            let rite_from_pe = rite_cell.get_port_from_pe_or_ca(&edge_connection.rite.port_no).left().expect("Unexpected border port");
            let rite_port = rite_cell.get_port(&edge_connection.rite.port_no).left().expect("Shold have been an interior port");
            let mut left_port_clone = left_port.clone();
            left_port_clone.listen_link_and_pe(left_from_pe);
            let mut rite_port_clone = rite_port.clone();
            rite_port_clone.listen_link_and_pe(rite_from_pe); // listening on pe should really happen from NalCell
            println!("Edge {} listening on ports left: {} and right: {}", Edge(edge_connection.left.cell_no, edge_connection.rite.cell_no), left_port_clone.get_port_no(), rite_port_clone.get_port_no());
            let link = Link::new(
                left_port_clone.get_id(),
                rite_port_clone.get_id(),
                LinkToPorts {
                    left: duplex_link_end_channel_map[&edge_connection.left].link_to_port.clone(),
                    rite: duplex_link_end_channel_map[&edge_connection.rite].link_to_port.clone(),
                }
            )?;
            println!("Created link for edge connection {}", edge_connection);
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.dc {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "connect_link" };
                    let trace = json!({ "left_cell": left_cell_id, "rite_cell": rite_cell_id, "left_port": left_port_clone.get_port_no(), "rite_port": rite_port_clone.get_port_no(), "link_id": link.get_id() });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            let mut link_clone = link.clone();
            let child_trace_header = fork_trace_header();
            let thread_name = format!("Link {} thread", link.get_id());
            let link_from_left = duplex_link_end_channel_map[&edge_connection.left].link_from_port.clone();
            let link_from_rite = duplex_link_end_channel_map[&edge_connection.rite].link_from_port.clone();
            let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
                update_trace_header(child_trace_header);
                let _ = link_clone.listen(LinkFromPorts {
                    left: link_from_left,
                    rite: link_from_rite,
                });
            })?;
            //let mut handle_pair = link.start_threads(link_to_left, link_from_left, link_to_rite, link_from_rite)?;
            link_handles.append(&mut vec![join_handle]);
            self.links.insert(edge_connection, link);
        }
        println!("Rack {}: Assigned ports; created and listening on simulated links", _f);
        Ok(link_handles)
    }
    pub fn construct(blueprint: &Blueprint, duplex_port_noc_channel_cell_port_map: HashMap::<CellNo, HashMap::<PortNo, DuplexPortNocChannel>>) -> Result<(Rack, Vec<JoinHandle<()>>), Error> {
        let _f = "construct";
        let mut rack = Rack::new();
        let join_handles = rack.initialize(blueprint, duplex_port_noc_channel_cell_port_map).context(RackError::Chain { func_name: _f, comment: S("initialize")})?;
        Ok((rack, join_handles))
    }
    pub fn get_cells(&self) -> &HashMap<CellNo, NalCell::<SimulatedInteriorPortFactory, SimulatedInteriorPort, SimulatedBorderPortFactory, SimulatedBorderPort>> { &self.cells }
    pub fn get_links_mut(&mut self) -> &mut HashMap<EdgeConnection, Link> { &mut self.links }
    pub fn get_links(&self) -> &HashMap<EdgeConnection, Link> { &self.links }
    pub fn get_cell_ids(&self) -> HashMap<CellNo, CellID> {
        self.cells.iter().map(|cell_no_and_cell| (*cell_no_and_cell.0, cell_no_and_cell.1.get_id())).collect::<HashMap<CellNo, _>>()
    }
    pub fn get_link_ids(&self) -> HashMap<EdgeConnection, LinkID> {
        self.links.iter().map(|edge_connection_and_link| (*edge_connection_and_link.0, edge_connection_and_link.1.get_id())).collect::<HashMap<EdgeConnection,  _>>()
    }
    pub fn select_noc_border_cell(&mut self) -> Result<(CellNo, NalCell::<SimulatedInteriorPortFactory, SimulatedInteriorPort, SimulatedBorderPortFactory, SimulatedBorderPort>), Error> {
        let _f = "select_noc_border_cell";
        return if CONFIG.replay {
            let mut trace_lines = get_cell_replay_lines("Rack").context(RackError::Chain { func_name: _f, comment: S("Rack") })?;
            let record = trace_lines.next().transpose()?.expect(&format!("First record for rack must be there"));
            let trace_format = process_trace_record(record)?;
            match trace_format {
                TraceFormat::BorderCell(cell_no,) => {
                    let cell = self.cells.get_mut(&cell_no)
                        .ok_or::<Error>(RackError::Boundary { func_name: _f }.into())?;
                    Ok((cell_no, (*cell).clone()))
                },
                _ => {
                    unimplemented!()
                }
            }
        } else {
            self.cells
                .iter()
                .find(|(_, nalcell)| nalcell.is_border())
                .map(|(cell_no, cell)| (*cell_no, (*cell).clone()))
                .ok_or::<Error>(RackError::Boundary { func_name: _f }.into())
        }
    }
}
impl fmt::Display for Rack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nLinks\n");
        for (_edge_connection, link) in &self.links {
            write!(s, "  {}\n", link)?;
        }
        s = s + "\nCells";
        for i in 0..self.cells.len() {
            if i < 30 { write!(s, "\n{}\n", self.cells[&CellNo(i)])?; }
        }
        write!(f, "{}", s)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum RackError {
    #[fail(display = "RackError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "RackError::Boundary {}: No boundary cells found", func_name)]
    Boundary { func_name: &'static str },
    #[fail(display = "RackError::Cells {}: The number of cells {:?} must be at least 1", func_name, num_cells)]
    Cells { num_cells: CellQty, func_name: &'static str },
    #[fail(display = "RackError::Edges {}: {:?} is not enough links to connect all cells", func_name, nlinks)]
    Edges { nlinks: LinkQty, func_name: &'static str },
    #[fail(display = "RackError::Wire {}: {:?} is not a valid edge at {}", func_name, edge, comment)]
    Wire { edge: Edge, func_name: &'static str, comment: &'static str },
    #[fail(display = "RackError::NoPortAvailable {}: {:?} No port available for {} side of edge at {}", func_name, side_name, edge, comment)]
    NoPortAvailable { edge: Edge, side_name: &'static str, func_name: &'static str, comment: &'static str },
}
