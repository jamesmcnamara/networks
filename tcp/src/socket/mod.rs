pub use self::send::SendSock;
use self::send::make_send_sock;

pub use self::recv::RecvSock;
use self::recv::make_recv_sock;

pub mod send;
pub mod recv;

use std::net::{UdpSocket, SocketAddr};
use std::sync::mpsc;
use std::str::FromStr;

pub enum Msg {
    Ack(u64),
    Fin(u64),
}

/// Opens a connection between `addr` and `dest` and returns a 
/// `SendSock` and `RecvSock` pair, which will manage reliable transfer
pub fn open_sender(local: UdpSocket, dest: &str) ->(SendSock, RecvSock) {
    let (sx, rx) = mpsc::channel();
    let recvr = local.try_clone().ok().expect("socket clone failed");
    let send_sock = make_send_sock(local, dest, rx);
    let dest = SocketAddr::from_str(dest).ok();
    let recv_sock = make_recv_sock(recvr, dest, sx);
    (send_sock, recv_sock)
}


/// Converts a UdpSocket into a receiver, which has no ability to send
/// data messages in the reverse direction (only acks)
pub fn open_recvr(local: UdpSocket) -> RecvSock {
    let (sx, _) = mpsc::channel();
    make_recv_sock(local, None, sx)
}

