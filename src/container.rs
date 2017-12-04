use std::fmt;
use std::collections::HashMap;

use failure::Error;

use message_types::{ContainerToVm, ContainerFromVm};
use name::{ContainerID, TreeID, UptreeID};
use service::{NocAgent, NocMaster, Service};
use uptree_spec::AllowedTree;

#[derive(Debug, Clone)]
pub struct Container {
	id: ContainerID,
    allowed_trees: Vec<AllowedTree>,
    container_to_vm: ContainerToVm,
	service: Service,
}
impl Container {
	pub fn new(id: ContainerID, allowed_trees: &Vec<AllowedTree>, container_to_vm: ContainerToVm, service_name: &str,
            ) -> Result<Container, Error> {
        let service = Service::create_service( &id, service_name)?;
        println!("{}", service);
 		Ok(Container { id: id, allowed_trees: allowed_trees.clone(), container_to_vm: container_to_vm, service: service })
	}
	pub fn initialize(&self, up_tree_id: &UptreeID, tree_ids: &HashMap<&str, TreeID>,
			container_from_vm: ContainerFromVm) -> Result<(), Error> {
        Ok(())
	}
}
impl fmt::Display for Container {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}: Service {}", self.id, self.service)
	}
}
