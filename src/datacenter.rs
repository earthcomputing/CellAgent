use std::{sync::mpsc::channel};

use crate::app_message_formats::{ApplicationFromNoc, ApplicationToNoc, NocFromApplication, NocToApplication};
use crate::blueprint::{Blueprint};
use crate::noc::{Noc};
use crate::rack::{Rack};
use crate::utility::{S};

#[derive(Debug)]
pub struct Datacenter {
    rack: Rack,
    application_to_noc: ApplicationToNoc,
    application_from_noc: ApplicationFromNoc,
}
impl Datacenter {
    pub fn construct(blueprint: Blueprint) -> Result<Datacenter, Error> {
        println!("{}", blueprint);
        let (mut rack, _join_handles) = Rack::construct(&blueprint).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        let (application_to_noc, noc_from_application): (ApplicationToNoc, NocFromApplication) = channel();
        let (noc_to_application, application_from_noc): (NocToApplication, ApplicationFromNoc) = channel();
        let mut noc = Noc::new(noc_to_application)?;
        let (port_to_noc, port_from_noc) = noc.initialize(&blueprint, noc_from_application).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        rack.connect_to_noc(port_to_noc, port_from_noc).context(DatacenterError::Chain { func_name: "initialize", comment: S("")})?;
        return Ok(Datacenter { rack, application_to_noc, application_from_noc});
    }
    pub fn get_rack(&self) -> &Rack { &self.rack }
    pub fn get_rack_mut(&mut self) -> &mut Rack { &mut self.rack }
    pub fn get_application_to_noc(&self) -> &ApplicationToNoc { &self.application_to_noc }
    pub fn get_application_from_noc(&self) -> &ApplicationFromNoc { &self.application_from_noc }
}

// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum DatacenterError {
    #[fail(display = "DatacenterError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
}
