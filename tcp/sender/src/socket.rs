use packet::packet::{Packet, Flag}; 

use std::error::Error;
use std::io;
use std::iter;
use std::net::{UdpSocket, ToSocketAddrs};
use std::sync::mpsc;

use schedule_recv::oneshot_ms;

use itertools::Itertools;

pub struct SendSock {
    inner: UdpSocket,
    other: String,
    acked: u64,
    ack_chan: mpsc::Receiver<u64>,
    timeout_ms: u32,
}

impl SendSock {
    pub fn send<R: io::Read>(&mut self, bytes: io::Bytes<R>) {
        for byte_block in bytes.into_iter().chunks_lazy(1500).into_iter() {
            println!("about to send a block");
            let payload: Vec<_> = byte_block.map(|x| x.unwrap()).collect();
            let len = payload.len() as u64;
            println!("sending packet with {} and len: {}", self.acked, len);
            self.send_packet(Packet::new(Flag::Data(self.acked), 
                                         payload));
            println!("Packet acked continuing");
            self.acked += len;
        }
    }
  
    fn send_packet(&self, packet: Packet) {
        let expected_ack = self.acked + packet.len();
        let borr_other: &str = &(self.other);
        let message = packet.encode().into_bytes();
        println!("message length is {}", message.len());
        self.inner.send_to(&packet.encode().into_bytes(), borr_other);
        let timer = oneshot_ms(self.timeout_ms);
        let ack_chan = &self.ack_chan;
        select! {
           ack = ack_chan.recv() => { 
               if ack.unwrap() != expected_ack {
                   println!("retransmit");
                    self.send_packet(packet);
               }
               println!("acked!");
           },
           _ = timer.recv() => self.send_packet(packet)
        }
    }

}

pub struct RecvSock {
    inner: UdpSocket,
    other: String,
    acked: u64,
    ack_chan: mpsc::Sender<u64>,
    buffer: Vec<Packet>
}

impl RecvSock {
    pub fn recv(&mut self) {
        loop {
            println!("starting a recv loop");
            let mut payload = [0u8; 32768];
            let n = match self.inner.recv_from(&mut payload) {
                Ok((n, _))  => n,
                Err(e) => continue,
            };
            let payload = &payload[0..n];
            if let Ok(pack) = String::from_utf8(payload.to_vec()) {
                let packet = Packet::decode(&pack).unwrap();
                match packet.flag {
                    Flag::Data(seq) => {
                        println!("received packet with seq {} and my ack is: {}", seq, self.acked);
                        if self.acked == seq {
                            self.acked += packet.len();
                            println!("{}", packet.body());
                        }
                        self.ack();
                    }
                    Flag::Ack(n) => {self.ack_chan.send(n).unwrap();},
                    Flag::Fin    => {self.close();},
                }
            }
        }
    }
    
    fn ack(&self) {
        let ack = Packet::new(Flag::Ack(self.acked), vec![]);
        let borr_other: &str = &(self.other);
        if let Err(_) = self.inner.send_to(ack.encode().as_bytes(), borr_other) {
            self.ack();
        }
    }

    fn close(&self) {}
}

pub fn open_connection(addr: &str, other: &str) -> (SendSock, RecvSock) {
    let sender = UdpSocket::bind(addr).ok().expect("bind failed");
    let recvr = sender.try_clone().ok().expect("clone failed");
    let (sx, rx) = mpsc::channel();
    let send_sock = SendSock {
        inner: sender,
        other: other.to_string(),
        acked: 212,
        ack_chan: rx,
        timeout_ms: 30000
    };
    let recv_sock = RecvSock {
        inner: recvr,
        other: other.to_string(),
        acked: 212,
        ack_chan: sx,
        buffer: vec![],
    };
    (send_sock, recv_sock)
}
