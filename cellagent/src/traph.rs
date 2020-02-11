use std::{fmt, fmt::Write,
          collections::{HashMap, HashSet},
          sync::{Arc, Mutex}};

use serde_json;
//use uuid::Uuid;

use crate::config::{PathLength, PortQty};
//use dumpstack::{dumpstack};
use crate::gvm_equation::{GvmEquation, GvmVariable, GvmVariableType};
use crate::name::{Name, CellID, PortTreeID, TreeID};
use crate::port_tree::PortTree;
use crate::routing_table_entry::{RoutingTableEntry};
use crate::traph_element::TraphElement;
use crate::tree::Tree;
use crate::utility::{Path, PortNo, PortNumber, S};
use crate::uuid_ec::Uuid;

type StackedTrees = HashMap<Uuid, Tree>;

#[derive(Debug, Clone)]
pub struct Traph {
    cell_id: CellID, // For debugging
    base_tree_id: TreeID,
    port_tree_id: Option<PortTreeID>,
    port_trees: HashMap<PortTreeID, PortTree>,
    stacked_trees: Arc<Mutex<StackedTrees>>,
    elements: Vec<TraphElement>,
    tried_ports: HashMap<PortTreeID,HashSet<PortNo>>
}
impl Traph {
    pub fn new(cell_id: &CellID, no_ports: PortQty, black_tree_id: TreeID, gvm_eqn: &GvmEquation)
            -> Result<Traph, Error> {
        let mut elements = Vec::new();
        for i in 0..=*no_ports {
            let port_number = PortNo(i as u8).make_port_number(no_ports).context(TraphError::Chain { func_name: "new", comment: S("")})?;
            elements.push(TraphElement::default_for_port(port_number));
        }
        let mut entry = RoutingTableEntry::default();
        entry.add_child(PortNumber::new0());
        let black_port_tree_id = black_tree_id.to_port_tree_id_0();
        let black_tree = Tree::new(black_port_tree_id, black_tree_id,
                                   black_port_tree_id, gvm_eqn, entry);
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
    pub fn delete_tree(&self, delete_tree_id: &TreeID) {
        let _f = "delete_tree";
        let delete_tree_uuid = delete_tree_id.get_uuid();
        let mut locked = self.stacked_trees.lock().unwrap();
        if let Some(tree) = locked.remove(&delete_tree_uuid) {
            // Reset parent for any tree stacked on deleted tree
            let parent_port_tree_id = tree.get_parent_port_tree_id();
            locked
                .iter_mut()
                .for_each(|(_uuid, tree)| {
                    if parent_port_tree_id.to_tree_id().get_uuid() == delete_tree_uuid {
                        tree.set_parent_port_tree_id(parent_port_tree_id);
                    }
                });
        }
        
    }
    pub fn get_port_tree(&self, port_tree_id: PortTreeID) -> Result<&PortTree, Error> {
        let _f = "get_port_tree";
        let port_no = port_tree_id.get_port_no();
        self.port_trees
            .get(&port_tree_id)
            .ok_or(TraphError::PortTree { cell_id: self.cell_id.clone(), func_name: _f, port_no: *port_no }.into())
    }
    pub fn get_base_tree_id(&self) -> TreeID { self.base_tree_id }
    fn get_hops(&self) -> Result<PathLength, Error> {
        self.get_parent_element()
            .map(|element| element.get_hops())
    }
    pub fn add_port_tree(&mut self, port_tree: &PortTree) -> PortTreeID {
        let _f = "add_port_tree";
        if self.port_tree_id.is_none() { self.port_tree_id = Some(port_tree.get_port_tree_id().clone()); }
        self.port_trees.insert(port_tree.get_port_tree_id().clone(), port_tree.clone()); // Duplicate inserts do no harm
        self.port_tree_id.clone().unwrap() // Unwrap is guaranteed to be safe by first line
    }
    pub fn get_elements(&self) -> &Vec<TraphElement> { &self.elements }
    pub fn _set_element(&mut self, traph_element: TraphElement) {
        self.elements[*traph_element.get_port_no() as usize] = traph_element;
    }
    pub fn get_element(&self, port_no: PortNo) -> Result<&TraphElement, Error> {
        let _f = "get_element";
        self.elements
            .get(*port_no as usize)
            .ok_or(TraphError::PortElement { func_name: _f, cell_id: self.cell_id.clone(), port_no: *port_no }.into())
    }
    pub fn get_element_mut (&mut self, port_no: PortNo) -> Result<&mut TraphElement, Error> {
        let _f = "get_element_mut";
        self.elements
            .get_mut(*port_no as usize)
            .ok_or(TraphError::PortElement { func_name: _f, cell_id: self.cell_id, port_no: *port_no }.into())
    }
    pub fn get_port_tree_by_port_number(&self, port_number: &PortNumber) -> PortTree {
        let _f = "get_port_tree";
        let port_no = port_number.get_port_no();
        self.port_trees
            .values()
            .cloned()
            .filter(|port_tree| port_tree.get_in_port_no() == port_no)
            .filter(|port_tree| port_tree.get_entry().get_parent() == port_no)
            .next()
            .expect(&S(TraphError::PortTree { func_name: _f, cell_id: self.cell_id, port_no: *port_no }) )
    }
    pub fn own_port_tree(&mut self, port_tree_id: PortTreeID) -> Option<PortTree> {
        self.port_trees.remove(&port_tree_id)
    }
    pub fn get_port_trees(&self) -> &HashMap<PortTreeID, PortTree> { &self.port_trees }
    pub fn clear_tried_ports(&mut self, rw_port_tree_id: PortTreeID) {
        self.tried_ports.insert(rw_port_tree_id, HashSet::new());
    }
    pub fn add_tried_port(&mut self, rw_port_tree_id: PortTreeID, port_no: PortNo) {
        let _f = "add_tried_port";
        let tried = self.tried_ports
            .entry(rw_port_tree_id)
            .or_insert(HashSet::new());
        tried.insert(port_no);
    }
    fn tried_ports_contains(&self, rw_port_tree_id: PortTreeID, port_no: PortNo) -> bool {
        self.tried_ports.get(&rw_port_tree_id)
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
            .ok_or(TraphError::Tree { func_name: _f, cell_id: self.cell_id, tree_uuid: *tree_uuid }.into())
    }
    pub fn _set_port_tree_entry(&mut self, port_tree_id: PortTreeID, entry: RoutingTableEntry)
            -> Result<(), Error> {
        let _f = "set_port_tree_entry";
        self.port_trees
            .get_mut(&port_tree_id)
            .map(|port_tree| port_tree.set_entry(entry))
            .ok_or(TraphError::Tree { func_name: _f, cell_id: self.cell_id, tree_uuid: port_tree_id.get_uuid() }.into())
    }
    pub fn get_stacked_trees(&self) -> &Arc<Mutex<StackedTrees>> { &self.stacked_trees }
    pub fn _has_tree(&self, tree_id: PortTreeID) -> bool {
        self.stacked_trees.lock().unwrap().contains_key(&tree_id.get_uuid())
    }
    pub fn _is_port_connected(&self, port_number: PortNumber) -> bool {
        self.elements[*port_number.get_port_no() as usize].is_connected()
    }
    pub fn _is_port_broken(&self, port_number: PortNumber) -> bool {
        self.elements[*port_number.get_port_no() as usize].is_broken()
    }
    pub fn set_broken(&mut self, port_number: PortNumber) {
        // Cannont set port status to pruned here because I subsequently use port status to find broken parent links
        self.elements[*port_number.get_port_no() as usize].set_broken();
    }
    pub fn _mark_parent(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_parent();
    }
    pub fn _mark_child(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize]._mark_child();
    }
    pub fn _mark_pruned(&mut self, port_number: PortNumber) {
        self.elements[*port_number.get_port_no() as usize].mark_pruned();
    }
    pub fn mark_broken(&mut self, port_number: PortNumber) {
        let index = *port_number.get_port_no() as usize;
        self.elements[index].set_broken();
        self.elements[index].mark_broken();
    }
    pub fn get_port_status(&self, port_number: PortNumber) -> PortState {
        self.elements[*port_number.get_port_no() as usize].get_state()
    }
    pub fn get_child_elements(&self) -> Vec<PortNo> {
        self.elements
            .iter()
            .filter(|&element| element.get_state() == PortState::Child)
            .map(|element| element.get_port_no())
            .collect()
    }
    pub fn get_parent_port(&self) -> Result<PortNo, Error> {
        self.get_parent_element()
            .map(|element| element.get_port_no())
    }
    pub fn get_parent_element(&self) -> Result<&TraphElement, Error> {
        let _f = "get_parent_element";
        // println!("get_parent_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .find(|&element| element.get_state() == PortState::Parent)
            .ok_or(TraphError::ParentElement { cell_id: self.cell_id, func_name: _f, tree_id: self.base_tree_id }.into())
    }
    fn get_parent_element_mut(&mut self) -> Result<&mut TraphElement, Error> {
        let _f = "get_parent_element_mut";
        // println!("get_parent_element - {}", self);
        // dumpstack();
        self.elements
            .iter_mut()
            .find(|element| element.get_state() == PortState::Parent)
            .ok_or(TraphError::ParentElement { cell_id: self.cell_id, func_name: _f, tree_id: self.base_tree_id }.into())
    }
    pub fn find_new_parent_port(&mut self, rw_port_tree_id: PortTreeID, broken_path: Path) -> Option<PortNo> {
        let _f = "find_new_parent_port";
        // The following 3 lines are useful for debugging
        let p1 = self.get_untried_parent_element(rw_port_tree_id, broken_path);
        let p2 = self.get_untried_pruned_element(rw_port_tree_id, broken_path);
        let p3 = self.get_untried_child_element(rw_port_tree_id);
        vec![p1, p2, p3]
            .into_iter()
            .filter_map(|element| element)
            .min_by_key(|&element| **element.get_hops())
            .cloned()
            .map(|element| {
                let port_no = element.get_port_no();
                self.add_tried_port(rw_port_tree_id, port_no);
                port_no
            })
    }
    fn get_untried_parent_element(&self, rw_port_tree_id: PortTreeID, broken_path: Path) -> Option<&TraphElement> {
        let _f = "get_untried_parent_element";
        // println!("get_untried_parent_element - {}", self);
        // dumpstack();
        self.get_parent_element()
            .ok()
            .filter(|&element| !element.is_on_broken_path(broken_path))
            .filter(|&element| !element.is_broken())
            .filter(|&element| element.is_connected())
            .filter(|&element| !self.tried_ports_contains(rw_port_tree_id, element.get_port_no()))
    }
    fn get_untried_pruned_element(&self, rw_port_tree_id: PortTreeID, broken_path: Path) -> Option<&TraphElement> {
        let _f = "get_untried_pruned_element";
        // println!("get_untried_pruned_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .filter(|&element| element.is_connected())
            .filter(|&element| element.is_state(PortState::Pruned))
            .filter(|&element| !self.tried_ports_contains(rw_port_tree_id, element.get_port_no()))
            .filter(|&element| !element.is_on_broken_path(broken_path))
            .filter(|&element| !element.is_broken())
            .min_by_key(|&element| **element.get_hops())
    }
    fn get_untried_child_element(&self, rw_port_tree_id: PortTreeID) -> Option<&TraphElement> {
        // TODO: Change to pick child with pruned port with shortest path to root
        let _f = "get_untried_child_element";
        // println!("get_untried_child_element - {}", self);
        // dumpstack();
        self.elements
            .iter()
            .filter(|&element| element.is_connected())
            .find(|&element| element.is_state(PortState::Child))
            .filter(|&element| !self.tried_ports_contains(rw_port_tree_id, element.get_port_no()))
            .filter(|&element| !element.is_broken())
    }
    pub fn set_parent(&mut self, new_parent: PortNumber, port_tree_id: PortTreeID)
            -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "set_parent";
        let port_tree_entry = self.port_trees
            .get_mut(&port_tree_id)
            .map(|port_tree| port_tree.set_parent(new_parent))
            .ok_or::<Error>(TraphError::PortTree { func_name: _f, cell_id: self.cell_id.clone(), port_no: *new_parent.get_port_no() }.into())?;
        let parent_element = self.get_parent_element_mut()?;
        if parent_element.get_port_no() != new_parent.get_port_no() {
            let mut entries= self.stacked_trees.lock().unwrap()
                .iter_mut()
                .map(|(_, tree)| { tree.set_parent(new_parent) })
                .collect::<Vec<_>>();
            // Get parent_element again to avoid mutability error; requires NLL
            let parent_element = self.get_parent_element_mut()?;
            parent_element.mark_pruned();
            let new_parent_port_no = new_parent.get_port_no();
            let new_parent_element = self.get_element_mut(new_parent_port_no)?;
            new_parent_element.mark_parent();
            entries.push(port_tree_entry);
            Ok(entries)
        } else {
            Ok(vec![port_tree_entry])
        }
    }
    fn apply_update(&mut self,
                    port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    port_tree_id: PortTreeID,
                    child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "add_or_remove_child";
        let port_tree_entry = self.port_tree_apply_update(port_tree_fn, port_tree_id, child).context(TraphError::Chain { func_name: _f, comment: S("") })?;
        let mut stacked_tree_entries = self.stacked_tree_apply_update(tree_fn, child)?;
        stacked_tree_entries.push(port_tree_entry);
        Ok(stacked_tree_entries)
    }
    fn port_tree_apply_update(&mut self,
                    port_tree_fn: fn(&mut PortTree, PortNumber) -> RoutingTableEntry,
                    port_tree_id: PortTreeID,
                    child: PortNumber) -> Result<RoutingTableEntry, Error> {
        let _f = "port_tree_apply_update";
        self.port_trees  // Table entry for port_tree
            .get_mut(&port_tree_id)
            .map(|port_tree| port_tree_fn(port_tree, child))
            .ok_or::<Error>(TraphError::Tree { func_name: _f, cell_id: self.cell_id.clone(), tree_uuid: port_tree_id.get_uuid() }.into())
    }
    fn stacked_tree_apply_update(&mut self,
                    tree_fn: fn(&mut Tree, PortNumber) -> RoutingTableEntry,
                    child: PortNumber) -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "stacked_tree_apply_update";
        let stacked_tree_entries = self.stacked_trees
            .lock()
            .unwrap()
            .values_mut()
            .map(|stacked_tree| {
                tree_fn(stacked_tree, child)})
            .collect::<Vec<_>>();
        Ok(stacked_tree_entries)
    }
    pub fn change_child(&mut self, port_tree_id: PortTreeID, old_child: PortNumber, new_child: PortNumber)
                        -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "swap_child";
        // The order of the next two statements doesn't matter even though a stacked tree will add the new
        // only if the old child is a child.  That test is handled before this call is made.
        self.add_child(port_tree_id, new_child)?;
        self.remove_child(port_tree_id, old_child)
    }
    pub fn add_child(&mut self, port_tree_id: PortTreeID, child: PortNumber)
            -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "add_child";
        self.apply_update(PortTree::add_child, Tree::add_child, port_tree_id, child)
    }
    fn remove_child(&mut self, port_tree_id: PortTreeID, child: PortNumber)
            -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "remove_child";
        self.apply_update(PortTree::remove_child, Tree::remove_child, port_tree_id, child)
    }
    pub fn _make_child_parent(&mut self, port_tree_id: PortTreeID, child: PortNumber)
            -> Result<Vec<RoutingTableEntry>, Error> {
        let _f = "make_child_parent";
        self.apply_update(PortTree::_make_child_parent, Tree::_make_child_parent,
                          port_tree_id, child)
    }
    pub fn update_element(&mut self, tree_id: TreeID, port_number: PortNumber, port_state: PortState,
                          children: &HashSet<PortNumber>, hops: PathLength, path: Path)
                          -> Result<RoutingTableEntry, Error> {
        let _f = "update_element";
        // println!("update_element - {}", self);
        // dumpstack();
        let port_no = port_number.get_port_no();
        let mut stacked_trees = self.stacked_trees.lock().unwrap();
        let tree = stacked_trees
            .get_mut(&tree_id.get_uuid())
            .ok_or::<Error>(TraphError::Tree { func_name: _f, cell_id: self.cell_id, tree_uuid: tree_id.get_uuid() }.into() )?;
        let mut table_entry = tree.get_table_entry();
        table_entry.set_tree_id(tree_id.to_port_tree_id_0());
        table_entry.add_children(&children);
        table_entry.set_inuse();
        if port_state == PortState::Parent {
            table_entry.set_parent(port_number);
        };
        tree.set_table_entry(table_entry);
        let element = TraphElement::new(true, port_no, port_state, hops, path);
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
    pub fn get_params(&self, vars: &[GvmVariable]) -> Result<Vec<GvmVariable>, Error> {
        let _f = "get_params";
        vars.iter()
            .map(|var| {
                match var.get_var_name().as_ref() {
                    "hops" => {
                        let hops = &self.get_hops().context(TraphError::Chain { func_name: "get_params", comment: S("")})?;
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut uuid = self.base_tree_id.get_uuid().to_string();
        uuid.truncate(8);
        let mut s = format!("Traph {}", self.base_tree_id);
        write!(s, "\n  Stacked Trees")?;
        let locked = self.stacked_trees.lock().unwrap();
        for tree in locked.values() {
            write!(s, "\n  {}", tree)?;
        }
        write!(s, "\n  Port Trees")?;
        for port_tree in &self.port_trees {
            write!(s, "\n  {}", port_tree.1)?;
        }
        write!(s, "\n Port Connected Broken Status Hops Path")?;
        // Can't replace with map() because s gets moved into closure
        for element in self.elements.iter() {
            if element.is_connected() && *element.get_port_no() > 0 {
                write!(s, "\n{}",element)?;
            }
        }
        write!(f, "{}", s)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Serialize)]
pub enum PortState {
    Unknown,
    Parent,
    Child,
    Pruned,
    Broken
}
impl Default for PortState { fn default() -> Self { PortState::Unknown } }
impl fmt::Display for PortState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            PortState::Unknown => "Unknown",
            PortState::Parent  => "Parent",
            PortState::Child   => "Child ",
            PortState::Pruned  => "Pruned",
            PortState::Broken  => "Broken"
        };
        write!(f, "{}", s)
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
    #[fail(display = "TraphError::PortTree {}: No port tree with ID {} on cell {}", func_name, port_no, cell_id)]
    PortTree { func_name: &'static str, cell_id: CellID, port_no: u8 },
    #[fail(display = "TraphError::Tree {}: No tree with UUID {} on cell {}", func_name, tree_uuid, cell_id)]
    Tree { func_name: &'static str, cell_id: CellID, tree_uuid: Uuid }
}
