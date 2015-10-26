use std::io;
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
        timeout_ms: 30000
    }
}


impl SendSock {

    /// Packetizes the given byte stream and transmits it to the dest
    pub fn send<R: io::Read>(&mut self, bytes: io::Bytes<R>) {
        for byte_block in bytes.into_iter().chunks_lazy(2048).into_iter() {
            // Intentionally crashes on malformed input bytes
            let payload: Vec<_> = byte_block.map(|x| x.unwrap()).collect();
            let len = payload.len() as u64;
            self.send_packet(Packet::new(Flag::Data(self.acked), payload));
            self.acked += len;
        }
    }
 
    /// Sends a packet to the sockets destination. Waits for timeouts or an ack
    /// before returning, ensuring safe delivery
    fn send_packet(&self, packet: Packet) {
        let expected_ack = self.acked + packet.len();
        if let Err(e) = self.inner.send_to(&packet.encode().into_bytes(), 
                                           self.dest.as_str()) {
            println!("sending error: {}", e);
            self.send_packet(packet);
        }
        else {
            // Wait for an ack or a timeout. Separate assignments are required
            // to massage into select's syntax
            let timer = oneshot_ms(self.timeout_ms);
            let ack_chan = &self.ack_chan;
            select! {
               ack = ack_chan.recv() => { 
                   if ack.unwrap() != expected_ack {
                        self.send_packet(packet);
                   }
               },
               _ = timer.recv() => self.send_packet(packet)
            }
        }
    }
}
