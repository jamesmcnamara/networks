use std::io::{Bytes, Read, Write};
use std::net::UdpSocket;
use std::sync::mpsc;

use itertools::Itertools;
use schedule_recv::oneshot_ms;

use packet::{Packet, Flag}; 

pub struct SendSock {
    inner: UdpSocket,
    dest: String,
    acked: u64,
    ack_chan: mpsc::Receiver<u64>,
    dup_acks: usize,
    outstanding: Vec<Packet>,
    limit: usize,
    timeout_ms: u32,
}

/// Public constructor defined outside of the Impl so that the 
/// module can import it, use it to define `open_connection` and
/// not re-export it; hackily creating a protected constructor
pub fn make_send_sock(inner: UdpSocket, dest: &str, 
                      ack_chan: mpsc::Receiver<u64>) -> SendSock {
    SendSock {
        inner: inner,
        dest: dest.to_string(),
        acked: 212,
        ack_chan: ack_chan,
        dup_acks: 0,
        outstanding: vec![],
        limit: 100,
        timeout_ms: 3000
    }
}


impl SendSock {

    /// Packetizes the given byte stream and transmits it to the dest
    pub fn send<R: Read>(&mut self, bytes: Bytes<R>) {
        let bytes = bytes.into_iter().chunks_lazy(2048);
        let mut packet_stream = bytes.into_iter();
        loop {
            let (ack, count) = self.receive_acks();

            // If acks were received, update the buffer and acks,
            // or count the duplicates
            if count > 0 {
                self.handle_acks(ack, count);
            }

            let mut seq_no = self.next_seq_number();
            while self.outstanding.len() < self.limit {
                match packet_stream.next() {
                    Some(block) => {
                        let payload: Vec<_> = block.map(|x| x.unwrap()).collect();
                        let payload_len = payload.len() as u64;
                        self.outstanding.push(Packet::new(Flag::Data(seq_no), 
                                                          payload));
                        self.send_packet(self.outstanding.last().unwrap());
                        seq_no += payload_len;
                    }
                    None        => return,
                }
            }
        }
    }

    fn handle_acks(&mut self, ack: u64, count: usize) {
        if ack > self.acked {
            self.outstanding.retain(|p| p.seq() >= ack);
            self.acked = ack;
            self.dup_acks = 0;
        } else {
            self.dup_acks += count;
        }
        log!("[recv ack] {}", self.acked);

        // 3 or more duplicate acks in a row trigger retransmission
        if count > 2 || self.dup_acks > 2 { 
            self.retransmit();
            self.dup_acks = 0;
        }
    }

    fn next_seq_number(&self) -> u64 {
        match self.outstanding.iter().max_by(|packet| packet.seq()) {
            Some(packet) => packet.seq() + packet.len(),
            None         => self.acked,
        }
    }

    /// Sends a packet to the sockets destination. Ensures that the packet at
    /// least gets onto the wire 
    fn send_packet(&self, packet: &Packet) {
        log!("[send data] {} ({})", packet.seq(), packet.len());
        if let Err(e) = self.inner.send_to(&packet.encode().into_bytes(), 
                                           self.dest.as_str()) {
            self.send_packet(packet);
        }
    }
    
    fn retransmit(&self) {
        log!("[retransmit] {}", self.outstanding.first().unwrap().seq());
        self.send_packet(self.outstanding.first().unwrap());
    }

    fn receive_acks(&self) -> (u64, usize) {
        let mut acks: Vec<u64> = vec![];
        if self.outstanding.len() == self.limit {
            // Wait for an ack or a timeout. Separate assignments are required
            // to massage into select's syntax
            let timer = oneshot_ms(self.timeout_ms);
            let ack_chan = &self.ack_chan;
            select! {
               ack = ack_chan.recv() => {acks.push(ack.unwrap())}, 
               _ = timer.recv() => self.retransmit()
            }
        }

        while let Ok(n) = self.ack_chan.try_recv() {
            acks.push(n);
        }
        if acks.len() > 0 {
            println!("acks are {:?}", acks);
        }
        match acks.iter().max() {
            Some(&max) => (max, acks.
                           iter()
                           .fold(0, |count, &x| count + (x == max) as usize)),
            None    => (0, 0),
        }
    }
}
