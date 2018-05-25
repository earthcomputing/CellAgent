use std::fmt;
use std::collections::HashSet;

use nalcell::CellConfig;
use utility::S;

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct Manifest {
	id: String,
	deployment_tree: AllowedTree,
	cell_config: CellConfig,
	allowed_trees: Vec<AllowedTree>,
	vms: Vec<VmSpec>,
	trees: Vec<UpTreeSpec>
}
impl Manifest {
	pub fn new(id: &str, cell_config: CellConfig, deployment_tree: &AllowedTree, allowed_refs: &Vec<&AllowedTree>,
			vm_refs: Vec<&VmSpec>, tree_refs: Vec<&UpTreeSpec>) -> Result<Manifest, UptreeSpecError> {
		let mut trees = Vec::new();
		for t in tree_refs { trees.push(t.clone()); }
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone().clone()); }
		let mut vms = Vec::new();
		for v in vm_refs {
			vms.push(v.to_owned());
			let allowed = v.get_allowed_trees();
			for tree in allowed {
				if !allowed_trees.contains(tree) { return Err(UptreeSpecError::Allowed { func_name: "new", vm_id: v.get_id().clone(), tree: tree.clone() }.into()); }
			}
		}
		Ok(Manifest { id: S(id), deployment_tree: deployment_tree.clone(), cell_config,
		     allowed_trees, vms, trees })
	}
	pub fn get_id(&self) -> &String { &self.id }
	pub fn get_deployment_tree(&self) -> &AllowedTree { &self.deployment_tree }
	pub fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
	pub fn get_vms(&self) -> &Vec<VmSpec> { &self.vms }
}
impl fmt::Display for Manifest {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
		let s = format!("Deploy {} on tree {}", self.id, self.deployment_tree);
		// Next 4 lines commented out for debugging purposes
		//s = s + &format!("\n  Allowed Trees");
		//for a in &self.allowed_trees { s = s + &format!("\n    {}", a); }
		//for t in &self.trees { s = s + &format!("\n  {}", t); }
		//for v in &self.vms { s = s + &format!("\n  {}", v); }
		write!(f, "{}", s)
	}	
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct VmSpec {
	id: String,
	image:String, 
	required_config: CellConfig,
	allowed_trees: Vec<AllowedTree>,
	containers: Vec<ContainerSpec>,
	trees: Vec<UpTreeSpec>
}
impl VmSpec {
	pub fn new(id: &str, image: &str, config: CellConfig, allowed_refs: &Vec<&AllowedTree>,
			container_refs: Vec<&ContainerSpec>, tree_refs: Vec<&UpTreeSpec>) -> Result<VmSpec, UptreeSpecError> {
		let mut max_tree_size = 0;
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone().clone()); }
		let mut trees = Vec::new();
		for t in tree_refs { 
			trees.push(t.clone());
			if t.get_tree_size() > max_tree_size { max_tree_size = t.get_tree_size() }; 
		}
		if max_tree_size > container_refs.len() { return Err(UptreeSpecError::Containers { func_name: "VmSpec::new", n_containers: container_refs.len() }.into()); }
		let mut containers = Vec::new();
		for c in container_refs {
			containers.push(c.clone());
			let allowed = c.get_allowed_trees();
			for tree in allowed {
				if !allowed_trees.contains(&tree) { return Err(UptreeSpecError::Allowed { func_name: "VmSpec::new", vm_id: c.get_id(), tree: tree.clone() }.into()); }
			}			
		}
		Ok(VmSpec { id: S(id), image: S(image), required_config: config,
				allowed_trees: allowed_trees.clone(), containers, trees })
	}
	pub fn get_id(&self) -> &String { &self.id }
	pub fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
	pub fn get_containers(&self) -> &Vec<ContainerSpec> { &self.containers }
}
impl fmt::Display for VmSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!("  Virtual Machine {}({}, {})", self.id, self.image, self.required_config);
		s = s + &format!("\n      Allowed Trees");
		for a in &self.allowed_trees { s = s + &format!("\n        {}", a); }
		for t in &self.trees { s = s + &format!("\n      {}", t); }
		for c in &self.containers { s = s + &format!("\n     {}", c); }
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContainerSpec {
	id: String, 
	image: String,
	params: Vec<String>,
	allowed_trees: Vec<AllowedTree>
}
impl ContainerSpec {
	pub fn new(id: &str, image: &str, param_refs: Vec<&str>, allowed_refs: &Vec<&AllowedTree>) -> Result<ContainerSpec, UptreeSpecError> {
		let mut params = Vec::new();
		for p in param_refs { params.push(S(p)); } 
		let mut allowed_trees = Vec::new();
		for a in allowed_refs { allowed_trees.push(a.clone().clone()); }
		Ok(ContainerSpec { id: S(id), image: S(image), params, allowed_trees: allowed_trees.clone() })
	}
	pub fn get_id(&self) -> String { self.id.clone() }
	pub fn get_image(&self) -> String { self.image.clone() }
	pub fn get_allowed_trees(&self) -> &Vec<AllowedTree> { &self.allowed_trees }
}
impl fmt::Display for ContainerSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = format!(" Service {}({})", self.id, self.image);
		if self.params.len() == 0 { s = s + &format!(" No parameters"); }
		else                      { s = s + &format!(" Parameters: {:?}", self.params); }
		s = s + &format!("\n        Allowed Trees");
		for a in &self.allowed_trees { s = s + &format!("\n          {}", a); }
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct UpTreeSpec {
	id: String,
	parent_list: Vec<usize>,
}
impl UpTreeSpec {
	pub fn new(id: &str, parent_list: Vec<usize>) -> Result<UpTreeSpec, UptreeSpecError> {
		// Validate parent_list
		if parent_list.len() > 1 {
			let mut count = 0;
			let mut root = 0;
			for i in 0..parent_list.len() { if i == parent_list[i] { root = i; count = count + 1; } }
			if count != 1 { return Err(UptreeSpecError::Tree { func_name: "UptreeSpec::new", id: S(id), parent_list, reason: "More than one root" }.into()); }
			for p in parent_list.clone() {
				//let mut reached_root = true;
				let mut r = p;
				let mut visited = HashSet::new();
				while r != root {
					if visited.contains(&r) { return Err(UptreeSpecError::Tree { func_name: "UptreeSpec::new", id: S(id), parent_list, reason: "Cycle" }.into()); }
					visited.insert(r);
					match parent_list.clone().get(r) {
						Some(p) => {
							r = *p;
							//if r == root { reached_root = true; } else { reached_root = false; }
						},
						None => return Err(UptreeSpecError::Tree{ func_name: "UptreeSpec::new", id: S(id), parent_list, reason: "Index out of range" }.into())
					}
				}
			}
		}
		Ok(UpTreeSpec { id: S(id), parent_list })
	}
	fn get_tree_size(&self) -> usize { self.parent_list.len() }
}
impl fmt::Display for UpTreeSpec {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("UpTree: {}, parent list {:?}", self.id, self.parent_list);
		write!(f, "{}", s)
	}
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AllowedTree {
	name: String,
}
impl AllowedTree {
	pub fn new(name: &str) -> AllowedTree {
		AllowedTree { name: S(name) }
	}
	pub fn get_name(&self) -> &String { &self.name }
}
impl fmt::Display for AllowedTree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let s = format!("{}", self.name);
		write!(f, "{}", s)
	}
}
#[derive(Debug, Fail)]
pub enum UptreeSpecError {
//	#[fail(display = "UptreeSpecError::Chain {} {}", func_name, comment)]
//	Chain { func_name: &'static str, comment: String },
	#[fail(display = "UpTreeSpecError::Allowed {}: tree {} is not in the allowed set for vm {}", func_name, tree, vm_id)]
	Allowed { func_name: &'static str, vm_id: String, tree: AllowedTree },
    #[fail(display = "UpTreeSpecError::Containers {}: {} containers isn't enough for the specified trees", func_name, n_containers)]
    Containers { func_name: &'static str, n_containers: usize },
    #[fail(display = "UpTreeSpecError::Tree {}: {} for parent list {:?} because {}", func_name, id, parent_list, reason)]
    Tree { func_name: &'static str, id: String, parent_list: Vec<usize>, reason: &'static str }
}
