use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          iter::FromIterator};

use crate::config::{MIN_BORDER_CELLS, CellNo, CellQty, CellType, Edge, PortNo, PortQty};

#[derive(Debug)]
pub struct Blueprint {
    interior_cells: Vec<InteriorCell>,
    border_cells: Vec<BorderCell>,
    edges: Vec<Edge>,
}
impl Blueprint {
    pub fn new(ncells: CellQty, ports_per_cell: PortQty, edges: &Vec<Edge>,
               exceptions: &HashMap<CellNo, PortQty>, border_cell_map: &HashMap<CellNo, Vec<PortNo>>) ->
               Result<Blueprint, BlueprintError> {
        let _f = "new";
        let num_border = border_cell_map.len();
        if num_border > *ncells {
            return Err(BlueprintError::CellCount{ func_name: _f, ncells: *ncells, num_border })
        };
        if num_border < *MIN_BORDER_CELLS {
            return Err(BlueprintError::BorderCellCount { func_name: _f, num_border, num_reqd: *MIN_BORDER_CELLS})
        }
        let mut interior_cells = Vec::new();
        let mut border_cells = 	Vec::new();
        for no in 0..*ncells {
            let cell_no = CellNo(no);
            let nports = *exceptions
                .get(&cell_no)
                .unwrap_or(&ports_per_cell);
            let port_list = (0..*nports as usize).map(|i| PortNo(i as u8)).collect();
            match border_cell_map.get(&cell_no) {
                Some(ports) => {
                    let border: HashSet<PortNo> = HashSet::from_iter(ports.clone());
                    let all: HashSet<PortNo> = HashSet::from_iter(port_list);
                    let mut interior_ports = all.difference(&border).cloned().collect::<Vec<_>>();
                    interior_ports.sort();
                    border_cells.push(BorderCell { cell_no, cell_type: CellType::Border, interior_ports, border_ports: ports.clone() });
                },
                None => interior_cells.push(InteriorCell { cell_no, cell_type: CellType::Interior, interior_ports : port_list })
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
    fn get_cell_type(&self) -> CellType;
    fn get_nports(&self) -> PortQty;
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
    fn get_nports(&self) -> PortQty { PortQty((self.border_ports.len() + self.interior_ports.len()) as u8) }
    fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
}
impl BorderCell {
    pub fn _get_border_ports(&self) -> &Vec<PortNo> { &self.border_ports }
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
    fn get_nports(&self) -> PortQty { PortQty(self.interior_ports.len() as u8) }
    fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
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
    #[fail(display = "BlueprintError::BorderCellCount {}: Must have {} border cells but only {} in blueprint", func_name, num_border, num_reqd)]
    BorderCellCount { func_name: &'static str, num_border: usize, num_reqd: usize},
    #[fail(display = "BlueprintError::CellCount {}: Invalid blueprint has more border cells {} than total cells {}", func_name, ncells, num_border)]
    CellCount { func_name: &'static str, ncells: usize, num_border: usize}
}
