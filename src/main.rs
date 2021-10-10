use sharku::fs::*;
use sharku::net::*;
use sharku::torrent_file::*;
use sharku::tracker::*;
use std::sync::Arc;

use anyhow::{Context, Result};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let torrent_file_path = PathBuf::from("debian.torrent");
    let torrent = decode_torrent_from_file(&torrent_file_path)?;
    let files = &torrent.info.files;
    if files.is_none() || files.as_ref().map(|files| files.len()).unwrap_or(0) != 1 {
        anyhow::bail!("Expected exactly one file, got: {:?}", files);
    }
    let files = files.as_ref().unwrap();
    let file = &files[0];
    if file.path.len() != 1 {
        anyhow::bail!("Expected exactly one path, got: {:?}", file.path);
    }
    let file_path = &file.path[0];
    let file_path = PathBuf::from(file_path);
    let _f = open(&file_path)?;

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

    for peer in peers.into_iter().take(4) {
        // FIXME
        tokio::spawn(async move {
            let addr = Arc::new(format!("{}:{}", peer.ip, peer.port));
            let _ = peer_talk(peer, info_hash, addr.clone())
                .await
                .map_err(|err| {
                    log::warn!("{}: Err: {}", &addr, err);
                });
        });
    }
    let notify = tokio::sync::Notify::new();
    notify.notified().await;
    Ok(())
}
