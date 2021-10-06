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

const PEER_ID: &'static str = "unpetitnuagebleuvert";

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

struct DownloadState {
    uploaded: usize,
    downloaded: usize,
    left: usize,
}

impl DownloadState {
    fn new() -> Self {
        DownloadState {
            uploaded: 0,
            downloaded: 0,
            left: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Peer {
    // #[serde(rename = "peer id")]
    // id: [u8; 20],
    port: u16,
    ip: String,
}

#[derive(Debug, Deserialize)]
struct TrackerResponse {
    #[serde(rename = "failure reason")]
    failure_reason: Option<String>,
    interval: Option<usize>,
    peers: Option<Vec<Peer>>,
}

async fn tracker_start(
    client: reqwest::Client,
    torrent: &Torrent,
    download_state: &DownloadState,
    port: u16,
) -> Result<()> {
    let url = torrent
        .announce
        .as_ref()
        .context("Missing announce URL in the torrent file")?;

    let info_bytes =
        serde_bencode::to_bytes(&torrent.info).context("Failed to serialize torrent info")?;
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    let info_hash = hasher.finalize();
    println!("{:x?}", info_hash);

    let info_hash_percent_encoded = info_hash
        .iter()
        .map(|b| format!("%{:02X}", b))
        .collect::<String>();

    let query = format!(
        "port={}&peer_id={}&left={}&uploaded={}&downloaded={}&info_hash={}",
        port,
        PEER_ID,
        download_state.left,
        download_state.uploaded,
        download_state.downloaded,
        info_hash_percent_encoded
    );
    let req = format!("{}?{}", url, query);
    println!("url={}", url);

    let res = client
        .get(req)
        .send()
        .await
        .context("Failed to contact tracker")?
        .text()
        .await?;

    println!("Res={}", res);
    let decoded_res: TrackerResponse = de::from_str::<TrackerResponse>(res.as_str())
        .with_context(|| "Failed to deserialize tracker response")?;
    println!("Res={:#?}", decoded_res);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut f = F::open("debian.torrent").context("Failed to open torrent file")?;
    let mut content = Vec::with_capacity(100_000);
    f.read_to_end(&mut content)
        .context("Failed to read torrent file")?;

    let torrent = de::from_bytes::<Torrent>(&content).context("Failed to parse torrent file")?;
    // println!("{:#?}", &torrent);

    let client = reqwest::Client::new();
    let download_state = DownloadState {
        downloaded: 0,
        uploaded: 0,
        left: torrent.info.length.unwrap_or(0) as usize,
    };
    let port: u16 = 6881;
    tracker_start(client, &torrent, &download_state, port)
        .await
        .context("Failed to start download with tracker")?;

    Ok(())
}
