use std::net::UdpSocket;
use std::sync::mpsc;

use packet::{Packet, Flag}; 

/// A socket which only receives messages, dispatches acks, and passes
/// received acks to the sender socket. Provides minimal buffering for 
/// out of order packets.
pub struct RecvSock {
    inner: UdpSocket,
    dest: String,
    acked: u64,
    ack_chan: mpsc::Sender<u64>,
    buffer: Vec<Packet>
}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_recv_sock(inner: UdpSocket, dest: &str, 
                      ack_chan: mpsc::Sender<u64>) -> RecvSock {
    RecvSock {
        inner: inner,
        dest: dest.to_string(),
        acked: 212,
        ack_chan: ack_chan,
        buffer: vec![],
    }
}

impl RecvSock {

    /// Infinite loop which reads messages into a buffer and then dispatches
    /// handling to `process_message`
    pub fn recv(&mut self) {
        loop {
            let mut payload = [0u8; 32768];
            let len = match self.inner.recv_from(&mut payload) {
                Ok((n, _))  => n,
                Err(e) => {println!("{}", e); continue;},
            };
            println!("bytes read: {}", len);
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
                Flag::Data(seq) => {
                    if seq == self.acked {
                        self.acked += packet.len();
                        println!("{}", packet.body());
                    }
                    self.ack();
                }
                Flag::Ack(n) => {
                    self.ack_chan.send(n).unwrap();
                },
            }
        }
    }

    /// Transmits an ack of the current bytes fully read. Continuously 
    /// retransmits on failure 
    fn ack(&self) {
        let ack = Packet::new(Flag::Ack(self.acked), vec![]);
        if let Err(e) = self.inner.send_to(ack.encode().as_bytes(), 
                                           self.dest.as_str()) {
            println!("Error occured on ack transmission: {}", e);
            self.ack();
        }
    }
}
