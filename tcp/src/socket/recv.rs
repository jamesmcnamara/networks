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
/// NOTE: `RecvSock`s appear in both the sender and receiver, but the 
/// reciever has additional features to process message. These are
/// annotated with `GlobalReceiver` 
pub struct RecvSock {
    /// Wrapped transmitter
    inner: UdpSocket,

    ///Count of bytes seen in order
    acked: u64,

    /// Should this socket stay open
    closed: bool,

    /// Channel to transmit acks to the sender 
    msg_chan: mpsc::Sender<Msg>,

    /// In-order buffer for out-of-order messages
    buffer: BTreeMap<u64, Packet>,

}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_recv_sock(inner: UdpSocket, msg_chan: mpsc::Sender<Msg>) -> RecvSock {
    RecvSock {
        inner: inner,
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
                Err(e) => {log!("recv error: {}", e); continue;},
            };

            if let Ok(pack) = String::from_utf8((&payload[0..len]).to_vec()) {
                self.process_message(pack, addr);
            }
        }
    }
   
    /// Attempts to decode the packet and transmit the appropriate ack
    /// for data messages. Passes received acks to the sender sock 
    fn process_message(&mut self, json: String, addr: SocketAddr) {
        if let Ok(packet) = Packet::decode(&json) {
            match packet.flag {
                // Only occurs if global sender 
                Flag::Data(_) =>  self.process_new_data(packet, addr),

                // Only occurs if local sender socket. Increment acked
                // and pass on the message to sender
                Flag::Ack(n)  => {
                    self.acked = n;
                    drop(self.msg_chan.send(Msg::Ack(n)))
                },

                //Finalizer message
                Flag::Fin(n)  => {
                    drop(self.msg_chan.send(Msg::Fin(n)));
                    if n == self.acked {
                        self.fin(addr);
                        self.closed = true;
                        log!("[completed] {}", self.acked)
                    }
                },
            }
        }
    }
    
    /// Global Receiver 
    /// Determines if this packet is old, in-order, or out-of-order,
    /// then drops, increments acked, or buffers as appropriate
    /// Always acks
    fn process_new_data(&mut self, packet: Packet, addr: SocketAddr) {
        let seq = packet.seq();
        let len = packet.len();
        let status = match seq.cmp(&self.acked) {
            Ordering::Less    => "IGNORED",
            Ordering::Equal   => {
                self.inc_acked(packet);
                self.read_buffer();
                "ACCEPTED (in-order)"
            },
            Ordering::Greater => {
                self.buffer.insert(seq, packet);
                "ACCEPTED (out-of-order)"
            }
        };

        log!("[recv data] {} ({}) {}", seq, len, status);
        if status != "IGNORED" {
            self.ack(addr);
        }
    }
   
    /// Global Receiver
    /// Consumes a packet, increments acked by it's len, and prints its body
    fn inc_acked(&mut self, packet: Packet) {
        self.acked += packet.len();
        print!("{}", packet.body());
    }

    /// Global Receiver
    /// Iterates through the buffer, and acks all packets that are in 
    /// sequence with the current ack, and reinserts those out of order
    fn read_buffer(&mut self) {
        let buffer = mem::replace(&mut self.buffer, BTreeMap::new());
        for (seq, packet) in buffer {
            if seq == self.acked {
                self.inc_acked(packet);
            } else {
                self.buffer.insert(seq, packet);
            }
        }
    }

    /// Global Receiver
    /// Transmits an ack of the current bytes fully read. Continuously 
    /// retransmits on failure 
    fn ack(&self, addr: SocketAddr) {
        let ack = Packet::new(Flag::Ack(self.acked), vec![]);
        self.send_packet(ack, addr);
    }
    
    /// Echos a fin to signify termination of the connection 
    fn fin(&self, addr: SocketAddr) {
        let fin = Packet::new(Flag::Fin(self.acked), vec![]);
        self.send_packet(fin, addr);
    }
   
    /// Sends a packet to the sockets destination. Ensures that the packet at
    /// least gets onto the wire 
    fn send_packet(&self, packet: Packet, addr: SocketAddr) {
        if let Err(e) = self.inner.send_to(packet.encode().as_bytes(), addr) {
            log!("Error occured on packet transmission: {}", e);
            self.send_packet(packet, addr);
        }
    }
}
