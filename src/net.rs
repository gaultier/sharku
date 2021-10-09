use crate::message::*;
use crate::tracker::Peer;
use anyhow::{Context, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::convert::TryInto;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn handshake(socket: &mut TcpStream, info_hash: &[u8; 20], addr: &str) -> Result<()> {
    socket
        .write_all(HANDSHAKE)
        .await
        .with_context(|| "Failed to write handshake to peer")?;
    log::debug!("{}: Sent handshake", &addr);

    socket
        .write_all(info_hash)
        .await
        .with_context(|| "Failed to write info_hash to peer")?;
    log::debug!("{}: Sent info_hash", &addr);

    assert_eq!(HANDSHAKE.len(), 28);
    let mut buf = [0u8; HANDSHAKE.len()];
    socket
        .read_exact(&mut buf)
        .await
        .with_context(|| "Failed to read from peer")?;

    if buf[..20] != HANDSHAKE[..20] {
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

    Ok(())
}

pub async fn peer_talk(_peer: Peer, info_hash: [u8; 20], addr: Arc<String>) -> Result<()> {
    log::debug!("{}: Trying to connect", &addr);
    let mut socket = TcpStream::connect(addr.deref()).await?;
    log::debug!("{}: Connected", &addr);

    handshake(&mut socket, &info_hash, &addr).await?;

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
            .with_context(|| format!("{}: Failed to write request", addr_writer))?;

        wr.write_all(&buf)
            .await
            .with_context(|| "Failed to write request to peer")?;
        log::debug!("{}: Sent request", addr_writer);
        Ok::<_, anyhow::Error>(())
    });

    let mut buf = vec![0; BLOCK_LENGTH as usize + 9];
    loop {
        rd.read_exact(&mut buf[..4])
            .await
            .with_context(|| "Failed to read from peer")?;

        log::debug!("{}: Received: data={:?}", &addr, &buf[..4]);

        let advisory_length: usize = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        log::debug!("{}: advisory_length={}", &addr, advisory_length);
        // TODO: ??
        if advisory_length > buf.len() {
            anyhow::bail!(
                "Advisory length is bigger than buffer size: advisory_length={}",
                advisory_length
            );
        }

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
