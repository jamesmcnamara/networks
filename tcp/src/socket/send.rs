use std::cmp;
use std::io::{Bytes, Read, Write};
use std::mem;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::vec;

use itertools::Itertools;
use schedule_recv::oneshot_ms;

use packet::{Packet, Flag}; 
use super::Msg;

pub struct SendSock {
    inner: UdpSocket,
    dest: String,
    acked: u64,
    msg_chan: mpsc::Receiver<Msg>,
    dup_acks: usize,
    goodput: usize,
    outstanding: Vec<Packet>,
    limit: usize,
    timeout_ms: u32,
    retransmit: bool
}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_send_sock(inner: UdpSocket, dest: &str, 
                      msg_chan: mpsc::Receiver<Msg>) -> SendSock {
    SendSock {
        inner: inner,
        dest: dest.to_string(),
        acked: 0,
        msg_chan: msg_chan,
        goodput: 0,
        dup_acks: 0,
        outstanding: vec![],
        limit: 8,
        timeout_ms: 3000,
        retransmit: false,
    }
}


impl SendSock {

    /// Packetizes the given byte stream and transmits it to the dest.
    /// Guarantees delivery of the entire message
    pub fn send<R: Read>(mut self, bytes: Bytes<R>) {
        let mut packet_stream = self.packetize_stream(bytes).into_iter();
        loop {
            // If acks were received, update the buffer and acks,
            // or count the duplicates
            let msgs = self.collect_messages();
            if let Some(ack) = self.calc_ack(msgs) {
                self.handle_ack(ack);
            }

            // If we saw 3 duplicate acks or a timeout, retransmit 
            // and update counters
            if self.retransmit {
                packet_stream = self.retransmit(packet_stream);
            }

            // Else transmit as much as possible
            while self.outstanding.len() < self.limit { 
                match packet_stream.next() {
                    Some(packet) => {
                        if packet.seq() >= self.acked {
                            self.outstanding.push(packet);
                            self.send_packet(self.outstanding.last().unwrap())
                        }
                    },
                    None      => {
                        if self.outstanding.len() <= 0 {
                            break; 
                        }
                        return self.close();
                    }, 
                }
            }
        }
    }
    
    fn close(&self) {
        let fin = Packet::new(Flag::Fin(self.acked), vec![]);
        self.send_packet(&fin);
        let timer = oneshot_ms(self.timeout_ms);
        let ref msg_chan = self.msg_chan;
        select! {
           msg = msg_chan.recv() => { match msg {
               Ok(Msg::Fin(_)) => log!("[completed] {}",  self.acked), 
               _               => self.close()
           }},
           _   = timer.recv()  => self.close()
        }
    }

    fn retransmit(&mut self, packet_stream: vec::IntoIter<Packet>) -> vec::IntoIter<Packet> {
        self.retransmit = false;
        self.limit = cmp::max(self.limit / 2, 1);
        self.dup_acks = 0;
        self.goodput = 0;

        let mut packets = mem::replace(&mut self.outstanding, vec![]);
        packets.extend(packet_stream);
        packets.into_iter()
    }

    fn packetize_stream<R: Read>(&self, bytes: Bytes<R>) -> Vec<Packet> {
        let block_size = 2048;
        bytes.map(|b| b.ok().expect("byte decoding error"))
            .into_iter()
            .chunks_lazy(block_size)
            .into_iter()
            .enumerate()
            .map(|(i, block)| 
                 Packet::new(Flag::Data(self.acked + (i * block_size) as u64), 
                             block.collect()))
            .collect()
    }

    fn collect_messages(&mut self) -> Vec<Msg> {
        let mut msgs: Vec<Msg> = vec![];
        if self.outstanding.len() == self.limit {
            // Wait for an ack or a timeout. Separate assignments are required
            // to massage into select's syntax
            let timer = oneshot_ms(self.timeout_ms);
            let ref msg_chan = self.msg_chan;
            select! {
               msg = msg_chan.recv() => {msgs.push(msg.ok().expect("msg receive"))}, 
               _   = timer.recv()    => {self.retransmit = true}
            }
        }

        while let Ok(n) = self.msg_chan.try_recv() {
            msgs.push(n);
        }
        msgs
    }

    fn calc_ack(&self, msgs: Vec<Msg>) -> Option<u64> {
       msgs.iter().filter_map(|msg| {
            match *msg {
                Msg::Ack(n)            =>  Some(n),
                Msg::Fin(_)            =>  None,
            }
        }).max()
    }

    fn handle_ack(&mut self, ack: u64) {
        log!("[recv ack] {}", self.acked);
        if ack > self.acked {
            self.outstanding.retain(|p| p.seq() >= ack);
            self.goodput += self.limit - self.outstanding.len();
            if self.goodput == self.limit {
                self.limit *= 2;
            }
            self.acked = ack;
            self.dup_acks = 0;
        } else {
            self.dup_acks + 1;
        }

        // 3 or more duplicate acks in a row trigger retransmission
        self.retransmit = self.dup_acks > 2;
    }
    
    /// Sends a packet to the sockets destination. Ensures that the packet at
    /// least gets onto the wire 
    fn send_packet(&self, packet: &Packet) {
        log!("[send data] {} ({})", packet.seq(), packet.len());
        if let Err(e) = self.inner.send_to(&packet.encode().into_bytes(), 
                                           self.dest.as_str()) {
            log!("send failed: {}", e);
            self.send_packet(packet);
        }
    }
}
