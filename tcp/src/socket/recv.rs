use std::collections::BTreeMap;
use std::cmp::Ordering;
use std::io::Write;
use std::mem;
use std::net::UdpSocket;
use std::sync::mpsc;

use itertools::Itertools;

use packet::{Packet, Flag}; 
use super::Msg;

/// A socket which only receives messages, dispatches acks, and passes
/// received acks to the sender socket. Provides minimal buffering for 
/// out of order packets.
pub struct RecvSock {
    inner: UdpSocket,
    dest: String,
    acked: u64,
    msg_chan: mpsc::Sender<Msg>,
    buffer: BTreeMap<u64, Packet>,
    limit: usize,

}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_recv_sock(inner: UdpSocket, dest: &str, 
                      msg_chan: mpsc::Sender<Msg>) -> RecvSock {
    RecvSock {
        inner: inner,
        dest: dest.to_string(),
        acked: 212,
        msg_chan: msg_chan,
        buffer: BTreeMap::new(),
        limit: 10
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
                Err(e) => {log!("{}", e); continue;},
            };
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
                Flag::Ack(n)  => {self.msg_chan.send(Msg::Ack(n)).unwrap();},
                Flag::Fin(n)  => {self.msg_chan.send(Msg::Fin(n)).unwrap();},
                Flag::SyncReq(n) => {
                    let seqs = self.decode_seqs(packet.payload());
                    self.msg_chan.send(Msg::SyncReq(n, seqs)).unwrap();
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

        log!("[recv data] {} ({}) {} {} ={}=", 
             seq, len, status, self.buffer.len(), self.acked);
        self.ack();
    }


    fn read_buffer(&mut self) {
        let buffer = mem::replace(&mut self.buffer, BTreeMap::new());
        for (seq, packet) in buffer {
            if seq == self.acked {
                self.acked += packet.len();
                println!("{}", packet.body());
                log!("[served from buffer] {}", packet.seq());
            } else {
                self.buffer.insert(seq, packet);
            }
        }
    }

    fn sync_req(&self) {
        let sync = Packet::new(Flag::SyncReq(self.acked), 
                               self.encode_seqs(self.buffer.keys()));
        log!("sync request, have {:?}", self.buffer.keys().collect_vec());
        self.send_packet(sync);
    }

    /// Transmits an ack of the current bytes fully read. Continuously 
    /// retransmits on failure 
    fn ack(&self) {
        let ack = Packet::new(Flag::Ack(self.acked), vec![]);
        self.send_packet(ack);
    }
   
    fn send_packet(&self, packet: Packet) {
        if let Err(e) = self.inner.send_to(packet.encode().as_bytes(), 
                                           self.dest.as_str()) {
            log!("Error occured on packet transmission: {}", e);
            self.send_packet(packet);
        }
    }

    fn encode_seqs<'a, Seqs>(&self, seqs: Seqs) -> Vec<u8> 
        where Seqs: Iterator<Item=&'a u64>
    {
        seqs.flat_map(|num| 
                      (0..8).map(|i| 
                                 (num >> (i * 8)) as u8).rev().collect_vec())
            .collect()
            
    }

    fn decode_seqs<'a, Raw>(&self, bytes: Raw) -> Vec<u64> 
        where Raw: Iterator<Item=&'a u8> 
    {
        bytes.chunks_lazy(8)
            .into_iter()
            .map(|num_arr| 
                 num_arr.fold(0, |sum, num| (sum << 8) + *num as u64))
            .collect()
    }
}
