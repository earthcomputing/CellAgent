use std::{fmt,
          collections::{HashMap, HashSet},
          slice::Iter,
          sync::{Arc, Mutex}};

use serde_json;
//use uuid::Uuid;

use crate::config::{MAX_PORTS, PathLength, PortNo};
//use dumpstack::{dumpstack};
use crate::gvm_equation::{GvmEquation, GvmVariable, GvmVariableType};
use crate::name::{Name, CellID, TreeID};
use crate::port_tree::PortTree;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::traph_element::TraphElement;
use crate::tree::Tree;
use crate::utility::{Path, PortNumber, S};
use crate::uuid_ec::Uuid;

type StackedTrees = HashMap<Uuid, Tree>;

#[derive(Debug, Clone)]
pub struct Traph {
    cell_id: CellID, // For debugging
    base_tree_id: TreeID,
    port_tree_id: Option<TreeID>,
    port_trees: HashMap<TreeID, PortTree>,
    stacked_trees: Arc<Mutex<StackedTrees>>,
    elements: Vec<TraphElement>,
    tried_ports: HashMap<TreeID,HashSet<PortNo>>
}
impl Traph {
    pub fn new(cell_id: &CellID, no_ports: PortNo, black_tree_id: &TreeID, gvm_eqn: &GvmEquation) -> Result<Traph, Error> {
        let mut elements = Vec::new();
        for i in 1..*no_ports + 1 {
            let port_number = PortNo(i as u8).make_port_number(MAX_PORTS).context(TraphError::Chain { func_name: "new", comment: S("")})?;
            elements.push(TraphElement::default(port_number));
        }
        let entry = RoutingTableEntry::default();
        let black_tree = Tree::new(black_tree_id, black_tree_id, black_tree_id, gvm_eqn, entry);
        let stacked_trees = Arc::new(Mutex::new(HashMap::new()));
        {
            let mut locked = stacked_trees.lock().unwrap();
            locked.insert(black_tree_id.get_uuid(), black_tree);
        }
        let cloth = Traph { cell_id: cell_id.clone(), base_tree_id: black_tree_id.clone(), port_tree_id: None,
                   port_trees: HashMap::new(), stacked_trees, elements, tried_ports: HashMap::new() };
        // println!("new - {}", cloth);
        // dumpstack();
        Ok(cloth)
    }
    pub fn _get_cell_id(&self) -> &CellID { &self.cell_id }
    pub fn get_tree(&self, tree_uuid: &Uuid) -> Result<Tree, Error> {
        let _f = "get_tree";
         self.stacked_trees.lock().unwrap().get(tree_uuid).cloned()
            .ok_or(TraphError::Tree { cell_id: self.cell_id.clone(), func_name: _f, tree_uuid: *tree_uuid }.into())
    }
    pub fn get_base_tree_id(&self) -> &TreeID { &self.base_tree_id }
    pub fn get_hops(&self) -> Result<PathLength, Error> {
        self.get_parent_element()
            .map(|element| element.get_hops())
    }
    pub fn add_port_tree(&mut self, port_tree: PortTree) -> TreeID {
        let _f = "add_port_tree";
        if self.port_tree_id.is_none() { self.port_tree_id = Some(port_tree.get_port_tree_id().clone()); }
        self.port_trees.insert(port_tree.get_port_tree_id().clone(), port_tree); // Duplicate inserts do no harm
        self.port_tree_id.clone().unwrap() // Unwrap is guaranteed to be safe by first line
    }
    pub fn get_elements(&self) -> Iter<TraphElement> { self.elements.iter() }
    pub fn set_element(&mut self, traph_element: TraphElement) {
        self.elements[*traph_element.get_port_no() as usize] = traph_element;
    }
    pub fn get_element(&self, port_no: PortNo) -> Result<&TraphElement, Error> {
        let _f = "get_element";
        self.get_elements()
            .find(|element| element.get_port_no() == port_no)
            .ok_or(TraphError::PortElement { func_name: _f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
    }
    pub fn own_port_tree(&mut self, port_tree_id: &TreeID) -> Option<PortTree> { self.port_trees.remove(port_tree_id) }
    pub fn get_port_trees(&self) -> &HashMap<TreeID, PortTree> { &self.port_trees }
    pub fn clear_tried_ports(&mut self, rw_tree_id: &TreeID) {
        self.tried_ports.insert(rw_tree_id.clone(), HashSet::new());
    }
    pub fn add_tried_port(&mut self, rw_tree_id: &TreeID, port_no: PortNo) {
        let _f = "add_tried_port";
        let mut tried = self.tried_ports
            .get(rw_tree_id)
            .cloned()
            .unwrap_or(HashSet::new());
        tried.insert(port_no);
        self.tried_ports.insert(rw_tree_id.clone(), tried);
    }
    fn tried_ports_contains(&self, rw_tree_id: &TreeID, port_no: PortNo) -> bool {
        self.tried_ports.get(rw_tree_id)
            .unwrap_or(&HashSet::new())
            .contains(&port_no)
    }
    pub fn get_tree_entry(&self, tree_uuid: &Uuid) -> Result<RoutingTableEntry, Error> {
        let _f = "get_tree_entry";
        let tree = self.get_tree(tree_uuid).context(TraphError::Chain { func_name: _f, comment: S("")})?;
        Ok(tree.get_table_entry())
    }
    pub fn set_tree_entry(&mut self, tree_uuid: &Uuid, entry: RoutingTableEntry) -> Result<(), Error> {
        let _f = "set_tree_entry";
        self.stacked_trees.lock().unwrap()
            .get_mut(tree_uuid)
            .map(|tree| tree.set_table_entry(entry))
            .ok_or(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: *tree_uuid }.into())
    }
    pub fn set_port_tree_entry(&mut self, port_tree_id: &TreeID, entry: RoutingTableEntry)
            -> Result<(), Error> {
        let _f = "set_port_tree_entry";
        self.port_trees
            .get_mut(port_tree_id)
            .map(|port_tree| port_tree.set_entry(entry))
            .ok_or(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: port_tree_id.get_uuid() }.into())
    }
    pub fn get_stacked_trees(&self) -> &Arc<Mutex<StackedTrees>> { &self.stacked_trees }
    pub fn has_tree(&self, tree_id: &TreeID) -> bool {
        self.stacked_trees.lock().unwrap().contains_key(&tree_id.get_uuid())
    }
    pub fn is_port_connected(&self, port_number: PortNumber) -> bool {
        self.elements[*port_number.get_port_no() as usize].is_connected()
    }
    pub fn is_port_broken(&self, port_number: PortNumber) -> bool {
        self.elements[*port_number.get_port_no() as usize].is_broken()
    }
    pub fn set_broken(&mut self, port_number: PortNumber) {
        // Cannont set port status to pruned here because I subsequently use port status to find broken parent links
        self.elements[*port_number.get_port_no() as usize].set_broken();
    }
    pub fn mark_parent(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_parent();
    }
    pub fn mark_child(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_child();
    }
    pub fn mark_pruned(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_pruned();
    }
    pub fn mark_broken(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_broken();
    }
    pub fn get_port_status(&self, port_number: PortNumber) -> PortStatus {
        self.elements[*port_number.get_port_no() as usize].get_status()
    }
    pub fn get_parent_port(&self) -> Result<PortNo, Error> {
        self.get_parent_element()
            .map(|element| element.get_port_no())
    }
    pub fn get_parent_element(&self) -> Result<TraphElement, Error> {
        let _f = "get_parent_element";
        // println!("get_parent_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .find(|&element| element.get_status() == PortStatus::Parent)
            .ok_or(TraphError::ParentElement { cell_id: self.cell_id.clone(), func_name: _f, tree_id: self.base_tree_id.clone() }.into())
            .map(|element| element.clone())
    }
    pub fn find_new_parent_port(&mut self, rw_tree_id: &TreeID, broken_path: Path) -> Option<PortNo> {
        let _f = "find_new_parent_port";
        // The following 3 lines are useful for debugging
        let p1 = self.get_untried_parent_element(rw_tree_id, broken_path);
        let p2 = self.get_untried_pruned_element(rw_tree_id, broken_path);
        let p3 = self.get_untried_child_element(rw_tree_id);
        vec![p1, p2, p3]
            .into_iter()
            .filter_map(|element| element)
            .min_by_key(|element| **element.get_hops())
            .map(|element| {
                let port_no = element.get_port_no();
                self.add_tried_port(rw_tree_id, port_no);
                port_no
            })
    }
    pub fn get_untried_parent_element(&self, rw_tree_id: &TreeID, broken_path: Path) -> Option<TraphElement> {
        let _f = "get_untried_parent_element";
        match self.get_parent_element() {
            Err(_) => None,
            Ok(element) => {
                if element.is_on_broken_path(broken_path) ||
                    element.is_broken()        ||
                    !element.is_connected()    ||
                    self.tried_ports_contains(rw_tree_id, element.get_port_no())
                {
                    None
                } else {
                    Some(element)
                }
            }
        }
    }
    pub fn get_untried_pruned_element(&self, rw_tree_id: &TreeID, broken_path: Path) -> Option<TraphElement> {
        let _f = "get_untried_pruned_element";
        // println!("get_untried_pruned_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .filter(|&element| element.is_connected())
            .filter(|&element| element.is_status(PortStatus::Pruned))
            .filter(|&element| !self.tried_ports_contains(rw_tree_id, element.get_port_no()))
            .filter(|&element| !element.is_on_broken_path(broken_path))
            .filter(|&element| !element.is_broken())
            .min_by_key(|&element| **element.get_hops())
            .map(|element| element.clone())
    }
    pub fn get_untried_child_element(&self, rw_tree_id: &TreeID) -> Option<TraphElement> {
        // TODO: Change to pick child with pruned port with shortest path to root
        let _f = "get_untried_child_element";
        // println!("get_untried_child_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .filter(|&element| element.is_connected())
            .find(|&element| element.is_status(PortStatus::Child))
            .filter(|&element| !self.tried_ports_contains(rw_tree_id, element.get_port_no()))
            .filter(|&element| !element.is_broken())
            .map(|element| element.clone())
    }
    fn apply_update(&mut self,
                    port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    port_tree_id: &TreeID,
                    child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "add_or_remove_child";
        let _tree_id = port_tree_id.without_root_port_number();
        let tree_entry = self.tree_apply_update(port_tree_fn, tree_fn, port_tree_id, child).context(TraphError::Chain { func_name: _f, comment: S("") })?;
        let port_tree_entry = self.port_tree_apply_update(port_tree_fn, tree_fn, port_tree_id, child).context(TraphError::Chain { func_name: _f, comment: S("") })?;
        let mut stacked_tree_entries = self.stacked_tree_apply_update(port_tree_fn, tree_fn, port_tree_id, child)?;
        stacked_tree_entries.push(tree_entry);
        stacked_tree_entries.push(port_tree_entry);
        Ok(stacked_tree_entries)
    }
    fn tree_apply_update(&mut self,
                    port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    _tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    port_tree_id: &TreeID,
                    child: PortNumber) -> Result<RoutingTableEntry, Error> {
        let _f = "tree_apply_update";
        let tree_id = port_tree_id.without_root_port_number();
        let tree_entry = self.port_trees  // Table entry for tree
            .get_mut(&tree_id)
            .map(|port_tree| port_tree_fn(port_tree, child))
            .ok_or::<Error>(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: port_tree_id.get_uuid() }.into() )?;
        Ok(tree_entry)
    }
    fn port_tree_apply_update(&mut self,
                    port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    _tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    port_tree_id: &TreeID,
                    child: PortNumber) -> Result<RoutingTableEntry, Error> {
        let _f = "port_tree_apply_update";
        let port_tree_entry = self.port_trees  // Table entry for port_tree
            .get_mut(port_tree_id)
            .map(|port_tree| port_tree_fn(port_tree, child))
            .ok_or::<Error>(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: port_tree_id.get_uuid() }.into() )?;
        Ok(port_tree_entry)
    }
    fn stacked_tree_apply_update(&mut self,
                    _port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    _port_tree_id: &TreeID,
                    child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "stacked_tree_apply_update";
        let stacked_tree_entries = self.stacked_trees.lock().unwrap()
            .values_mut()
            .map(|stacked_tree| tree_fn(stacked_tree, child))
            .collect::<Vec<_>>();
        Ok(stacked_tree_entries)
    }
    pub fn swap_child(&mut self, port_tree_id: &TreeID, old_child: PortNumber, new_child: PortNumber)
            -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "swap_child";
        self.tree_apply_update(PortTree::remove_child, Tree::remove_child, port_tree_id, old_child).context(TraphError::Chain { func_name: _f, comment: S("tree remove") })?;
        let tree_entry = self.tree_apply_update(PortTree::add_child, Tree::add_child, port_tree_id, new_child).context(TraphError::Chain { func_name: _f, comment: S("tree add") })?;
        self.port_tree_apply_update(PortTree::remove_child, Tree::remove_child, port_tree_id, old_child).context(TraphError::Chain { func_name: _f, comment: S("port tree remove") })?;
        let port_tree_entry = self.port_tree_apply_update(PortTree::add_child, Tree::add_child, port_tree_id, new_child).context(TraphError::Chain { func_name: _f, comment: S("port tree add") })?;
        self.stacked_tree_apply_update(PortTree::remove_child, Tree::remove_child, port_tree_id, old_child).context(TraphError::Chain { func_name: _f, comment: S("stack tree remove") })?;
        let mut stacked_tree_entries = self.stacked_tree_apply_update(PortTree::add_child, Tree::add_child, port_tree_id, new_child).context(TraphError::Chain { func_name: _f, comment: S("stack tree add") })?;
        stacked_tree_entries.append(&mut vec![tree_entry, port_tree_entry]);
        Ok(stacked_tree_entries)
    }
    pub fn add_child(&mut self, port_tree_id: &TreeID, child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "add_child";
        self.apply_update(PortTree::add_child, Tree::add_child, port_tree_id, child)
    }
    pub fn remove_child(&mut self, port_tree_id: &TreeID, child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "remove_child";
        self.apply_update(PortTree::remove_child, Tree::remove_child, port_tree_id, child)
    }
    pub fn update_element(&mut self, tree_id: &TreeID, port_number: PortNumber, port_status: PortStatus,
                          children: HashSet<PortNumber>, hops: PathLength, path: Path)
                          -> Result<RoutingTableEntry, Error> {
        let _f = "update_element";
        // println!("update_element - {}", self);
        // dumpstack();
        let port_no = port_number.get_port_no();
        let mut stacked_trees = self.stacked_trees.lock().unwrap();
        let mut tree = stacked_trees
            .get(&tree_id.without_root_port_number().get_uuid())
            .cloned()
            .ok_or::<Error>(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: tree_id.get_uuid() }.into() )?;
        let mut table_entry = tree.get_table_entry();
        table_entry.set_tree_id(tree_id);
        table_entry.add_children(&children);
        table_entry.set_inuse();
        if port_status == PortStatus::Parent {
            table_entry.set_parent(port_number);
        };
        tree.set_table_entry(table_entry);
        stacked_trees.insert(tree_id.get_uuid(), tree);
        let element = TraphElement::new(true, port_no, port_status, hops, path);
        // println!("update_element2 - {}", element);
        self.elements[*port_no as usize] = element; // Cannot fail because self.elements has MAX_PORTS elements
        Ok(table_entry)
    }
    pub fn has_broken_parent(&self) -> bool {
        let _f = "has_broken_parent";
        self.get_parent_element()
            .map(|parent_element| parent_element.is_broken())
            .unwrap_or(false)
    }
    pub fn is_one_hop(&self) -> bool {
        let _f = "is_one_hop";
        self.get_parent_element()
            .map(|parent_element| 1 == **parent_element.get_hops())
            .unwrap_or(false)
    }
    pub fn stack_tree(&mut self, tree: Tree) {
        self.stacked_trees.lock().unwrap().insert(tree.get_uuid(), tree);
    }
    pub fn get_params(&self, vars: &Vec<GvmVariable>) -> Result<Vec<GvmVariable>, Error> {
        let _f = "get_params";
        vars.iter()
            .map(|var| {
                match var.get_var_name().as_ref() {
                    "hops" => {
                        let ref hops = self.get_hops().context(TraphError::Chain { func_name: "get_params", comment: S("")})?;
                        let str_hops = serde_json::to_string(hops).context(TraphError::Chain { func_name: "get_params", comment: S("") })?;
                        let mut updated = GvmVariable::new(GvmVariableType::PathLength, "hops");
                        updated.set_value(str_hops);
                        Ok(updated)
                    },
                    _ => Err(TraphError::Gvm { func_name: _f, var_name: var.get_var_name().clone() }.into())
                }
            })
            .collect()
    }
}
impl fmt::Display for Traph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut uuid = self.base_tree_id.get_uuid().to_string();
        uuid.truncate(8);
        let mut s = format!("Traph {}", self.base_tree_id);
        s = s + &format!("\n  Stacked Trees");
        let locked = self.stacked_trees.lock().unwrap();
        for tree in locked.values() {
            s = s + &format!("\n  {}", tree);
        }
        s = s + &format!("\n  Port Trees");
        for port_tree in &self.port_trees {
            s = s + &format!("\n  {}", port_tree.1);
        }
        s = s + &format!("\n Port Connected Broken Status Hops Path");
        // Can't replace with map() because s gets moved into closure
        for element in self.elements.iter() {
            if element.is_connected() { s = s + &format!("\n{}",element); }
        }
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Serialize)]
pub enum PortStatus {
    Parent,
    Child,
    Pruned,
    Broken
}
impl fmt::Display for PortStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PortStatus::Parent => write!(f, "Parent"),
            PortStatus::Child  => write!(f, "Child "),
            PortStatus::Pruned => write!(f, "Pruned"),
            PortStatus::Broken => write!(f, "Broken")
        }
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum TraphError {
    #[fail(display = "TraphError::Chain {}: {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "TraphError::Gvm {}: var_name {} not implemented", func_name, var_name)]
    Gvm { func_name: &'static str, var_name: String },
    #[fail(display = "TraphError::ParentElement {}: No parent element for tree {} on cell {}", func_name, tree_id, cell_id)]
    ParentElement { func_name: &'static str, cell_id: CellID, tree_id: TreeID },
    #[fail(display = "TraphError::PortElement {}: No element for port {} on cell {}", func_name, port_no, cell_id)]
    PortElement { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "TraphError::Tree {}: No tree with UUID {} on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid }
}
