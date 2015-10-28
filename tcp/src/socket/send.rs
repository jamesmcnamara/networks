use std::collections::BTreeSet;
use std::io::{Bytes, Read, Write};
use std::iter::FromIterator;
use std::net::UdpSocket;
use std::sync::mpsc;

use itertools::Itertools;
use schedule_recv::oneshot_ms;
use time;

use packet::{Packet, Flag}; 
use super::Msg;

pub struct SendSock {
    inner: UdpSocket,
    dest: String,
    acked: u64,
    seq: u64,
    msg_chan: mpsc::Receiver<Msg>,
    dup_acks: usize,
    outstanding: Vec<Packet>,
    limit: usize,
    timeout_ms: u32,
    last_sync: time::Tm,
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
        acked: 212,
        seq: 212,
        msg_chan: msg_chan,
        dup_acks: 0,
        outstanding: vec![],
        limit: 100,
        timeout_ms: 3000,
        last_sync: time::now(),
        retransmit: false,
    }
}


impl SendSock {

    /// Packetizes the given byte stream and transmits it to the dest
    pub fn send<R: Read>(&mut self, bytes: Bytes<R>) {
        let bytes = bytes.map(|b| b.unwrap()).into_iter().chunks_lazy(2048);
        let mut packet_stream = bytes.into_iter();
        loop {
            let msgs = self.receive_messages();
            let (ack, count) = self.process_messages(msgs);

            // If acks were received, update the buffer and acks,
            // or count the duplicates
            self.handle_acks(ack, count);
            
            while self.outstanding.len() < self.limit { 
                if self.retransmit {
                    let slack = self.limit - self.outstanding.len();
                    let dup_packets = (&self.outstanding[0..slack])
                        .iter()
                        .cloned()
                        .collect_vec();
                    for packet in dup_packets { 
                        self.outstanding.push(packet);
                        self.send_packet(self.outstanding.last().unwrap());
                    }
                } else {
                    match packet_stream.next() {
                        Some(block) => {
                            self.packetize_block(block.collect());
                            self.send_packet(self.outstanding.last().unwrap())
                        }
                        None      => return,
                    }
                }
            }
        }
    }

    fn packetize_block(&mut self, payload: Vec<u8>) {
        self.outstanding.push(Packet::new(Flag::Data(self.seq), payload));
        let packet = self.outstanding.last().unwrap();
        self.seq += packet.len();
    }

    fn receive_messages(&mut self) -> Vec<Msg> {
        let mut msgs: Vec<Msg> = vec![];
        if self.outstanding.len() == self.limit {
            // Wait for an ack or a timeout. Separate assignments are required
            // to massage into select's syntax
            let timer = oneshot_ms(self.timeout_ms);
            let ref msg_chan = self.msg_chan;
            select! {
               msg = msg_chan.recv() => {msgs.push(msg.unwrap())}, 
               _   = timer.recv()    => {self.retransmit = true}
            }
        }

        while let Ok(n) = self.msg_chan.try_recv() {
            msgs.push(n);
        }
        msgs
    }

    fn process_messages(&mut self, msgs: Vec<Msg>) -> (u64, usize) {
        let acks: Vec<u64> = msgs.iter().filter_map(|msg| {
            match *msg {
                Msg::Ack(n)            =>  Some(n),
                Msg::SyncReq(n, ref v) => {self.sync(n, v); None},
                Msg::Fin(_)            =>  None,
            }
        })
        .collect();

        match acks.iter().max() {
            Some(&max) => 
                (max, acks.iter().fold(0, |count, &x| count + (x == max) as usize)),
            None       => (0, 0),
        }
    }

    fn handle_acks(&mut self, ack: u64, count: usize) {
        if count > 0 {
            if ack > self.acked {
                self.outstanding.retain(|p| p.seq() >= ack);
                self.acked = ack;
                self.dup_acks = 0;
                self.retransmit = false;
            } else {
                self.dup_acks += count;
            }
            log!("[recv ack] {}", self.acked);

            // 3 or more duplicate acks in a row trigger retransmission
            if count > 2 || self.dup_acks > 2 { 
                self.retransmit = true;
                self.dup_acks = 0;
            }
        }
    }

    /// Sends a packet to the sockets destination. Ensures that the packet at
    /// least gets onto the wire 
    fn send_packet(&self, packet: &Packet) {
        log!("[send data] {} ({}) ={}= {}", 
             packet.seq(), packet.len(), self.acked, self.outstanding.len());
        if let Err(e) = self.inner.send_to(&packet.encode().into_bytes(), 
                                           self.dest.as_str()) {
            log!("send failed: {}", e);
            self.send_packet(packet);
        }
    }

    fn sync(&mut self, acked: u64, recvrs_buffer: &[u64]) {
        let three_seconds = time::Duration::seconds(3);
        if (time::now() - three_seconds) > self.last_sync { 
            let recvrs_buffer = BTreeSet::from_iter(recvrs_buffer.iter());
            self.outstanding
                .retain(|p| p.seq() >= acked && !recvrs_buffer.contains(&p.seq()));

            for packet in &self.outstanding {
                log!("[retransmitting from sync] {}", packet.seq());
                self.send_packet(packet);
            }
        }
    }
}
