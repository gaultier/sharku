use anyhow::*;
use bit_vec::BitVec;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;

use crate::message::Message;

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

    pub async fn run(
        &mut self,
        rx: &mut Receiver<Message>,
        tx: &mut Sender<Message>,
    ) -> Result<()> {
        loop {
            match rx.recv().await? {
                msg => {
                    tx.send(msg)?;
                }
            }
        }
    }
}
