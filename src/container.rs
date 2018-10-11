use std::fmt;
use failure::Error;

use message_types::{ContainerToVm, ContainerFromVm};
use name::{ContainerID, UptreeID};
use service::{Service};
use uptree_spec::AllowedTree;

#[derive(Debug, Clone)]
pub struct Container {
    id: ContainerID,
    allowed_trees: Vec<AllowedTree>,
    service: Service,
}
impl Container {
    pub fn new(id: &ContainerID, service_name: &str, allowed_trees: &Vec<AllowedTree>,
               container_to_vm: ContainerToVm) -> Result<Container, Error> {
        //println!("Create container {}", id);
        let service = Service::new( &id, service_name, allowed_trees, container_to_vm)?;
        Ok(Container { id: id.clone(), allowed_trees: allowed_trees.clone(), service })
    }
    pub fn initialize(&self, up_tree_id: &UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        self.service.initialize(up_tree_id, container_from_vm)
    }
}
impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: Service {}", self.id, self.service)
    }
}
