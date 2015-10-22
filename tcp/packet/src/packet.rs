use error::{Result, PacketError};

const HEADER_LEN: usize = 16;

#[derive(PartialEq, Debug)]
pub struct Packet {
    id: u64,
    length: u64,
    payload: Vec<u8>,
}

fn u8s_to_u64(u8s: &[u8]) -> u64 {
    u8s.iter().fold(0u64, |sum, num| (sum << 8) + *num as u64)
}

fn u64_to_u8s(num: u64) -> Vec<u8> {
    (0..8).map(|i| (num >> (i * 8)) as u8).rev().collect()
}

impl Packet {
    fn new(id: u64, payload: Vec<u8>) -> Packet {
        Packet {
            id: id,
            length: payload.len() as u64,
            payload: payload
        }
    }

    fn encode(&self) -> Vec<u8> {
        u64_to_u8s(self.id)
            .iter()
            .chain(u64_to_u8s(self.length).iter())
            .chain(self.payload.iter())
            .map(|&b| b)
            .collect()
    }

    fn decode(packet: &[u8]) -> Result<Packet> {
        let (header, payload) = (&packet[..HEADER_LEN], &packet[HEADER_LEN..]);
        let (id, length) = match &header.chunks(8).map(u8s_to_u64).collect::<Vec<_>>()[..] {
            [id, length] => (id, length),
            _            => return Err(PacketError::HeaderCorrupted) 
        };
        if payload.len() as u64 == length {
            Ok(Packet::new(id, payload.to_vec()))
        } else {
            Err(PacketError::PayloadSizeError)
        }
    }
}

#[test]
fn test_everything() {
    let packet = Packet::new(782, vec![9, 3, 5, 0, 11, 40, 250]);
    assert_eq!(Packet::decode(&packet.encode()).unwrap(), packet);
}

#[test]
fn test_u64_to_u8s() {
    assert_eq!(u64_to_u8s(10u64), [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 10u8]); 
    assert_eq!(u64_to_u8s(511u64), [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 255u8]); 
}

#[test]
fn test_u8s_to_u64() {
    assert_eq!(u8s_to_u64(&[0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 10u8]), 10u64); 
    assert_eq!(u8s_to_u64(&[0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 255u8]), 511u64); 
}
