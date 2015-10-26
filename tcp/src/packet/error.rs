use std::error::Error;
use std::result;
use std::fmt;
use rustc_serialize::json::{EncoderError, DecoderError};

#[derive(Debug)]
pub enum PacketError {
    EncodeFailed(EncoderError),
    DecodeFailed(DecoderError),
    PayloadCorrupted,
}

impl Error for PacketError {
    fn description(&self) -> &str {
        match *self {
            PacketError::EncodeFailed(_)  => 
                "an error occured while serializing the packet",
            PacketError::DecodeFailed(_)  => 
                "an error occured while deserializing the packet",
            PacketError::PayloadCorrupted =>
                "The hash of the payload did not match the provided hash",
        }
    }
}

impl From<EncoderError> for PacketError {
    fn from(err: EncoderError) -> PacketError {
        PacketError::EncodeFailed(err)   
    }
}

impl From<DecoderError> for PacketError {
    fn from(err: DecoderError) -> PacketError {
        PacketError::DecodeFailed(err)   
    }
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.description(), f)
    }
}

pub type Result<T> = result::Result<T, PacketError>;
