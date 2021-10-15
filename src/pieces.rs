use anyhow::*;
use bit_vec::BitVec;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;

use crate::message::BLOCK_LENGTH;
use crate::message::{Event, Message};

pub struct Pieces {
    have_pieces: BitVec,
    have_chunks: BitVec,
}

impl Pieces {
    pub fn new() -> Self {
        Pieces {
            have_pieces: BitVec::new(),
            have_chunks: BitVec::new(),
        }
    }
}
