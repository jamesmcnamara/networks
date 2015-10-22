pub struct Packet {
    id: u64,
    payload: Vec<u8>,
}

impl Packet {
    fn new(id: u64, payload: Vec<u8>) -> Packet {
        Packet { id: id, payload: payload }
    }

    fn encode(&self) -> Iterator<u8> {
        return id.to_string().bytes().chain(self.
    }
}
