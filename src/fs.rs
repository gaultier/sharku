use actix::prelude::*;
use anyhow::{Context as Ctx, Result};
use memmap::MmapMut;
use std::fs::OpenOptions;
use std::path::Path;

use crate::message::Message as M;

impl Message for M {
    type Result = ();
}

pub struct FileActor {
    file: std::fs::File,
    mmap: MmapMut,
    piece_length: u32,
}

impl Actor for FileActor {
    type Context = Context<Self>;
}

impl Handler<M> for FileActor {
    type Result = ();

    fn handle(&mut self, msg: M, _: &mut Context<Self>) -> Self::Result {
        println!("Msg={:?}", msg);
        match msg {
            M::Piece { index, begin, data } => {
                // TODO: checks

                let start = index as usize * self.piece_length as usize + begin as usize;
                let end = start as usize + data.len();
                self.mmap[start..end].copy_from_slice(data.as_slice());
            }
            _ => todo!(),
        }
    }
}

impl FileActor {
    pub fn new(path: &Path, file_length: u64, piece_length: u32) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .with_context(|| {
                format!(
                    "Failed to open file as RW: path={} len={}",
                    path.to_string_lossy(),
                    file_length
                )
            })?;

        file.set_len(file_length).with_context(|| {
            format!(
                "Failed to open file as RW: path={} len={}",
                path.to_string_lossy(),
                file_length
            )
        })?;

        let mmap = unsafe {
            MmapMut::map_mut(&file)
                .with_context(|| format!("Failed to mmap: path={}", path.to_string_lossy()))?
        };

        Ok(FileActor {
            file,
            piece_length,
            mmap,
        })
    }
}
