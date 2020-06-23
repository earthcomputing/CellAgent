use std;
use std::{fmt,
          collections::HashMap,
          env::args,
          fs::{OpenOptions, create_dir, remove_dir_all},
          path::Path,
          ops::{Deref}};

use itertools::Itertools;
use lazy_static::lazy_static;

use crate::utility::{CellConfig, CellNo, Edge, PortNo, Quench, S};

pub type MaskType = u16;
pub const MASK_MAX: u16 = MaskType::max_value();
// System parameters
pub const SCHEMA_VERSION: &str = "0.1";
pub const REPO: &str = "CellAgent";
pub const MAX_CHARS: usize = 32; // Longest valid name
pub const SEPARATOR: & str = "+"; // Separator for compound names
pub const CONTROL_TREE_NAME: & str = "Control";
pub const CONNECTED_PORTS_TREE_NAME: & str = "Connected";
pub const BASE_TREE_NAME: & str = "Base";
pub const PAYLOAD_DEFAULT_ELEMENT: u8 = 0;
pub const PACKET_MIN: usize = 64;   // Can't be in Config because I use it as a const in packet.rs
pub const PACKET_MAX: usize = 9000; // Can't be in Config because I use it as a const in packet.rs

lazy_static!{
    pub static ref CONFIG: Config = Config::new().expect("Error in config file");
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_num_phys_ports_per_cell: PortQty,
    pub min_num_border_cells: CellQty,
    pub quench: Quench,
    pub continue_on_error: bool,
    pub auto_break: Option<Edge>,
    pub output_dir_name: String,
    pub output_file_name: String,
    pub kafka_server: String,
    pub kafka_topic: String,
    pub min_trees: usize, // Minimum number of trees seen before sending mine
    pub min_hello: usize, // Minimum number of Hello messages before sending my discover
    pub breadth_first: bool, // Breadth first or stochastic discover
    pub num_cells: CellQty,
    pub num_ports_per_cell: PortQty,
    pub cell_port_exceptions: HashMap<CellNo, PortQty>,
    pub border_cell_ports: HashMap<CellNo, Vec<PortNo>>,
    pub cell_config: HashMap<CellNo, CellConfig>,
    nrows: usize,
    ncols: usize,
    pub edge_list: Vec<Edge>,
    pub geometry: Vec<(usize, usize)>,
    pub race_sleep: u64,
    pub visualize: Vec<String>,
    pub trace_options: TraceOptions,
    pub debug_options: DebugOptions
}
impl Config {
    pub fn new() -> Result<Config, Error> {
        let _f = "new";
        let config_file_name = args()
            .skip(1)
            .next()
            .unwrap_or(S("configs/10cell_config.json"));
        println!("\nReading configuratation from {}", config_file_name);
        let config_file = OpenOptions::new().read(true).open(config_file_name)?;//.context(ConfigError::File { func_name: _f, file_name: config_file_name})?;
        let mut config: Config = serde_json::from_reader(config_file)?;//.context(ConfigError::Chain { func_name: _f, comment: S("") })?;
        if *config.num_cells == 0 {
            let (nr, nc) = (config.nrows, config.ncols);
            config.num_cells = CellQty(nr*nc);
            config.geometry = (0..nr).cartesian_product(0..nc).collect();
            config.edge_list = Config::make_edges(nr, nc);
            // Select border cells
            config.border_cell_ports = Default::default();
            for cell_no in 0..nc {
                if 1 == cell_no%2 {
                    config.border_cell_ports.insert(CellNo(cell_no), vec![PortNo(2)]);
                }
                if 0 == cell_no%2 {
                    config.border_cell_ports.insert(CellNo(cell_no + nc*(nr-1)), vec![PortNo(1)]);
                }
            }
        }
        // The following must be true for the Trace Visualizer
        for viz in &config.visualize {
            match viz.as_str() {
                "dc"  => config.trace_options.dc  = true,
                "ca"  => config.trace_options.ca  = true,
                "nal" => config.trace_options.nal = true,
                "noc" => config.trace_options.noc = true,
                "svc" => config.trace_options.svc = true,
                "vm"  => config.trace_options.vm  = true,
                "cm"  => config.trace_options.cm  = true,
                "pe"  => config.trace_options.pe  = true,
                _    => eprintln!("---> {} is not a valid visualization designator", viz)
            }
        }
        if Path::new(&config.output_dir_name).exists() {
            remove_dir_all(&config.output_dir_name)?;
        }
        let _ = OpenOptions::new().write(true).truncate(true).open(&config.output_file_name);
        create_dir(&config.output_dir_name)?;
        Ok(config)
    }
    fn make_edges(nr: usize, nc: usize) -> Vec<Edge> {
        (0..nr).fold(vec![], |mut edges, r| {
            let along_row = ((r*nc)..((r+1)* nc-1))
                .zip((r*nc+1)..((r+1)*nc))
                .map(|(r, c)| { Edge(CellNo(r), CellNo(c)) });
            edges.extend_from_slice(&along_row.collect::<Vec<Edge>>());
            if r < (nr-1) {
                let along_col = ((r * nc)..((r + 1) * nc))
                    .zip(((r + 1) * nc)..((r + 2) * nc))
                    .map(|(r, c)| { Edge(CellNo(r), CellNo(c)) });
                edges.extend_from_slice(&along_col.collect::<Vec<Edge>>());
                let diag_rite = ((r * nc)..((r + 1) * nc - 1))
                    .zip(((r + 1) * nc + 1)..((r + 2) * nc))
                    .map(|(r, c)| { Edge(CellNo(r), CellNo(c)) });
                edges.extend_from_slice(&diag_rite.collect::<Vec<Edge>>());
                let diag_left = ((r * nc + 1)..((r + 1) * nc))
                    .zip(((r + 1) * nc)..((r + 2) * nc - 1))
                    .map(|(r, c)| { Edge(CellNo(r), CellNo(c)) });
                edges.extend_from_slice(&diag_left.collect::<Vec<Edge>>());
            }
            edges
        })
    }
}
// TODO: Use log crate for this
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceOptions {
    pub all:      bool,
    pub dc:       bool,
    pub nal:      bool,
    pub noc:      bool,
    pub svc:      bool,
    pub vm:       bool,
    pub ca:       bool,
    pub cm:       bool,
    pub pe:       bool,
    pub pe_cm:    bool,
    pub pe_port:  bool,
    pub port:     bool,
    pub link:     bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugOptions {
    pub all:            bool,
    pub flow_control:   bool,
    pub ca_msg_recv:    bool,
    pub ca_msg_send:    bool,
    pub cm_from_ca:     bool,
    pub cm_to_ca:       bool,
    pub cm_from_pe:     bool,
    pub cm_to_pe:       bool,
    pub application:    bool,
    pub deploy:         bool,
    pub discover:       bool,
    pub discoverd:      bool,
    pub discover_done:  bool,
    pub hello:          bool,
    pub manifest:       bool,
    pub pe_pkt_recv:    bool,
    pub pe_pkt_send:    bool,
    pub process_msg:    bool,
    pub pe_process_pkt: bool,
    pub port:           bool,
    pub saved_discover: bool,
    pub saved_stack:    bool,
    pub stack_tree:     bool,
    pub traph_entry:    bool,
}
// Size of various fields
#[derive(Debug, Copy, Clone, Default, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CellQty(pub usize);
impl Deref for CellQty { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for CellQty { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0)} }
#[derive(Debug, Copy, Clone, Default)]
pub struct ContainerNo(pub usize);
impl Deref for ContainerNo { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Default)]
pub struct DatacenterNo(pub u16);
impl Deref for DatacenterNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Default)]
pub struct LinkQty(pub usize);
impl Deref for LinkQty { type Target = usize; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MaskValue(pub MaskType);
impl Deref for MaskValue { type Target = MaskType; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Default, Serialize)]
pub struct PacketNo(pub u16);
impl Deref for PacketNo { type Target = u16; fn deref(&self) -> &Self::Target { &self.0 } }
#[derive(Debug, Copy, Clone, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PathLength(pub CellQty);
impl Deref for PathLength { type Target = CellQty; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PathLength { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", *self.0)} }
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortQty(pub u8);
impl Deref for PortQty { type Target = u8; fn deref(&self) -> &Self::Target { &self.0 } }
impl fmt::Display for PortQty { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0)} }

// Errors
use failure::{Error, Fail};

#[derive(Debug, Fail)]
pub enum ConfigError {
    #[fail(display = "ConfigError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "ConfigError::Args {}: Must supply a file name for configuration file", func_name)]
    Args { func_name: &'static str },
    #[fail(display = "ConfigError::File {}: Cannot open file {}", func_name, file_name)]
    File { func_name: &'static str, file_name: String },
    #[fail(display = "ConfigError::Quench {} must be one of {:?}", bad, quench)]
    Quench { bad: String, quench: Vec<String>},
}
