use anyhow::{Context, Result};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_bencode::de;
use serde_bytes::ByteBuf;
use std::fs::File as F;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Node(String, i64);

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
    pub path: Vec<String>,
    length: i64,
    md5sum: Option<String>,
}

#[derive(Derivative)]
#[derivative(Debug)]
#[derive(Deserialize, Serialize)]
pub struct Info {
    pub name: String,
    #[derivative(Debug = "ignore")]
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    md5sum: Option<String>,
    pub length: Option<usize>,
    pub files: Option<Vec<File>>,
    private: Option<u8>,
    path: Option<Vec<String>>,
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

impl Info {
    pub fn pieces_count(&self) -> usize {
        assert!(self.piece_length > 0);
        // Div ceil
        let piece_length = self.piece_length as usize;
        let length = self.length.unwrap_or(0);
        // Div ceil
        let pieces_count = (length + piece_length - 1) / piece_length;
        // Pad remaining bits of the last byte
        let res = (pieces_count + 8 - 1) / 8;
        res * 8
    }
}

#[cfg(test)]
mod tests {
    use serde_bytes::ByteBuf;

    use crate::torrent_file::Info;

    #[test]
    fn compute_pieces_count() {
        let info = Info {
            name: String::new(),
            pieces: ByteBuf::new(),
            piece_length: 262144,
            md5sum: None,
            length: Some(395313152),
            files: None,
            private: None,
            path: None,
            root_hash: None,
        };
        assert_eq!(info.pieces_count(), 1512);
    }
}

#[derive(Debug, Deserialize)]
pub struct Torrent {
    pub info: Info,
    pub announce: Option<String>,
    nodes: Option<Vec<Node>>,
    encoding: Option<String>,
    httpseeds: Option<Vec<String>>,
    #[serde(rename = "announce-list")]
    announce_list: Option<Vec<Vec<String>>>,
    #[serde(rename = "creation date")]
    creation_date: Option<i64>,
    #[serde(rename = "comment")]
    comment: Option<String>,
    #[serde(rename = "created by")]
    created_by: Option<String>,
}

pub fn decode_torrent_from_file(file_name: &Path) -> Result<Torrent> {
    let mut f = F::open(file_name).context("Failed to open torrent file")?;
    let mut content = Vec::with_capacity(100_000);
    f.read_to_end(&mut content)
        .context("Failed to read torrent file")?;

    de::from_bytes::<Torrent>(&content).context("Failed to parse torrent file")
}
