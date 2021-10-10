use bit_vec::BitVec;

pub const PEER_ID: &[u8; 20] = b"unpetitnuagebleuvert";
pub const HANDSHAKE: &[u8; 28] = b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
pub const BLOCK_LENGTH: u32 = 16384;

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(BitVec),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        data: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
}

impl Message {
    pub fn tag(&self) -> MessageKind {
        match &self {
            Message::Choke => MessageKind::Choke,
            Message::Unchoke => MessageKind::Unchoke,
            Message::Interested => MessageKind::Interested,
            Message::NotInterested => MessageKind::NotInterested,
            Message::Have(_) => MessageKind::Have,
            Message::Bitfield(_) => MessageKind::Bitfield,
            Message::Request { .. } => MessageKind::Request,
            Message::Piece { .. } => MessageKind::Piece,
            Message::Cancel { .. } => MessageKind::Cancel,
        }
    }
}
