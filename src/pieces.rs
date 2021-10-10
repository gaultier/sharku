use anyhow::*;
use bit_vec::BitVec;
use tokio::sync::broadcast::Receiver;

use crate::message::Event;

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

    pub async fn run(&mut self, rx: &mut Receiver<Event>) -> Result<()> {
        loop {
            match rx
                .recv()
                .await
                .with_context(|| "Pieces: Failed to recv message")?
            {
                msg => {
                    log::debug!("Pieces: msg={:#?}", &msg);
                }
            }
        }
    }
}
