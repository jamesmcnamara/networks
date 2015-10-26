pub use self::send::SendSock;
use self::send::make_send_sock;

pub use self::recv::RecvSock;
use self::recv::make_recv_sock;

pub mod send;
pub mod recv;

use std::net::UdpSocket;
use std::sync::mpsc;

/// Opens a connection between `addr` and `dest` and returns a 
/// `SendSock` and `RecvSock` pair, which will manage reliable transfer
pub fn open_connection(addr: &str, dest: &str) -> (SendSock, RecvSock) {
    let sender = UdpSocket::bind(addr)
        .ok()
        .expect(&format!("binding to interface {} failed", addr));
    let recvr = sender.try_clone()
        .ok()
        .expect("cloning socket failed");
    let (sx, rx) = mpsc::channel();
    let send_sock = make_send_sock(sender, dest, rx);
    let recv_sock = make_recv_sock(recvr, dest, sx);
    (send_sock, recv_sock)
}
