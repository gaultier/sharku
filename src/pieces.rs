use actix::prelude::*;
use bit_vec::BitVec;

use crate::message::{Message as M, BLOCK_LENGTH};

pub struct PiecesActor {
    // have_pieces: BitVec,
    have_chunks: BitVec,
    blocks_per_piece: usize,
}

impl Actor for PiecesActor {
    type Context = Context<Self>;
}

impl Handler<M> for PiecesActor {
    type Result = ();

    fn handle(&mut self, msg: M, _: &mut Context<Self>) -> Self::Result {
        println!("Msg={:?}", msg);
        match msg {
            M::Piece { index, .. } => {
                self.have_chunks
                    .set(index as usize * self.blocks_per_piece, true)
                //  TODO: have_pieces, checksum
            }
            _ => todo!(),
        }
    }
}

impl PiecesActor {
    pub fn new(pieces_count: usize, piece_length: u32) -> Self {
        let blocks_per_piece: usize = piece_length as usize / BLOCK_LENGTH as usize;
        PiecesActor {
            blocks_per_piece,
            have_chunks: BitVec::with_capacity(blocks_per_piece * pieces_count),
        }
    }
}
