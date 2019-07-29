use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          iter::FromIterator,
};

use crate::config::{CONFIG, CellQty, PortQty};
use crate::utility::{CellNo, CellType, Edge, PortNo};

#[derive(Debug)]
pub struct Blueprint {
    interior_cells: Vec<InteriorCell>,
    border_cells: Vec<BorderCell>,
    edges: Vec<Edge>,
}
impl Blueprint {
    pub fn new(num_cells: CellQty, edges: &Vec<Edge>, default_num_phys_ports_per_cell: PortQty,
               cell_port_exceptions: &HashMap<CellNo, PortQty>, border_cell_ports: &HashMap<CellNo, Vec<PortNo>>) ->
               Result<Blueprint, BlueprintError> {
        let _f = "new";
        for edge in edges {
            if *edge.0 > *num_cells {
                return Err(BlueprintError::EdgeEndpoint{
                    func_name: _f,
                    num_cells: *num_cells,
                    invalid_endpoint: *edge.0,
                });
            }
            if *edge.1 > *num_cells {
                return Err(BlueprintError::EdgeEndpoint{
                    func_name: _f,
                    num_cells: *num_cells,
                    invalid_endpoint: *edge.1,
                });
            }
        }
        let mut cell_num_phys_ports: HashMap<CellNo, PortQty> = HashMap::new();
        if *default_num_phys_ports_per_cell > *CONFIG.max_num_phys_ports_per_cell {
            return Err(BlueprintError::DefaultNumPhysPortsPerCell {
                func_name: _f,
                default_num_phys_ports_per_cell: *default_num_phys_ports_per_cell,
                max_num_phys_ports_per_cell: *CONFIG.max_num_phys_ports_per_cell,
            }.into());
        }
        for (cell_no, num_phys_ports) in cell_port_exceptions {
            if **cell_no >= *num_cells {
                return Err(BlueprintError::CellPortsExceptionsCell {
                    func_name: _f,
                    cell_no: **cell_no,
                    num_cells: *num_cells,
                }.into());
            }
            if **num_phys_ports > *CONFIG.max_num_phys_ports_per_cell {
                return Err(BlueprintError::CellPortsExceptionsPorts {
                    func_name: _f,
                    cell_no: **cell_no,
                    num_phys_ports: **num_phys_ports,
                    max_num_phys_ports_per_cell: *CONFIG.max_num_phys_ports_per_cell,
                }.into());
            }
        }
        for no in 0..*num_cells {
            let cell_no = CellNo(no);
            cell_num_phys_ports.insert(cell_no,
                                       *cell_port_exceptions
                                       .get(&cell_no)
                                       .unwrap_or(&default_num_phys_ports_per_cell));
        }
        for (cell_no, port_nos) in border_cell_ports {
            if **cell_no >= *num_cells {
                return Err(BlueprintError::BorderCellPortsCell {
                    func_name: _f,
                    cell_no: **cell_no,
                    num_cells: *num_cells,
                }.into());
            }
            for port_no in port_nos {
                let num_phys_ports: PortQty = cell_num_phys_ports[cell_no];
                if **port_no >= *num_phys_ports {
                    return Err(BlueprintError::BorderCellPortsPort {
                        func_name: _f,
                        cell_no: **cell_no,
                        port_no: **port_no,
                        num_phys_ports: *num_phys_ports,
                    }.into());
                }
            }
        }
        let num_border = border_cell_ports.len();
        if num_border < *CONFIG.min_num_border_cells {
            return Err(BlueprintError::BorderCellCount { func_name: _f, num_border, num_reqd: *CONFIG.min_num_border_cells})
        }
        let mut interior_cells = Vec::new();
        let mut border_cells = 	Vec::new();
        for no in 0..*num_cells {
            let cell_no = CellNo(no);
            let phys_port_list : Vec<PortNo> = (0..*cell_num_phys_ports[&cell_no] as usize).map(|i| PortNo(i as u8)).collect();
            match border_cell_ports.get(&cell_no) {
                Some(border_ports) => {
                    let border: HashSet<PortNo> = HashSet::from_iter(border_ports.clone());
                    let all: HashSet<PortNo> = HashSet::from_iter(phys_port_list);
                    let mut interior_ports = all.difference(&border).cloned().collect::<Vec<_>>();
                    interior_ports.sort();
                    border_cells.push(BorderCell { cell_no, cell_type: CellType::Border, interior_ports, border_ports: border_ports.clone() });
                },
                None => interior_cells.push(InteriorCell { cell_no, cell_type: CellType::Interior, interior_ports : phys_port_list })
            }
        }
        Ok(Blueprint { interior_cells, border_cells, edges:edges.clone() })
    }
    pub fn get_ncells(&self) -> CellQty { CellQty(self.get_n_interior_cells() + self.get_n_border_cells()) }
    pub fn get_n_border_cells(&self) -> usize { self.border_cells.len() }
    pub fn get_n_interior_cells(&self) -> usize { self.interior_cells.len() }
    pub fn get_edge_list(&self) -> &Vec<Edge> { &self.edges }
    pub fn get_border_cells(&self) -> &Vec<BorderCell> { &self.border_cells }
    pub fn get_interior_cells(&self) -> &Vec<InteriorCell> { &self.interior_cells }
}
impl fmt::Display for Blueprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\nBlueprint");
        for cell in self.border_cells.iter() { s = s + &format!("{}", cell); }
        for cell in self.interior_cells.iter() { s = s + &format!("{}", cell); }
        s = s + &format!("\n  Edges: ");
        for edge in self.edges.iter() { s = s + &format!("({},{})", *(edge.0), *(edge.1)); }
        write!(f, "{}", s) }
}
pub trait Cell {
    fn get_cell_no(&self) -> CellNo;
    fn get_name(&self) -> String {
        return format!("{}", self.get_cell_no());
    }
    fn get_cell_type(&self) -> CellType;
    fn get_num_phys_ports(&self) -> PortQty;
    fn get_interior_ports(&self) -> &Vec<PortNo>;
}
#[derive(Debug, Clone)]
pub struct BorderCell {
    cell_no: CellNo,
    cell_type: CellType,
    interior_ports: Vec<PortNo>,
    border_ports: Vec<PortNo>,
}
impl Cell for BorderCell {
    fn get_cell_no(&self) -> CellNo { self.cell_no }
    fn get_cell_type(&self) -> CellType { self.cell_type }
    fn get_num_phys_ports(&self) -> PortQty { PortQty((self.border_ports.len() + self.interior_ports.len()) as u8) }
    fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
}
impl BorderCell {
    pub fn _new(cell_no: CellNo, cell_type: CellType, interior_ports: Vec<PortNo>, border_ports: Vec<PortNo>) -> BorderCell {
	    BorderCell { cell_no, cell_type, interior_ports, border_ports }
    }
    pub fn get_border_ports(&self) -> &Vec<PortNo> { &self.border_ports }
}
impl fmt::Display for BorderCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\n  Border Cell {}: ", *self.cell_no);
        s = s + &format!("Border Ports:");
        for p in self.border_ports.iter().cloned() { s = s + &format!(" {}", *p); }
        s = s + &format!(", Interior Ports:");
        for p in self.interior_ports.iter().cloned() { s = s + &format!(" {}", *p); }
        write!(f, "{}", s)
    }
}
#[derive(Debug, Clone)]
pub struct InteriorCell {
    cell_no: CellNo,
    cell_type: CellType,
    interior_ports: Vec<PortNo>
}
impl Cell for InteriorCell {
    fn get_cell_no(&self) -> CellNo { self.cell_no }
    fn get_cell_type(&self) -> CellType { self.cell_type }
    fn get_num_phys_ports(&self) -> PortQty { PortQty(self.interior_ports.len() as u8) }
    fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
}
impl InteriorCell {
    pub fn _new(cell_no: CellNo, cell_type: CellType, interior_ports: Vec<PortNo>) -> InteriorCell {
        InteriorCell { cell_no, cell_type, interior_ports }
    }
}
impl fmt::Display for InteriorCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\n  Interior Cell {}: ", *self.cell_no);
        write!(s, "Interior Ports:")?;
        for p in self.interior_ports.iter().cloned() { write!(s, " {}", *p)?; }
        write!(f, "{}", s)
    }
}

// Errors
#[derive(Debug, Fail)]
pub enum BlueprintError {
    #[fail(display = "BlueprintError::EdgeEndpoint {}: Cell reference {} in edges should be less than total number of cells {}", func_name, num_cells, invalid_endpoint)]
    EdgeEndpoint { func_name: &'static str, num_cells: usize, invalid_endpoint: usize},
    #[fail(display = "BlueprintError::DefaultNumPhysPortsPerCell {}:  Default number of physical ports per cell {} is greater than the maximum allowed {}", func_name, default_num_phys_ports_per_cell, max_num_phys_ports_per_cell)]
    DefaultNumPhysPortsPerCell { func_name: &'static str, default_num_phys_ports_per_cell: u8, max_num_phys_ports_per_cell: u8},
    #[fail(display = "BlueprintError::CellPortsExceptionsCell {}:  Can't create ports exception for invalid cell {}; number of cells is {}", func_name, cell_no, num_cells)]
    CellPortsExceptionsCell { func_name: &'static str, cell_no: usize, num_cells: usize},
    #[fail(display = "BlueprintError::CellPortsExceptionsPorts {}:  Ports exception {} is greater than maximum number allowed {} for cell {}", func_name, num_phys_ports, max_num_phys_ports_per_cell, cell_no)]
    CellPortsExceptionsPorts { func_name: &'static str, num_phys_ports: u8, max_num_phys_ports_per_cell: u8, cell_no: usize},
    #[fail(display = "BlueprintError::BorderCellPortsCell {}:  Border ports requested for cell {}; number of cells is {}", func_name, cell_no, num_cells)]
    BorderCellPortsCell { func_name: &'static str, cell_no: usize, num_cells: usize},
    #[fail(display = "BlueprintError::BorderCellPortsPort {}:  Border port {} requested for cell {}; number of ports is {}", func_name, port_no, cell_no, num_phys_ports)]
    BorderCellPortsPort { func_name: &'static str, port_no: u8, cell_no: usize, num_phys_ports: u8},
    #[fail(display = "BlueprintError::BorderCellCount {}: Must have {} border cells but only {} supplied", func_name, num_reqd, num_border)]
    BorderCellCount { func_name: &'static str, num_border: usize, num_reqd: usize},
}
