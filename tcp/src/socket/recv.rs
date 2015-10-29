use std::collections::BTreeMap;
use std::cmp::Ordering;
use std::io::Write;
use std::mem;
use std::net::{UdpSocket, SocketAddr};
use std::sync::mpsc;

use packet::{Packet, Flag}; 
use super::Msg;

/// A socket which only receives messages, dispatches acks, and passes
/// received acks to the sender socket. Provides minimal buffering for 
/// out of order packets.
pub struct RecvSock {
    inner: UdpSocket,
    dest: Option<SocketAddr>,
    acked: u64,
    closed: bool,
    msg_chan: mpsc::Sender<Msg>,
    buffer: BTreeMap<u64, Packet>,

}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_recv_sock(inner: UdpSocket, dest: Option<SocketAddr>, 
                      msg_chan: mpsc::Sender<Msg>) -> RecvSock {
    RecvSock {
        inner: inner,
        dest: dest,
        acked: 0,
        closed: false,
        msg_chan: msg_chan,
        buffer: BTreeMap::new(),
    }
}

impl RecvSock {

    /// Infinite loop which reads messages into a buffer and then dispatches
    /// handling to `process_message`
    pub fn recv(mut self) {
        loop {
            if self.closed {return};
            let mut payload = [0u8; 32768];
            let (len, addr) = match self.inner.recv_from(&mut payload) {
                Ok((n, addr))  => (n, addr),
                Err(e) => {log!("{}", e); continue;},
            };
            if let None = self.dest {
                self.dest = Some(addr);
            }
            if let Ok(pack) = String::from_utf8((&payload[0..len]).to_vec()) {
                self.process_message(pack);
            }
        }
    }
   
    /// Attempts to decode the packet and transmit the appropriate ack
    /// for data messages. Passes received acks to the sender sock 
    fn process_message(&mut self, json: String) {
        if let Ok(packet) = Packet::decode(&json) {
            match packet.flag {
                Flag::Data(_) =>  self.process_new_data(packet),
                Flag::Ack(n)  => {
                    self.msg_chan.send(Msg::Ack(n))
                        .ok().expect("sender hung up");},
                Flag::Fin(n)  => {
                    drop(self.msg_chan.send(Msg::Fin(n)));
                    self.fin();
                    self.closed = true;
                    log!("[completed] {}", self.acked())
                },
            }
        }
    }
   
    fn process_new_data(&mut self, packet: Packet) {
        let seq = packet.seq();
        let len = packet.len();
        let status = match seq.cmp(&self.acked) {
            Ordering::Less    => "IGNORED",
            Ordering::Equal   => {
                self.acked += packet.len();
                self.read_buffer();
                "ACCEPTED (in-order)"
            },
            Ordering::Greater => {
                self.buffer.insert(seq, packet);
                "ACCEPTED (out-of-order)"
            }
        };

        log!("[recv data] {} ({}) {} {}", seq, len, status, self.buffer.len());
        self.ack();
    }


    fn read_buffer(&mut self) {
        let buffer = mem::replace(&mut self.buffer, BTreeMap::new());
        for (seq, packet) in buffer {
            if seq == self.acked {
                self.acked += packet.len();
                println!("{}", packet.body());
            } else {
                self.buffer.insert(seq, packet);
            }
        }
    }

    /// Transmits an ack of the current bytes fully read. Continuously 
    /// retransmits on failure 
    fn ack(&self) {
        let ack = Packet::new(Flag::Ack(self.acked), vec![]);
        self.send_packet(ack);
    }
    
    /// Echos a fin to signify termination of the connection 
    fn fin(&self) {
        let fin = Packet::new(Flag::Fin(self.acked), vec![]);
        self.send_packet(fin);
    }
   
    fn send_packet(&self, packet: Packet) {
        if let Err(e) = 
            self.inner.send_to(packet.encode().as_bytes(), 
                               self.dest.expect("No destination defined")) {
            log!("Error occured on packet transmission: {}", e);
            self.send_packet(packet);
        }
    }
}
