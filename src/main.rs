use sharku::torrent_file::*;

use anyhow::{Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use serde_bencode::de;
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::convert::TryInto;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Deref;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const PEER_ID: &str = "unpetitnuagebleuvert";
const HANDSHAKE: &[u8; 28] = b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";

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

fn info_hash(torrent: &Torrent) -> Result<[u8; 20]> {
    let info_bytes =
        serde_bencode::to_bytes(&torrent.info).context("Failed to serialize torrent info")?;
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    Ok(hasher.finalize().into())
}

async fn tracker_start(
    client: reqwest::Client,
    torrent: &Torrent,
    download_state: &DownloadState,
    port: u16,
    info_hash: &[u8; 20],
) -> Result<Vec<Peer>> {
    let url = torrent
        .announce
        .as_ref()
        .context("Missing announce URL in the torrent file")?;

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
    log::debug!("url={}", url);

    let res = client
        .get(req)
        .send()
        .await
        .context("Failed to contact tracker")?
        .bytes()
        .await?;

    let decoded_res: TrackerResponse = de::from_bytes::<TrackerResponse>(&res)
        .with_context(|| "Failed to deserialize tracker response")?;

    Ok(decode_compact_peers(decoded_res.peers.as_slice())?)
}

fn as_u16_be(array: &[u8; 2]) -> u16 {
    ((array[0] as u16) << 8) + (array[1] as u16)
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

async fn peer_talk(peer: Peer, info_hash: [u8; 20]) -> Result<()> {
    let addr = Arc::new(format!("{}:{}", peer.ip, peer.port));
    log::debug!("{}: Trying to connect", &addr);
    let mut socket = TcpStream::connect(addr.deref()).await?;
    log::debug!("{}: Connected", &addr);

    socket
        .write_all(HANDSHAKE)
        .await
        .with_context(|| "Failed to write handshake to peer")?;
    log::debug!("{}: Sent handshake", &addr);

    socket
        .write_all(&info_hash)
        .await
        .with_context(|| "Failed to write info_hash to peer")?;
    log::debug!("{}: Sent info_hash", &addr);

    let padding_len = 8;
    let mut buf = vec![0; 1024];
    let n = socket
        .read_exact(&mut buf[..HANDSHAKE.len()])
        .await
        .with_context(|| "Failed to read from peer")?;

    if buf[..n - padding_len] != HANDSHAKE[..n - padding_len] {
        log::warn!(
            "{}: Received wrong handshake:\nexpected=\t{:?}\ngot=\t{:?}",
            &addr,
            &HANDSHAKE[..n - padding_len],
            &buf[..n - padding_len]
        );
        return Ok(());
    }
    log::debug!("{}: Validated handshake", &addr);

    let (mut rd, mut _wr) = io::split(socket);

    // Write data in the background
    // let addr_writer = addr.clone();
    // let _write_task = tokio::spawn(async move {
    //     wr.write_all(&info_hash)
    //         .await
    //         .with_context(|| "Failed to write info_hash to peer")?;
    //     log::debug!("{}: Sent info_hash", &addr_writer);
    //     Ok::<_, anyhow::Error>(())
    // });

    loop {
        let n = rd
            .read_exact(&mut buf[..HANDSHAKE.len()])
            .await
            .with_context(|| "Failed to read from peer")?;

        log::debug!("{}: Received: n={} data={:?}", &addr, n, &buf[..n]);
        if n == 0 {
            break;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let torrent_file_path = PathBuf::from("debian.torrent");
    let torrent = decode_torrent_from_file(&torrent_file_path)?;

    let client = reqwest::Client::new();
    let download_state = DownloadState {
        left: torrent.info.length.unwrap_or(0) as usize,
        ..DownloadState::default()
    };
    let port: u16 = 6881;
    let info_hash = info_hash(&torrent)?;
    let peers = tracker_start(client, &torrent, &download_state, port, &info_hash)
        .await
        .context("Failed to start download with tracker")?;

    let tasks = peers
        .into_iter()
        .map(|p| tokio::spawn(async move { peer_talk(p, info_hash).await }))
        .collect::<Vec<_>>();

    join_all(tasks).await;
    Ok(())
}
