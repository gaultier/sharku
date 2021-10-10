use bit_vec::BitVec;

pub struct Pieces {
    have_pieces: BitVec,
    have_chunks: BitVec,
}

impl Pieces {
    fun new() {
        Pieces{have_pieces: BitVec::new(), have_chunks: BitVec::new()}
    }
}
