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
    /// Wrapped socket
    inner: UdpSocket,
    dest: String,
    acked: u64,

    /// Channel to receive acks from the RecvSock
    msg_chan: mpsc::Receiver<Msg>,

    /// tally of duplicate acks
    dup_acks: usize,

    /// tally of successful messages acked in a row
    goodput: usize,

    /// buffer
    outstanding: Vec<Packet>,

    /// Number of packets that can be outstanding at once
    limit: usize,
    timeout_ms: u32,

    /// Should the socket retransmit the packets in the buffer
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
        limit: 12,
        timeout_ms: 5000,
        retransmit: false,
    }
}


impl SendSock {

    /// Packetizes the given byte stream and transmits it to the dest.
    /// Guarantees delivery of the entire message
    pub fn send<R: Read>(mut self, bytes: Bytes<R>) {
        let mut packet_stream = self.packetize_stream(bytes).into_iter();
        loop {
            // If we saw 3 duplicate acks or a timeout, retransmit 
            // and update counters
            if self.retransmit {
                packet_stream = self.retransmit(packet_stream);
            }

            // Send as many packets as the window allows, and 
            // determine if we are done transferring
            if !self.transmit(&mut packet_stream) {
                break;
            }

            // If acks were received, update the buffer and acks,
            // or count the duplicates
            let msgs = self.collect_messages();
            if let Some((ack, n)) = self.calc_ack(msgs) {
                self.handle_acks(ack, n);
            }
        }
        self.close()
    }

    /// Consumes a stream of bytes, chunks them, then returns them as packets
    fn packetize_stream<R: Read>(&self, bytes: Bytes<R>) -> Vec<Packet> {
        let block_size = 4096;
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

    /// Concatenates all of the outstanding messages onto the packet stream
    /// and shortens the window of allowed outstanding buffers
    fn retransmit(&mut self, packet_stream: vec::IntoIter<Packet>) -> vec::IntoIter<Packet> {
        self.retransmit = false;
        self.limit = cmp::max((self.limit * 3) / 4, 1) as usize;
        self.dup_acks = 0;
        self.goodput = 0;

        let mut packets = mem::replace(&mut self.outstanding, vec![]);
        packets.extend(packet_stream);
        packets.into_iter()
    }

    /// Transmit as much as possible, and determine if we should continue
    /// looping or not
    fn transmit(&mut self, packets: &mut vec::IntoIter<Packet>) -> bool {
        while self.outstanding.len() < self.limit { 
            match packets.next() {
                Some(packet) => {
                    if packet.seq() >= self.acked {
                        self.outstanding.push(packet);
                        self.send_packet(self.outstanding.last().unwrap())
                    }
                },
                None      => { return self.outstanding.len() > 0; },
            }
        }
        true
    }
    
    
    /// Blocks until it receives a message from the receiver socket or a timeout
    /// fires (in which case it flags the socket to begin retransmission). Then
    /// collects as many outstanding acks as possible from the channel, in the
    /// case of multiple acks
    fn collect_messages(&mut self) -> Vec<Msg> {
        let mut msgs: Vec<Msg> = vec![];
        // Wait for an ack or a timeout. Separate assignments are required
        // to massage into select's syntax
        let timer = oneshot_ms(self.timeout_ms);
        let ref msg_chan = self.msg_chan;
        select! {
           msg = msg_chan.recv() => {msgs.push(msg.ok().expect("local receiver hung up"))}, 
           _   = timer.recv()    => {self.retransmit = true}
        }

        while let Ok(n) = self.msg_chan.try_recv() {
            msgs.push(n);
        }
        msgs
    }   

    /// Determines the largest ack in the messages, and how many times
    /// it appears
    fn calc_ack(&self, msgs: Vec<Msg>) -> Option<(u64, usize)> {
       msgs.iter().filter_map(|msg| {
            match *msg {
                Msg::Ack(n)            =>  Some(n),
                Msg::Fin(_)            =>  None,
            }
        }).fold(None, |max_count, element|
                match max_count {
                    Some((n, _)) if n < element  => Some((element, 1)),
                    Some((n, c)) if n == element => Some((n, c + 1)),
                    None                         => Some((element, 1)),
                    acc                          => acc,
                }
        )
    }

    /// Given an ack and a count, determines if it should increment the window
    /// and ack count, or if it should begin a retransmission
    fn handle_acks(&mut self, ack: u64, count: usize) {
        log!("[recv ack] {}", self.acked);
        if ack > self.acked {
            self.outstanding.retain(|p| p.seq() >= ack);
            self.goodput += self.limit - self.outstanding.len();
            if self.goodput == self.limit {
                self.limit *= 4;
            }
            self.acked = ack;
            self.dup_acks = count;
        } else {
            self.dup_acks += count;
        }

        // 3 or more duplicate acks in a row trigger retransmission
        self.retransmit = self.dup_acks > 2;
    }

    /// Sends a packet to the sockets destination. Ensures that the packet at
    /// least gets onto the wire 
    fn send_packet(&self, packet: &Packet) {
        log!("[send data] {} ({}), ={}= {}", packet.seq(), packet.len(), self.acked, self.limit);
        if let Err(e) = self.inner.send_to(&packet.encode().into_bytes(), 
                                           self.dest.as_str()) {
            log!("send failed: {}", e);
            self.send_packet(packet);
        }
    }

    /// Transmits a fin message, and waits for a timeout or an ack, and then 
    /// recurses or returns respectively
    fn close(&self) {
        let fin = Packet::new(Flag::Fin(self.acked), vec![]);
        self.send_packet(&fin);
        if self.wait_for_fin() {
            log!("[completed] {}", self.acked);
        } else {
            self.close()
        }

    }

    /// Blocks until it receives a message from the receiver socket or a timeout
    /// fires (in which case it flags the socket to begin retransmission). Then
    /// collects as many outstanding acks as possible from the channel, in the
    /// case of multiple acks
    fn wait_for_fin(&self) -> bool {
        // Wait for an ack or a timeout. Separate assignments are required
        // to massage into select's syntax
        let timer = oneshot_ms(self.timeout_ms);
        let ref msg_chan = self.msg_chan;
        loop {
            select! {
               msg = msg_chan.recv() => {
                   if let Ok(Msg::Fin(_)) = msg {
                        return true;
                   }
               },
               _   = timer.recv()    => {break}
            }
        }
        false
    }
}
