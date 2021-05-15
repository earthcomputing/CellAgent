/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
// Snake is a code name used for a strategy to deal with packets that
// may be lost when a node fails.  As a packet travels from its
// source to its destination, each cell on the path keeps a copy.
// When the packet has reach it destination, it acknowledges 
// receipt back along the path.  The packet is forgotted
// on receiving the acknowledgement.

use std::{fmt};

use crate::packet::Packet;
use crate::utility::PortNo;
#[derive(Clone, Debug, Serialize)]
pub struct Snake {
    ack_port_no: PortNo,
    packet: Packet,
    count: usize,
}
impl Snake {
    pub fn new(port_no: PortNo, count: usize, packet: Packet) -> Snake {
        Snake { ack_port_no: port_no, packet, count}
    }
    pub fn get_ack_port_no(&self) -> PortNo { self.ack_port_no }
    pub fn get_packet(&self) -> &Packet { &self.packet }
    pub fn get_count(&self) -> usize {self.count }
    pub fn set_count(&mut self, count: usize) { self.count = count; }
    pub fn decrement_count(&mut self) -> usize { 
        if self.count > 0 { self.count = self.count - 1; }
        self.count 
    }
}
impl fmt::Display for Snake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("Snake: {} {} {} {}", self.count, self.ack_port_no, 
                            self.packet.get_uniquifier(), self.packet);
        write!(f, "{}", s)
    }
}
