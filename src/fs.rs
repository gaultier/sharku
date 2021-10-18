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
                self.mmap.flush().unwrap();
                // let _ = self.mmap.flush_range(start, data.len()).unwrap();
                // .map_err(|err| warn!("Failed to flush mmapped file: {}", err));
                eprintln!("Flush ok");
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
                "Failed to set file length: path={} len={}",
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
#[cfg(test)]
mod tests {
    use actix::clock::sleep;

    use crate::{
        fs::*,
        message::{Message, BLOCK_LENGTH},
    };
    use std::path::PathBuf;
    use std::{env, io::Read};
    use std::{fs::File, io::Seek, io::SeekFrom};

    #[actix::test]
    async fn file_should_be_written_to_on_piece_message() {
        let mut tmp_path = PathBuf::from(env::temp_dir());
        tmp_path.push("sharku_file_should_be_written_to_on_piece_message");

        std::fs::remove_file(&tmp_path).unwrap();

        let file_actor_addr = FileActor::new(&tmp_path, BLOCK_LENGTH as u64 * 2, BLOCK_LENGTH)
            .unwrap()
            .start();

        let data: [u8; BLOCK_LENGTH as usize] = [42u8; BLOCK_LENGTH as usize];
        file_actor_addr
            .try_send(Message::Piece {
                index: 0,
                begin: BLOCK_LENGTH,
                data: data.to_vec(),
            })
            .unwrap();

        sleep(std::time::Duration::from_secs(1)).await;
        let mut buf: [u8; BLOCK_LENGTH as usize] = [0u8; BLOCK_LENGTH as usize];
        let mut f = File::open(&tmp_path).unwrap();
        f.seek(SeekFrom::Start(BLOCK_LENGTH as u64)).unwrap();
        f.read_exact(&mut buf).unwrap();
        assert_eq!(buf.len(), data.len());
        assert_eq!(&buf, &data);
    }
}
