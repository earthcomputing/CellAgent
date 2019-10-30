use std::fmt;
use std::collections::HashSet;
use failure::Error;

use crate::app_message_formats::{ContainerToVm, ContainerFromVm};
use crate::name::{ContainerID, UptreeID};
use crate::service::{Service};
use crate::uptree_spec::AllowedTree;

#[derive(Debug, Clone)]
pub struct Container {
    id: ContainerID,
    allowed_trees: HashSet<AllowedTree>,
    service: Service,
}
impl Container {
    pub fn new(id: ContainerID, service_name: &str, allowed_trees: &HashSet<AllowedTree>,
               container_to_vm: ContainerToVm) -> Result<Container, Error> {
        //println!("Create container {}", id);
        let service = Service::new( id, service_name, allowed_trees, container_to_vm)?;
        Ok(Container { id, allowed_trees: allowed_trees.to_owned(), service })
    }
    pub fn initialize(&self, up_tree_id: UptreeID, container_from_vm: ContainerFromVm) -> Result<(), Error> {
        self.service.initialize(up_tree_id, container_from_vm,)
    }
}
impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: Service {}", self.id, self.service)
    }
}