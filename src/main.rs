use sharku::torrent_file::*;

use anyhow::{Context, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures::future::join_all;
use serde::Deserialize;
use serde_bencode::de;
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::convert::TryInto;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Deref;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const PEER_ID: &[u8; 20] = b"unpetitnuagebleuvert";
const HANDSHAKE: &[u8; 28] = b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
const BLOCK_LENGTH: u32 = 16384;

#[derive(Debug)]
enum MessageKind {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

#[derive(Debug)]
enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have,
    Bitfield,
    Request { index: u32, begin: u32, length: u32 },
    Piece,
    Cancel,
}

impl Message {
    fn to_bytes(&self, buf: &mut Vec<u8>) -> Result<()> {
        match self {
            &Message::Request {
                begin,
                index,
                length,
            } => {
                WriteBytesExt::write_u8(buf, MessageKind::Request as u8)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, index)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, begin)?;
                WriteBytesExt::write_u32::<BigEndian>(buf, length)?;
            }
            _ => todo!(),
        };
        Ok(())
    }
}

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
            let port_bytes: &[u8; 2] = bytes[4..6]
                .try_into()
                .with_context(|| "Failed to get 2 bytes for the peer port")
                .unwrap();
            Peer {
                ip: IpAddr::V4(Ipv4Addr::from(ip_bytes)),
                port: ReadBytesExt::read_u16::<BigEndian>(&mut Cursor::new(port_bytes)).unwrap(),
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

    let mut buf = vec![0; 1024];
    socket
        .read_exact(&mut buf[..HANDSHAKE.len()])
        .await
        .with_context(|| "Failed to read from peer")?;

    if buf[..20] != HANDSHAKE[..20] {
        log::warn!(
            "{}: Received wrong handshake:\nexpected=\t{:?}\ngot=\t{:?}",
            &addr,
            &HANDSHAKE[..20],
            &buf[..20]
        );
        anyhow::bail!(
            "{}: Received wrong handshake:\nexpected=\t{:?}\ngot=\t{:?}",
            &addr,
            &HANDSHAKE[..20],
            &buf[..20]
        );
    }
    log::debug!("{}: Validated handshake", &addr);

    socket
        .read_exact(&mut buf[..info_hash.len()])
        .await
        .with_context(|| "Failed to read info_hash")?;
    log::debug!(
        "{}: Received info_hash:{:?}",
        &addr,
        &buf[..info_hash.len()],
    );

    socket
        .write_all(PEER_ID)
        .await
        .with_context(|| "Failed to write peer id")?;
    log::debug!("{}: Sent peer id", &addr);

    socket
        .read_exact(&mut buf[..PEER_ID.len()])
        .await
        .with_context(|| "Failed to read peer id")?;
    log::debug!("{}: Received peer id:{:?}", &addr, &buf[..PEER_ID.len()],);

    // Interested
    socket
        .write_all(&u32::to_be_bytes(1))
        .await
        .with_context(|| "Failed to write size")?;
    socket
        .write_all(&[MessageKind::Interested as u8])
        .await
        .with_context(|| "Failed to write Interested")?;
    log::debug!("{}: Sent interested", &addr);

    // Choke
    socket
        .write_all(&u32::to_be_bytes(1))
        .await
        .with_context(|| "Failed to write size")?;
    socket
        .write_all(&[MessageKind::Choke as u8])
        .await
        .with_context(|| "Failed to write Choke")?;
    log::debug!("{}: Sent choke", &addr);

    let (mut rd, mut wr) = io::split(socket);

    let addr_writer = addr.clone();
    let _write_task = tokio::spawn(async move {
        let mut buf = vec![0; 1024];
        let msg = Message::Request {
            index: 0,
            begin: 0,
            length: BLOCK_LENGTH,
        };
        WriteBytesExt::write_u32::<BigEndian>(&mut buf, 1 + 4 * 3)?;
        msg.to_bytes(&mut buf)
            .with_context(|| format!("{}: Failed to write request", &addr_writer))?;

        wr.write_all(&buf)
            .await
            .with_context(|| "Failed to write request to peer")?;
        log::debug!("{}: Sent request", &addr_writer);
        Ok::<_, anyhow::Error>(())
    });

    loop {
        rd.read_exact(&mut buf[..4])
            .await
            .with_context(|| "Failed to read from peer")?;

        log::debug!("{}: Received: data={:?}", &addr, &buf[..4]);

        let advisory_length: usize = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        log::debug!("{}: advisory_length={}", &addr, advisory_length);
        // TODO: ??
        if advisory_length > BLOCK_LENGTH as usize + 9 {
            log::warn!(
                "Advisory length is bigger than buffer size: advisory_length={}",
                advisory_length
            );
            anyhow::bail!(
                "Advisory length is bigger than buffer size: advisory_length={}",
                advisory_length
            );
        }
        buf.resize(advisory_length, 0);

        rd.read_exact(&mut buf[..advisory_length])
            .await
            .with_context(|| "Failed to read from peer")?;
        let msg = parse_message(&mut buf[..advisory_length])?;
        log::debug!("{}: msg={:?}", &addr, &msg);
    }
}

fn parse_message(buf: &mut [u8]) -> Result<Message> {
    match buf {
        &mut [] => unreachable!(),
        &mut [k, _] if (k & 0xff) == MessageKind::Choke as u8 => Ok(Message::Choke),
        &mut [k, _] if (k & 0xff) == MessageKind::Unchoke as u8 => Ok(Message::Unchoke),
        &mut [k, _] if (k & 0xff) == MessageKind::Interested as u8 => Ok(Message::Interested),
        &mut [k, _] if (k & 0xff) == MessageKind::NotInterested as u8 => Ok(Message::NotInterested),
        &mut [k, _] if (k & 0xff) == MessageKind::Have as u8 => Ok(Message::Have),
        &mut [k, _] if (k & 0xff) == MessageKind::Bitfield as u8 => Ok(Message::Bitfield),
        &mut [k, _] if (k & 0xff) == MessageKind::Request as u8 => {
            let mut cursor = Cursor::new(buf);
            Ok(Message::Request {
                index: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                begin: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                length: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
            })
        }
        &mut [k, _] if (k & 0xff) == MessageKind::Piece as u8 => Ok(Message::Piece),
        &mut [k, _] if (k & 0xff) == MessageKind::Cancel as u8 => Ok(Message::Cancel),
        _ => anyhow::bail!("Unkown message: {:?}", buf),
    }
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
