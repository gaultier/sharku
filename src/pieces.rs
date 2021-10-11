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

    pub async fn run(&mut self, tx: &mut Sender<Event>, rx: &mut Receiver<Event>) -> Result<()> {
        for i in 0..5 {
            tx.send(Event {
                peer_id: 3,
                message: Message::Request {
                    index: i,
                    begin: 0,
                    length: BLOCK_LENGTH,
                },
            })
            .with_context(|| "Failed to send message")?;
        }

        loop {
            match rx.recv().await.with_context(|| "Failed to recv message")? {
                msg => {
                    log::debug!("msg={:#?}", &msg);
                }
            }
        }
    }
}
