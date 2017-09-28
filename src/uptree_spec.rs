use std::fmt;
use std::collections::HashSet;

use gvm_equation::GvmEquation;
use utility::S;

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentSpec {
	id: String,
	deployment_tree: String,
	gvm_eqn: GvmEquation,
	allowed_trees: Vec<AllowedTree>,
	vms: Vec<VmSpec>,
	trees: Vec<UpTreeSpec>
}
impl DeploymentSpec {
	pub fn new(id: &str, deployment_tree: &str, allowed_refs: Vec<&AllowedTree>,
			vm_refs: Vec<&VmSpec>, tree_refs: Vec<&UpTreeSpec>, gvm_eqn: &GvmEquation) -> Result<DeploymentSpec> {
		let mut trees = Vec::new();
		for t in tree_refs { trees.push(t.clone()); }
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone()); }
		let mut vms = Vec::new();
		for v in vm_refs {
			vms.push(v.clone());
			let allowed = v.get_allowed_trees();
			for a in allowed { 
				if !allowed_trees.contains(a) { return Err(ErrorKind::Allowed(v.get_id(), S(a)).into()); } 
			}
		}
		Ok( DeploymentSpec { id: S(id), deployment_tree: S(deployment_tree), allowed_trees: allowed_trees, 
				vms: vms, trees: trees, gvm_eqn: gvm_eqn.clone() })
	}
}
impl fmt::Display for DeploymentSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let mut s = format!("\nDeploy {} on tree {}: {}", self.id, self.deployment_tree, self.gvm_eqn);
		s = s + &format!("\n  Allowed Trees");
		for a in &self.allowed_trees { s = s + &format!("\n    {}", a); }
		for t in &self.trees { s = s + &format!("\n  {}", t); }
		for v in &self.vms { s = s + &format!("\n  {}", v); }
		write!(f, "{}", s)
	}	
}
#[derive(Debug, Clone, Deserialize)]
pub struct VmSpec {
	id: String,
	image:String, 
	allowed_trees: Vec<AllowedTree>,
	containers: Vec<ContainerSpec>,
	trees: Vec<UpTreeSpec>
}
impl VmSpec {
	pub fn new(id: &str, image: &str, allowed_refs: Vec<&AllowedTree>, 
			container_refs: Vec<&ContainerSpec>, tree_refs: Vec<&UpTreeSpec>) -> Result<VmSpec> {
		let mut max_tree_size = 0;
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone()); }
		let mut trees = Vec::new();
		for t in tree_refs { 
			trees.push(t.clone());
			if t.get_tree_size() > max_tree_size { max_tree_size = t.get_tree_size() }; 
		}
		if max_tree_size > container_refs.len() { return Err(ErrorKind::Containers(S(id), container_refs.len()).into()); }
		let mut containers = Vec::new();
		for c in container_refs {
			containers.push(c.clone());
			let allowed = c.get_allowed_trees();
			for a in allowed { 
				if !allowed_trees.contains(&a) { return Err(ErrorKind::Allowed(c.get_id(), S(a)).into()); } 
			}			
		}
		Ok(VmSpec { id: S(id), image: S(image), allowed_trees: allowed_trees, containers: containers, trees: trees })
	}
	fn get_id(&self) -> String { self.id.clone() }
	fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
}
impl fmt::Display for VmSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("  Virtual Machine {}({})", self.id, self.image);
		s = s + &format!("\n      Allowed Trees");
		for a in &self.allowed_trees { s = s + &format!("\n        {}", a); }
		for t in &self.trees { s = s + &format!("\n      {}", t); }
		for c in &self.containers { s = s + &format!("\n     {}", c); }
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Deserialize)]
pub struct ContainerSpec {
	id: String, 
	image: String,
	allowed_trees: Vec<AllowedTree>
}
impl ContainerSpec {
	pub fn new(id: &str, image: &str, allowed_refs: Vec<&AllowedTree>) -> Result<ContainerSpec> {
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone()); }
		Ok(ContainerSpec { id: S(id), image: S(image), allowed_trees: allowed_trees })
	}
	fn get_id(&self) -> String { self.id.clone() }
	fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
}
impl fmt::Display for ContainerSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!(" Container {}({})", self.id, self.image);
		s = s + &format!("\n        Allowed Trees");
		for a in &self.allowed_trees { s = s + &format!("\n          {}", a); }
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Deserialize)]
pub struct UpTreeSpec {
	id: String,
	parent_list: Vec<usize>,
	gvm_eqn: GvmEquation
}
impl UpTreeSpec {
	pub fn new(id: &str, parent_list: Vec<usize>, gvm_eqn: &GvmEquation) -> Result<UpTreeSpec> {
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
		Ok(UpTreeSpec { id: S(id), parent_list: parent_list, gvm_eqn: gvm_eqn.clone() })
	}
	fn get_tree_size(&self) -> usize { self.parent_list.len() }
}
impl fmt::Display for UpTreeSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("UpTree: {}, parent list {:?}: {}", self.id, self.parent_list, self.gvm_eqn);
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct AllowedTree {
	id: String,
	gvm_eqn: GvmEquation
}
impl AllowedTree {
	pub fn new(id: &str, gvm_eqn: &GvmEquation) -> AllowedTree {
		AllowedTree { id: S(id), gvm_eqn: gvm_eqn.clone() }
	}
}
impl fmt::Display for AllowedTree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("{}: {}", self.id, self.gvm_eqn);
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