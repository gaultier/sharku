use anyhow::Result;
use byteorder::{BigEndian, WriteBytesExt};

pub const PEER_ID: &[u8; 20] = b"unpetitnuagebleuvert";
pub const HANDSHAKE: &[u8; 28] = b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
pub const BLOCK_LENGTH: u32 = 16384;

#[derive(Debug)]
pub enum MessageKind {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

#[derive(Debug)]
pub enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have,
    Bitfield,
    Request { index: u32, begin: u32, length: u32 },
    Piece,
    Cancel,
}

impl Message {
    pub fn to_bytes(&self, buf: &mut Vec<u8>) -> Result<()> {
        match self {
            &Message::Request {
                begin,
                index,
                length,
            } => {
                WriteBytesExt::write_u8(buf, MessageKind::Request as u8)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, index)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, begin)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, length)?;
            }
            _ => todo!(),
        };
        Ok(())
    }
}
