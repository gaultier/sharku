use sharku::net::*;
use sharku::torrent_file::*;
use sharku::tracker::*;

use anyhow::{Context, Result};
use futures::future::join_all;
use sha1::{Digest, Sha1};
use std::path::PathBuf;

fn info_hash(torrent: &Torrent) -> Result<[u8; 20]> {
    let info_bytes =
        serde_bencode::to_bytes(&torrent.info).context("Failed to serialize torrent info")?;
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    Ok(hasher.finalize().into())
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
        .map(|p| {
            tokio::spawn(async move {
                peer_talk(p, info_hash).await.map_err(|err| {
                    log::trace!("Error: {}", err);
                    err
                })
            })
        })
        .collect::<Vec<_>>();

    join_all(tasks).await;
    Ok(())
}
