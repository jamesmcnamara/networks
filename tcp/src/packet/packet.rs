use std::hash::{Hash, Hasher, SipHasher};
use std::slice::Iter;

use packet::{Result, PacketError};
use rustc_serialize::json;

#[derive(PartialEq, Debug, RustcEncodable, RustcDecodable, Clone)]
pub enum Flag {
    Data(u64),
    Ack(u64),
    SyncReq(u64),
    Fin(u64),
}


#[derive(PartialEq, Debug, RustcEncodable, RustcDecodable, Clone)]
pub struct Packet {
    pub flag: Flag,
    hash: u64,
    payload: Vec<u8>,
}

impl Packet {
    pub fn new(flag: Flag, payload: Vec<u8>) -> Packet {
        Packet {
            flag: flag,
            hash: Packet::hash_payload(&payload),
            payload: payload
        }
    }
    
    pub fn encode(&self) -> String {
        match json::encode(self) {
            Ok(s)  => s,
            Err(e) => panic!("packed decode failed: {}", e),
        }
    }

    pub fn hash_payload(payload: &[u8]) -> u64 {
        let mut hasher = SipHasher::new();
        payload.hash(&mut hasher);
        hasher.finish()
    }

    pub fn decode(data: &str) -> Result<Packet> {
        let packet: Packet = try!(json::decode(data));
        if packet.hash != Packet::hash_payload(&packet.payload) {
            Err(PacketError::PayloadCorrupted)
        } else {
            Ok(packet)
        }
    }
    
    pub fn len(&self) -> u64 {
        self.payload.len() as u64
    }

    pub fn body(&self) -> String {
        String::from_utf8(self.payload.clone()).unwrap()
    }

    pub fn payload<'pkt>(&'pkt self) -> Iter<'pkt, u8> {
        self.payload.iter()
    }

    pub fn seq(&self) -> u64 {
        match self.flag {
            Flag::Data(seq)    => seq,
            Flag::Ack(seq)     => seq,
            Flag::SyncReq(seq) => seq,
            Flag::Fin(seq)     => seq,
        }
    }
}

#[test]
fn test_everything() {
    let packet = Packet::new(Flag::Data(212), vec![9, 3, 5, 0, 11, 40, 250]);
    let json_packet = packet.encode();
    assert_eq!(Packet::decode(&json_packet).unwrap(), 
               packet);
}

