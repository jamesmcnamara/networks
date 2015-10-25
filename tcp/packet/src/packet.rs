use error::{Result, PacketError};
use rustc_serialize::json;

#[derive(PartialEq, Debug, RustcEncodable, RustcDecodable)]
pub enum Flag {
    Data(u64),
    Ack(u64),
    Fin,
}


#[derive(PartialEq, Debug, RustcEncodable, RustcDecodable)]
pub struct Packet {
    pub flag: Flag,
    payload: Vec<u8>,
}

impl Packet {
    pub fn new(flag: Flag, payload: Vec<u8>) -> Packet {
        Packet {
            flag: flag,
            payload: payload
        }
    }
    
    pub fn encode(&self) -> String {
        json::encode(self).unwrap()
    }

    pub fn decode(data: &str) -> Result<Packet> {
        Ok(try!(json::decode(data)))
    }
    
    pub fn len(&self) -> u64 {
        self.payload.len() as u64
    }

    pub fn body(&self) -> String {
        String::from_utf8(self.payload.clone()).unwrap()
    }
}

#[test]
fn test_everything() {
    let packet = Packet::new(Flag::Data(212), vec![9, 3, 5, 0, 11, 40, 250]);
    let json_packet = packet.encode();
    assert_eq!(Packet::decode(&json_packet).unwrap(), 
               packet);
}

