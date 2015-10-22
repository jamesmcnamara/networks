pub struct Packet {
    id: u64,
    payload: Vec<u8>,
}

impl Packet {
    fn new(id: u64, payload: Vec<u8>) -> Packet {
        Packet { id: id, payload: payload }
    }

    fn encode(&self) -> Vec<u8> {
        
    }
}
