use crate::message::PEER_ID;
use crate::state::DownloadState;
use crate::torrent_file::Torrent;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_bencode::de;
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::convert::TryInto;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Deserialize)]
pub struct Peer {
    pub port: u16,
    pub ip: IpAddr,
}

#[derive(Debug, Deserialize)]
pub struct TrackerResponse {
    #[serde(rename = "failure reason")]
    pub failure_reason: Option<String>,
    pub interval: Option<usize>,
    pub peers: ByteBuf,
}

pub fn info_hash(torrent: &Torrent) -> Result<[u8; 20]> {
    let info_bytes =
        serde_bencode::to_bytes(&torrent.info).context("Failed to serialize torrent info")?;
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    Ok(hasher.finalize().into())
}

pub async fn tracker_start(
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
        String::from_utf8_lossy(PEER_ID),
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

fn decode_compact_peers(compact_peers: &[u8]) -> Result<Vec<Peer>> {
    if compact_peers.len() % 6 != 0 {
        anyhow::bail!(
            "The compact peers list has the wrong size: {}",
            compact_peers.len()
        );
    }
    Ok(compact_peers
        .chunks(6)
        .map(|bytes| {
            let ip_bytes: [u8; 4] = bytes[0..4]
                .try_into()
                .with_context(|| "Failed to get 4 bytes for the peer ip")
                .unwrap();
            let port_bytes: [u8; 2] = bytes[4..6]
                .try_into()
                .with_context(|| "Failed to get 2 bytes for the peer port")
                .unwrap();
            Peer {
                ip: IpAddr::V4(Ipv4Addr::from(ip_bytes)),
                port: u16::from_be_bytes(port_bytes),
            }
        })
        .collect())
}
