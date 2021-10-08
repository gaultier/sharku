use sharku::torrent_file::*;

use anyhow::{Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use serde_bencode::de;
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::convert::TryInto;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::str;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const PEER_ID: &'static str = "unpetitnuagebleuvert";

struct DownloadState {
    uploaded: usize,
    downloaded: usize,
    left: usize,
}

impl DownloadState {
    fn default() -> Self {
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
    ip: IpAddr,
}

#[derive(Debug, Deserialize)]
struct TrackerResponse {
    #[serde(rename = "failure reason")]
    failure_reason: Option<String>,
    interval: Option<usize>,
    peers: ByteBuf,
}

async fn tracker_start(
    client: reqwest::Client,
    torrent: &Torrent,
    download_state: &DownloadState,
    port: u16,
) -> Result<Vec<Peer>> {
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
        "port={}&compact=1&peer_id={}&left={}&uploaded={}&downloaded={}&info_hash={}",
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
        .bytes()
        .await?;

    println!("Res={:?}", res);
    let decoded_res: TrackerResponse = de::from_bytes::<TrackerResponse>(&res)
        .with_context(|| "Failed to deserialize tracker response")?;
    println!("Res={:#?}", decoded_res);

    Ok(decode_compact_peers(decoded_res.peers.as_slice())?)
}

fn as_u16_be(array: &[u8; 2]) -> u16 {
    ((array[0] as u16) << 8) + ((array[1] as u16) << 0)
}

fn decode_compact_peers(compact_peers: &[u8]) -> Result<Vec<Peer>> {
    if compact_peers.len() % 6 != 0 {
        return Err(anyhow::anyhow!("The compact peers list has the wrong size"));
    }
    Ok(compact_peers
        .chunks(6)
        .map(|bytes| {
            let ip_bytes: [u8; 4] = bytes[0..4]
                .try_into()
                .with_context(|| "Failed to get 4 bytes for the peer ip")
                .unwrap();
            let port_bytes: &[u8; 2] = bytes[4..6]
                .try_into()
                .with_context(|| "Failed to get 4 bytes for the peer ip")
                .unwrap();
            Peer {
                ip: IpAddr::V4(Ipv4Addr::from(ip_bytes)),
                port: as_u16_be(port_bytes),
            }
        })
        .collect())
}

async fn peer_talk(peer: Peer) -> Result<()> {
    let addr = format!("{}:{}", peer.ip, peer.port);
    let socket = TcpStream::connect(&addr).await?;
    println!("Connected to {}", &addr);
    let (mut rd, mut wr) = io::split(socket);

    // Write data in the background
    let _write_task = tokio::spawn(async move {
        wr.write_all(b"hello\r\n")
            .await
            .with_context(|| "Failed to write to peer")
    });

    let mut buf = vec![0; 1024];

    loop {
        let n = rd
            .read(&mut buf)
            .await
            .with_context(|| "Failed to read from peer")?;

        if n == 0 {
            break;
        }

        println!("GOT {:?}", &buf[..n]);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let torrent_file_path = PathBuf::from("debian.torrent");
    let torrent = decode_torrent_from_file(&torrent_file_path)?;

    let client = reqwest::Client::new();
    let download_state = DownloadState {
        left: torrent.info.length.unwrap_or(0) as usize,
        ..DownloadState::default()
    };
    let port: u16 = 6881;
    let peers = tracker_start(client, &torrent, &download_state, port)
        .await
        .context("Failed to start download with tracker")?;

    let tasks = peers
        .into_iter()
        .map(|p| {
            println!("Peer: {:#?}", p);
            tokio::spawn(async move { peer_talk(p).await })
        })
        .collect::<Vec<_>>();

    join_all(tasks).await;
    Ok(())
}
