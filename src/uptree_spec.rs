use std::fmt;
use std::collections::HashSet;

use utility::S;

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentSpec {
	id: String,
	allowed_trees: Vec<String>,
	vms: Vec<VmSpec>,
	trees: Vec<UpTreeSpec>
}
impl DeploymentSpec {
	pub fn new(id: &str, allowed: Vec<&str>, vms: Vec<VmSpec>, trees: Vec<UpTreeSpec>) -> Result<DeploymentSpec> {
		let allowed_trees: Vec<String> = allowed.iter().map(|t| S(t)).collect();
		for v in &vms {
			let allowed = v.get_allowed_trees();
			for a in allowed { 
				if !allowed_trees.contains(a) { return Err(ErrorKind::Allowed(v.get_id(), S(a)).into()); } 
			}
		}
		Ok( DeploymentSpec { id: S(id), allowed_trees: allowed_trees, vms: vms, trees: trees })
	}
}
impl fmt::Display for DeploymentSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nUpTree Definition {}: ", self.id);
		s = s + &format!("\n  Allowed Trees:");
		for a in &self.allowed_trees { s = s + &format!(" {}", a); }
		for v in &self.vms { s = s + &format!("\n  {}", v); }
		for t in &self.trees { s = s + &format!("\n  {}", t); }
		write!(f, "{}", s)
	}	
}
#[derive(Debug, Clone, Deserialize)]
pub struct VmSpec {
	id: String,
	allowed_trees: Vec<String>,
	containers: Vec<String>,
	trees: Vec<UpTreeSpec>
}
impl VmSpec {
	pub fn new(id: &str, allowed_trees: Vec<&str>, containers: Vec<&str>, trees: Vec<UpTreeSpec>) -> Result<VmSpec> {
		let mut max_tree_size = 0;
		for t in trees.iter() { if t.get_tree_size() > max_tree_size { max_tree_size = t.get_tree_size() }; }
		if max_tree_size > containers.len() { return Err(ErrorKind::Containers(S(id), containers.len()).into()); }
		let allowed: Vec<String> = allowed_trees.iter().map(|t| S(t)).collect();
		let cs: Vec<String> = containers.iter().map(|c| S(c)).collect();
		Ok(VmSpec { id: S(id), allowed_trees: allowed, containers: cs, trees: trees })
	}
	fn get_id(&self) -> String { self.id.clone() }
	fn get_allowed_trees(&self) -> &Vec<String> { &self.allowed_trees }
}
impl fmt::Display for VmSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("Virtual Machine {}", self.id);
		s = s + &format!("\n    Allowed Trees:");
		for a in &self.allowed_trees { s = s + &format!(" {}", a); }
		s = s + &format!("\n    Containers:");
		for c in &self.containers { s = s + &format!(" {}", c); }
		for t in &self.trees { s = s + &format!("\n    {}", t); }
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Deserialize)]
pub struct UpTreeSpec {
	id: String,
	parent_list: Vec<usize>
}
impl UpTreeSpec {
	pub fn new(id: &str, parent_list: Vec<usize>) -> Result<UpTreeSpec> {
		// Validate parent_list
		let mut count = 0;
		let mut root = 0;
		for i in 0..parent_list.len() { if i == parent_list[i] { root = i; count = count + 1; } }
		if count != 1 { return Err(ErrorKind::Tree(S(id), parent_list, S("More than one root")).into()); }
		for p in parent_list.clone() {
			let mut reached_root = true;
			let mut r = p;
			let mut visited = HashSet::new();
			while r != root {
				if visited.contains(&r) { return Err(ErrorKind::Tree(S(id), parent_list, S("Cycle")).into()); }
				visited.insert(r);
				match parent_list.clone().get(r) {
					Some(p) => {
						r = *p;
						if r == root { reached_root = true; } else { reached_root = false; }
					},
					None => return Err(ErrorKind::Tree(S(id), parent_list, S("Index out of range")).into())
				}
			}
			if !reached_root { return Err(ErrorKind::Tree(S(id), parent_list, S("No path to root")).into()); }
		}
		Ok(UpTreeSpec { id: S(id), parent_list: parent_list })
	}
	fn get_tree_size(&self) -> usize { self.parent_list.len() }
}
impl fmt::Display for UpTreeSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("UpTree: {}, parent list {:?}", self.id, self.parent_list);
		write!(f, "{}", s)
	}
}
error_chain! {
	errors {
		Allowed(id: String, tree: String) {
			display("UpTreeSpec {}: tree {} is not in the allowed set", id, tree)
		}
		Containers(id: String, n_containers: usize) {
			display("UpTreeSpec {}: {} containers isn't enough for the specified trees", id, n_containers)
		}
		Tree(id: String, parent_list: Vec<usize>, reason: String) {
			display("UpTreeSpec {}: {} for parent list {:?}", id, reason, parent_list) 
		}
	}
}