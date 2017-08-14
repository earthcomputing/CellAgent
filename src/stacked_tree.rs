use std::fmt;

use name::TreeID;

#[derive(Debug, Clone)]
pub struct StackedTree {
	tree_id: TreeID,
	base_tree_id: TreeID,
	black_tree_id: TreeID,
	stacked_tree_ids: Vec<TreeID>,
	// Assumes GVM equation evaluated once when stacked tree is created
	recv: bool,
	send: bool,
	save: bool,
}
impl StackedTree {
	pub fn new(tree_id: &TreeID, base_tree_id: &TreeID, black_tree_id: &TreeID,
			recv: bool, send: bool, save: bool) -> StackedTree {
		StackedTree { tree_id: tree_id.clone(), base_tree_id: base_tree_id.clone(),
			black_tree_id: black_tree_id.clone(), recv: recv, send: send, save: save,
			stacked_tree_ids: Vec::new() }
	}
}
impl fmt::Display for StackedTree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Stacked TreeID {}: Base {}, Black {}\n", self.tree_id, self.base_tree_id, self.black_tree_id);
		if self.recv { s = s + &format!("Will receive"); }
		else         { s = s + &format!("Will not receive"); }
		if self.send { s = s + &format!(", Can send"); }
		else         { s = s + &format!(", Cannot send"); }
		if self.save { s = s + &format!(", Saved for late port connects"); }
		else         { s = s + &format!(", Not save for late port connects"); }
		for stacked in &self.stacked_tree_ids {
			s = s + &format!("\n{}", stacked);
		}
		write!(f, "{}", s)
	}	
}
error_chain! {
	
}