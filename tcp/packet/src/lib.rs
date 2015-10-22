use std::mem;

#[derive(PartialEq)]
pub struct Packet {
    id: u64,
    length: u64,
    payload: Vec<u8>,
}

fn convert_num<T, R>(number: T) -> R {
    unsafe {
        mem::transmute::<T, R>(number)
    }
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
        convert_num::<_, [u8; 8]>(self.id)
            .iter()
            .chain(convert_num::<_, [u8; 8]>(self.length).iter())
            .chain(self.payload.iter())
            .map(|&b| b)
            .collect()
    }

    fn decode(packet: Vec<u8>) -> Packet {
        let mut pack_iter = packet.iter();
        let id = convert_num::<[u8; 8], u64>(pack_iter.take(8).map(|&b| b).collect());
        let length = convert_num::<[u8; 8], u64>(pack_iter.take(8).map(|&b| b).collect());
        let payload: Vec<u8> = pack_iter.map(|&b| b).collect();
        Packet::new(id, payload)
    }
}

#[test]
fn test_everything() {
    let packet = Packet::new(782, vec![9, 3, 5, 0, 11, 40, 250]);
    assert_eq!(Packet::decode(packet.encode()), packet);
}

