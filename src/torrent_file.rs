use anyhow::{Context, Result};
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
    path: Vec<String>,
    length: i64,
    md5sum: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Info {
    name: String,
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    piece_length: i64,
    md5sum: Option<String>,
    pub length: Option<i64>,
    files: Option<Vec<File>>,
    private: Option<u8>,
    path: Option<Vec<String>>,
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
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
