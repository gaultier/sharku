use actix::prelude::*;

use crate::message::Message as M;

pub struct PeerActor {
    choked: bool,
    interested: bool,
    peer_choked: bool,
    peer_interested: bool,
}

impl Actor for PeerActor {
    type Context = Context<Self>;
}

impl Handler<M> for PeerActor {
    type Result = ();

    fn handle(&mut self, msg: M, _: &mut Context<Self>) -> Self::Result {
        println!("Msg={:?}", msg);
        match msg {
            M::Choke => self.peer_choked = true,
            M::Unchoke => self.peer_choked = false,
            M::Interested => self.peer_interested = true,
            M::NotInterested => self.peer_interested = false,
            _ => todo!(),
        }
    }
}

impl PeerActor {
    pub fn new() -> Self {
        PeerActor {
            choked: false,
            interested: false,
            peer_choked: false,
            peer_interested: false,
        }
    }
}
