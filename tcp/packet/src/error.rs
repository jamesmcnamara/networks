use std::error::Error;
use std::result;
use std::fmt;


#[derive(Debug)]
pub enum PacketError {
    HeaderCorrupted,
    PayloadSizeError,
}

impl Error for PacketError {
    fn description(&self) -> &str {
        match *self {
            PacketError::HeaderCorrupted  => "packet header corrupted",
            PacketError::PayloadSizeError => "the payload is not the stated length",
        }
    }
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.description(), f)
    }
}

pub type Result<T> = result::Result<T, PacketError>;
