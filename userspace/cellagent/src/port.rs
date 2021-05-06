use std::{
    clone::{Clone},
    fmt,
    thread,
    thread::JoinHandle,
};

use crate::app_message_formats::{PortToCa, PortToCaMsg, PortFromCa};
use crate::config::CONFIG;
use crate::dal::{add_to_trace, fork_trace_header, update_trace_header};
use crate::ec_message_formats::{PeToPortPacket, PortToPe, PortFromPe};
use crate::name::{Name, CellID, PortID};
use crate::packet::{Packet};
use crate::utility::{ActivityData, ByteArray, PortNo, PortNumber, S,
                     TraceHeader, TraceHeaderParams, TraceType,
                     write_err};

#[derive(Clone, Debug)]
pub struct DuplexPortPeChannel {
    port_from_pe: PortFromPe,
    port_to_pe: PortToPe,
}
impl DuplexPortPeChannel {
    pub fn new(port_from_pe: PortFromPe, port_to_pe: PortToPe) -> DuplexPortPeChannel {
        DuplexPortPeChannel { port_from_pe, port_to_pe }
    }
    pub fn get_port_from_pe(&self) -> &PortFromPe { &self.port_from_pe }
    pub fn get_port_to_pe(&self) -> &PortToPe { &self.port_to_pe }
}

#[derive(Clone, Debug)]
pub struct DuplexPortCaChannel {
    port_from_ca: PortFromCa,
    port_to_ca: PortToCa,
}
impl DuplexPortCaChannel {
    pub fn new(port_from_ca: PortFromCa, port_to_ca: PortToCa) -> DuplexPortCaChannel {
        DuplexPortCaChannel { port_from_ca, port_to_ca }
    }
    pub fn get_port_from_ca(&self) -> &PortFromCa { &self.port_from_ca }
    pub fn get_port_to_ca(&self) -> &PortToCa { &self.port_to_ca }
}

// TODO: There is no distinction between a broken link and a disconnected one.  We may want to revisit.
#[derive(Debug, Copy, Clone, Serialize)]
#[cfg(feature = "api-new")]
pub enum PortStatus {
    Connected,
    Disconnected(FailoverInfo),
}
#[cfg(feature = "api-new")]
impl fmt::Display for PortStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortStatus::Connected    => write!(f, "Connected"),
            PortStatus::Disconnected(info) => write!(f, "Disconnected: FailoverInfo {}", info)
        }
    }
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[cfg(feature = "api-old")]
pub enum PortStatus {
    Connected,
    Disconnected,
}
#[cfg(feature = "api-old")]
impl fmt::Display for PortStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortStatus::Connected    => write!(f, "Connected"),
            PortStatus::Disconnected => write!(f, "Disconnected")
        }
    }
}

pub trait CommonPortLike: 'static {
    fn get_id(&self) -> PortID { return self.get_base_port().get_id(); }
    fn get_cell_id(&self) -> CellID { return self.get_base_port().get_cell_id(); }
    fn get_port_no(&self) -> PortNo { return self.get_base_port().get_port_no(); }
//  fn get_port_number(&self) -> PortNumber;
    fn get_whether_connected(&self) -> bool;
    fn set_connected(&mut self);
    fn set_disconnected(&mut self);

    // THESE COULD BE PROTECTED
    fn get_base_port(&self) -> &BasePort;
}

pub trait InteriorPortLike: 'static + Clone + Sync + Send + CommonPortLike {
    fn listen_link_and_pe(&mut self) {
        let _f = "listen_link_and_pe_loops";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let port_clone = self.clone();
        let thread_name = format!("Port {} listen_link", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen().map_err(|e| write_err("port listen link", &e));
            if CONFIG.continue_on_error { port.listen().map_err(|e| write_err("port continue listen link", &e)).ok();  }
        }).expect("thread failed");
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} listen_pe", port_clone.get_id().get_name());
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_pe_loop().map_err(|e| write_err("port listen pe", &e));
            if CONFIG.continue_on_error { port.listen_pe_loop().map_err(|e| write_err("port continue listen pe", &e)).ok(); }
        }).expect("thread failed");
    }

    // THESE COULD BE PROTECTED
    fn send_to_link(self: &mut Self, packet: &mut Packet) -> Result<(), Error>;
    fn listen_link(&mut self, port_pe: &DuplexPortPeChannel) -> Result<(), Error>;

    // THESE COULD BE PRIVATE
    fn listen(&mut self) -> Result<(), Error> {
        let _f = "listen";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_pe = self.get_duplex_port_pe_channel().clone();
        self.listen_link(&port_pe)
    }
    fn get_duplex_port_pe_channel(&self) -> &DuplexPortPeChannel {
        self.get_base_port().get_duplex_port_pe_channel()
    }
    // WORKER (PortFromPe)
    fn listen_pe_loop(&mut self) -> Result<(), Error> {
        let _f = "listen_pe_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            //println!("Port {}: waiting for packet from pe", id);
            let msg = self.get_duplex_port_pe_channel().port_from_pe.recv().context(PortError::Chain { func_name: _f, comment: S(self.get_id().get_name()) + " port_from_pe"})?;
            match msg {
                PeToPortPacket::Packet(mut packet) => {
                    {
                        let ait_state = packet.get_ait_state();
                        if CONFIG.trace_options.all || CONFIG.trace_options.port {
                            let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_pe_like" };
                            let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "ait_state": ait_state, "packet":packet.stringify()? });
                            add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                        }
                }
                    self.send_to_link(&mut packet)?;
                }
                #[cfg(feature = "api-new")]
                PortToPePacket::Packet(packet) => { todo!() },
                #[cfg(feature = "api-new")]
                PortToPePacket::Ready => { todo!() },
                #[cfg(feature = "api-new")]
                PortToPePacket::Activity => { todo!() }
            }
        }  
    } 
}

pub trait BorderPortLike: 'static + Clone + Sync + Send + CommonPortLike {
    fn listen_noc_and_ca(&self) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_noc_and_ca";
        let status = PortToCaMsg::Status(self.get_port_no(), PortStatus::Connected);
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_to_pe_status" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "status": PortStatus::Connected });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        let port_to_ca = self.get_base_port().get_duplex_port_ca_channel().port_to_ca;
        port_to_ca.send(status).context(PortError::Chain { func_name: "noc_channel", comment: S(self.get_id().get_name()) + " send to pe"})?;
        self.clone().listen_noc()?;
        let join_handle = self.listen_ca()?;
        Ok(join_handle)
    }

    // THESE COULD BE PROTECTED
    fn send(self: &Self, bytes: &mut ByteArray) -> Result<(), Error>;
    fn listen_and_forward_to(&mut self, port_to_ca: &PortToCa) -> Result<(), Error>;

    // THESE COULD BE PRIVATE
    fn listen(&mut self) -> Result<(), Error> {
        self.listen_and_forward_to(self.get_base_port().get_duplex_port_ca_channel().get_port_to_ca())
    }
    // SPAWN THREAD (listen_noc_for_pe_loop)
    fn listen_noc(&self) -> Result<(), Error> {
        let _f = "listen_noc";
        let mut port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_noc_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.clone().listen_noc(); }
        })?;
        Ok(())
    }

    // WORKER (PortFromNoc)
    fn listen_noc_loop(&mut self) -> Result<(), Error> {
        let _f = "listen_noc_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        return self.listen();
    }

    // SPAWN THREAD (listen_ca_loop)
    fn listen_ca(&self) -> Result<JoinHandle<()>, Error> {
        let _f = "listen_ca";
        let port = self.clone();
        let child_trace_header = fork_trace_header();
        let thread_name = format!("Port {} {}", self.get_id().get_name(), _f);
        let join_handle = thread::Builder::new().name(thread_name).spawn( move || {
            update_trace_header(child_trace_header);
            let _ = port.listen_ca_loop().map_err(|e| write_err("port", &e));
            if CONFIG.continue_on_error { let _ = port.listen_ca(); }
        })?;
        Ok(join_handle)
    }

    // WORKER (PortFromPe)
    fn listen_ca_loop(&self) -> Result<(), Error> {
        let _f = "listen_ca_loop";
        {
            if CONFIG.trace_options.all || CONFIG.trace_options.port {
                let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "worker" };
                let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "thread_name": thread::current().name(), "thread_id": TraceHeader::parse(thread::current().id()) });
                add_to_trace(TraceType::Trace, trace_params, &trace, _f);
            }
        }
        loop {
            let mut bytes = self.get_base_port().get_duplex_port_ca_channel().port_from_ca.recv().context(PortError::Chain { func_name: _f, comment: S(self.get_id().get_name()) + " recv from ca"})?;
            {
                if CONFIG.trace_options.all || CONFIG.trace_options.port {
                    let trace_params = &TraceHeaderParams { module: file!(), line_no: line!(), function: _f, format: "port_from_ca" };
                    let trace = json!({ "cell_id": self.get_cell_id(), "id": self.get_id().get_name(), "bytes": bytes.stringify()? });
                    add_to_trace(TraceType::Trace, trace_params, &trace, _f);
                }
            }
            (*self).send(&mut bytes)?;
        }
    }
}

#[derive(Debug, Clone)]
pub struct BasePort {
    cell_id: CellID, // Used in trace records
    id: PortID,
    port_number: PortNumber,
    is_border: bool,
    activity_data: ActivityData,
    duplex_port_pe_or_ca_channel: DuplexPortPeOrCaChannel,
}
impl BasePort {
    pub fn new(cell_id: CellID, port_number: PortNumber, is_border: bool,
               duplex_port_pe_or_ca_channel: DuplexPortPeOrCaChannel) -> Result<BasePort, Error> {
        let port_id = PortID::new(cell_id, port_number).context(PortError::Chain { func_name: "new", comment: S(cell_id.get_name()) + &S(*port_number.get_port_no())})?;
        Ok(BasePort { cell_id, id: port_id, port_number, is_border,
                      activity_data: Default::default(),
                      duplex_port_pe_or_ca_channel,
        })
    }
    pub fn get_id(&self) -> PortID { self.id }
    pub fn get_cell_id(&self) -> CellID { self.cell_id }
    pub fn get_port_no(&self) -> PortNo { self.port_number.get_port_no() }
    pub fn is_border(&self) -> bool { self.is_border }
    pub fn get_activity_data(&self) -> &ActivityData { &self.activity_data }
    pub fn update_activity_data(&mut self) { self.activity_data.increment(); }
    fn get_duplex_port_pe_or_ca_channel(&self) -> &DuplexPortPeOrCaChannel { &self.duplex_port_pe_or_ca_channel }
    fn get_duplex_port_pe_channel(&self) -> &DuplexPortPeChannel {
        match self.get_duplex_port_pe_or_ca_channel() {
            DuplexPortPeOrCaChannel::Interior(c) => c,
            DuplexPortPeOrCaChannel::Border(_) => panic!("Looking for Interior, found Border")
        }
    }
    fn get_duplex_port_ca_channel(&self) -> DuplexPortCaChannel {
        match self.duplex_port_pe_or_ca_channel.clone() {
            DuplexPortPeOrCaChannel::Border(c) => c,
            DuplexPortPeOrCaChannel::Interior(_) => panic!("Looking for Border, found Interor")
        }
    }
}
impl fmt::Display for BasePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
// HOW TO GET CONNECTION STATUS PRINTED AGAIN??        
//        let is_connected = self.get_whether_connected();  HOW TO GET THIS PRINTED AGAIN??
        let mut s = format!("BasePort {} {}", self.port_number, self.id);
        if self.is_border { s = s + " is boundary  port,"; }
        else              { s = s + " is ECLP port,"; }
//        if is_connected   { s = s + " is connected"; }
//        else              { s = s + " is not connected"; }
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub enum Port<InteriorPortType: 'static + Clone + InteriorPortLike,
              BorderPortType: 'static + Clone + BorderPortLike,
         > {
    Border(Box<BorderPortType>),
    Interior(Box<InteriorPortType>),
}

#[derive(Debug, Clone)]
pub enum DuplexPortPeOrCaChannel {
    Interior(DuplexPortPeChannel),
    Border(DuplexPortCaChannel),
}

pub trait InteriorPortFactoryLike<InteriorPortType: InteriorPortLike>: Clone {
    fn new_port(&self, cell_id: CellID, id: PortID, port_number: PortNumber, duplex_port_pe_channel: DuplexPortPeChannel) -> Result<InteriorPortType, Error>;
    fn get_port_seed(&self) -> &PortSeed;
    fn get_port_seed_mut(&mut self) -> &mut PortSeed;
}

pub trait BorderPortFactoryLike<BorderPortType: BorderPortLike>: Clone {
    fn new_port(&self, cell_id: CellID, id: PortID, port_number: PortNumber, duplex_port_ca_channel: DuplexPortCaChannel) -> Result<BorderPortType, Error>;
    fn get_port_seed(&self) -> &PortSeed;
    fn get_port_seed_mut(&mut self) -> &mut PortSeed;
}

#[derive(Debug, Clone)]
pub struct PortSeed {
    // This could also contain is_border, but we get that from other information available to NalCell::new
}
impl PortSeed {
    pub fn new() -> PortSeed {
        PortSeed {
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct FailoverInfo {
    port_no: PortNo,
    sent: bool,
    recd: bool,
    packet_opt: Option<Packet>
}
impl FailoverInfo {
    pub fn new(port_no: PortNo) -> FailoverInfo { 
        FailoverInfo { port_no, sent: false, recd: false, packet_opt: Default::default() }
    }
    pub fn if_sent(&self) -> bool { self.sent }
    pub fn if_recd(&self) -> bool { self.sent | self.recd }
    pub fn get_saved_packet(&self) -> Option<Packet> { self.packet_opt }
    // Call on every data packet send
    pub fn save_packet(&mut self, packet: &Packet) {
        self.sent = true;
        self.recd = false;
        self.packet_opt = Some(packet.clone());
    }
    // Call on every data packet receive
    pub fn clear_saved_packet(&mut self) {
        self.sent = false;
        self.recd = true;
        self.packet_opt = None;
    }
}
impl fmt::Display for FailoverInfo {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let packet_out = match self.packet_opt {
            Some(p) => p.stringify().expect("Failover Display: Stringify packet must succeed"),
            None => "None".to_string()
        };
        write!(_f, "PortID {} Sent {}, Recd {}, Packet {:?}", self.port_no, self.if_sent(), self.if_recd(), packet_out)
    }
}
// Errors
use failure::{Error, ResultExt};
#[derive(Debug, Fail)]
pub enum PortError {
    #[fail(display = "PortError::Chain {} {}", func_name, comment)]
    Chain { func_name: &'static str, comment: String },
    #[fail(display = "PortError::NonApp {}: Non APP message received on port {}, with is a border port", func_name, port_no)]
    NonApp { func_name: &'static str, port_no: u8 },
    #[fail(display = "PortError::App {}: APP message received on port {}, with is not a border port", func_name, port_no)]
    App { func_name: &'static str, port_no: u8 }
}
