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
    fn new() -> Self {
        Pieces {
            have_pieces: BitVec::new(),
            have_chunks: BitVec::new(),
        }
    }

    async fn run(rx: &mut Receiver<Message>, tx: Sender<Message>) -> Result<()> {
        loop {
            match rx.recv().await? {
                msg => {
                    tx.send(msg)?;
                }
            }
        }
    }
}
