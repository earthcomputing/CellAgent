use std::fmt;
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

use config::{CellNo, CellType, Edge, PortNo};

#[derive(Debug)]
pub struct Blueprint {
	interior_cells: Vec<InteriorCell>,
	border_cells: Vec<BorderCell>,
	edges: Vec<Edge>,
}
impl Blueprint {
	pub fn new(ncells: CellNo, ports_per_cell: PortNo, edges: Vec<Edge>,
			exceptions: HashMap<CellNo, PortNo>, border_cell_map: HashMap<CellNo, Vec<PortNo>>) -> Result<Blueprint> {
		if border_cell_map.len() > *ncells { return Err(ErrorKind::CellCount(ncells, border_cell_map.len()).into()) }
		let mut interior_cells = Vec::new();
		let mut border_cells = 	Vec::new();
		for no in 0..*ncells {
			let cell_no = CellNo(no);
			let nports = match exceptions.get(&cell_no) {
				Some(p) => *p,
				None => ports_per_cell
			};
			let port_list = (0..*nports as usize).map(|i| PortNo{v:i as u8}).collect();
			match border_cell_map.get(&cell_no) {
				Some(ports) => {
					let border: HashSet<PortNo> = HashSet::from_iter(ports.clone());
					let all: HashSet<PortNo> = HashSet::from_iter(port_list);
					let mut interior = all.difference(&border).cloned().collect::<Vec<_>>();
					interior.sort();
					border_cells.push(BorderCell { cell_no: cell_no, interior_ports: interior, border_ports: ports.clone() });					
				},
				None => interior_cells.push(InteriorCell { cell_no: cell_no, interior_ports : port_list })
			}
		}
		Ok(Blueprint { interior_cells: interior_cells, border_cells: border_cells, edges: edges })
	}
	pub fn get_ncells(&self) -> CellNo { CellNo(self.get_n_interior_cells() + self.get_n_border_cells()) }
	pub fn get_n_border_cells(&self) -> usize { self.border_cells.len() }
	pub fn get_n_interior_cells(&self) -> usize { self.interior_cells.len() }
	pub fn get_edge_list(&self) -> &Vec<Edge> { &self.edges }
	pub fn get_border_cells(&self) -> &Vec<BorderCell> { &self.border_cells }
	pub fn get_interior_cells(&self) -> &Vec<InteriorCell> { &self.interior_cells }
}
impl fmt::Display for Blueprint {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nBlueprint");
		for cell in self.border_cells.iter() { s = s + &format!("{}", cell); }
		for cell in self.interior_cells.iter() { s = s + &format!("{}", cell); }
		s = s + &format!("\n  Edges: ");
		for edge in self.edges.iter() { s = s + &format!("({},{})", *edge.v.0, *edge.v.1); }
		write!(f, "{}", s) }
}
#[derive(Debug, Clone)]
pub struct BorderCell {
	cell_no: CellNo, 
	interior_ports: Vec<PortNo>,
	border_ports: Vec<PortNo>,
}
impl BorderCell {
	pub fn get_cell_no(&self) -> CellNo { self.cell_no }
	pub fn get_nports(&self) -> PortNo { PortNo{ v: (self.border_ports.len() + self.interior_ports.len()) as u8} }
	pub fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
	pub fn get_border_ports(&self) -> &Vec<PortNo> { &self.border_ports }
}
impl fmt::Display for BorderCell {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\n  Border Cell {}: ", *self.cell_no);
		s = s + &format!("Border Ports:");
		for p in self.border_ports.iter() { s = s + &format!(" {}", p.v); }
		s = s + &format!(", Interior Ports:");
		for p in self.interior_ports.iter() { s = s + &format!(" {}", p.v); }
		write!(f, "{}", s)
	}	
}
#[derive(Debug, Clone)]
pub struct InteriorCell {
	cell_no: CellNo,
	interior_ports: Vec<PortNo>
}
impl InteriorCell {
	pub fn get_cell_no(&self) -> CellNo { self.cell_no }
	pub fn get_nports(&self) -> PortNo { PortNo { v: self.interior_ports.len() as u8} }
	pub fn get_interior_ports(&self) -> &Vec<PortNo> { &self.interior_ports }
}
impl fmt::Display for InteriorCell {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\n  Interior Cell {}: ", *self.cell_no);
		s = s + &format!("Interior Ports:");
		for p in self.interior_ports.iter() { s = s + &format!(" {}", p.v); }
		write!(f, "{}", s)
	}	
}
error_chain! {
	errors {
		CellCount(total: CellNo, border_count: usize) {
			display("Invalid blueprint has more border cells {} than total cells {}", border_count, **total)
		}
	}
}