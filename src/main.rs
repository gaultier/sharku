extern crate serde;
extern crate serde_bencode;
#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;

use anyhow::{Context, Result};
use serde_bencode::de;
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::fs::File as F;
use std::io::Read;

#[derive(Debug, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize, Serialize)]
struct File {
    path: Vec<String>,
    length: i64,
    #[serde(default)]
    md5sum: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    name: String,
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    piece_length: i64,
    #[serde(default)]
    md5sum: Option<String>,
    #[serde(default)]
    length: Option<i64>,
    #[serde(default)]
    files: Option<Vec<File>>,
    #[serde(default)]
    private: Option<u8>,
    #[serde(default)]
    path: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Torrent {
    info: Info,
    #[serde(default)]
    announce: Option<String>,
    #[serde(default)]
    nodes: Option<Vec<Node>>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    httpseeds: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    announce_list: Option<Vec<Vec<String>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    creation_date: Option<i64>,
    #[serde(rename = "comment")]
    comment: Option<String>,
    #[serde(default)]
    #[serde(rename = "created by")]
    created_by: Option<String>,
}

async fn tracker_start(client: reqwest::Client, torrent: &Torrent) -> Result<()> {
    let url = torrent
        .announce
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing annouce URL in the torrent file"))?;

    let req = client.get(url);

    let res = req.send().await?.text().await?;

    println!("Res={:#?}", res);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut f = F::open("openbsd.torrent").context("Failed to open torrent file")?;
    let mut content = Vec::with_capacity(100_000);
    f.read_to_end(&mut content)
        .context("Failed to read torrent file")?;

    let torrent = de::from_bytes::<Torrent>(&content).context("Failed to parse torrent file")?;
    println!("{:#?}", &torrent);

    let info_bytes =
        serde_bencode::to_bytes(&torrent.info).context("Failed to serialize torrent info")?;
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    let info_hash = hasher.finalize();
    println!("info_hash={:?}", info_hash);

    let client = reqwest::Client::new();
    tracker_start(client, &torrent)
        .await
        .context("Failed to contact tracker")?;
    Ok(())
}
