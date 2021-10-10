use crate::message::*;
use anyhow::{Context, Result};
use bit_vec::BitVec;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::convert::TryInto;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_MESSAGE_LEN: usize = BLOCK_LENGTH as usize + 1 + 4 + 4;

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

impl Message {
    fn size(&self) -> u32 {
        match &self {
            Message::Have(_) => 4,
            Message::Bitfield(bytes) => bytes.len() as u32,
            Message::Request { .. } => 4 + 4 + 4,
            Message::Piece { data, .. } => 4 + 4 + data.len() as u32,
            Message::Cancel { .. } => 4 + 4 + 4,
            _ => 0,
        }
    }

    fn write(&self, buf: &mut [u8]) -> Result<()> {
        let mut cursor = Cursor::new(buf);
        WriteBytesExt::write_u32::<BigEndian>(&mut cursor, self.size())?;
        WriteBytesExt::write_u8(&mut cursor, self.tag() as u8)?;

        match &self {
            Message::Have(piece) => {
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *piece)?;
            }
            Message::Bitfield(bytes) => {
                std::io::Write::write_all(&mut cursor, &bytes.to_bytes())?;
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *index)?;
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *begin)?;
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *length)?;
            }
            Message::Piece { index, begin, data } => {
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *index)?;
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *begin)?;
                std::io::Write::write_all(&mut cursor, data)?;
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *index)?;
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *begin)?;
                WriteBytesExt::write_u32::<BigEndian>(&mut cursor, *length)?;
            }
            _ => {}
        };
        Ok(())
    }
}

pub async fn peer_talk(info_hash: [u8; 20], addr: Arc<String>) -> Result<()> {
    log::debug!("{}: Trying to connect", &addr);
    let mut socket = TcpStream::connect(addr.deref()).await?;
    log::debug!("{}: Connected", &addr);

    handshake(&mut socket, &info_hash, &addr).await?;

    // Interested
    let mut buf_writer = vec![0; MAX_MESSAGE_LEN];
    Message::Interested
        .write(&mut buf_writer)
        .with_context(|| "Failed to serialize Message::Interested")?;

    socket
        .write_all(&buf_writer)
        .await
        .with_context(|| "Failed to send Message::Interested")?;
    log::debug!("{}: Sent Message::Interested", &addr);

    // Choke
    Message::Choke
        .write(&mut buf_writer)
        .with_context(|| "Failed to serialize Message::Choke")?;

    socket
        .write_all(&buf_writer)
        .await
        .with_context(|| "Failed to send Message::Choke")?;
    log::debug!("{}: Sent Message::Choke", &addr);

    let (mut rd, mut wr) = io::split(socket);

    let addr_writer = addr.clone();
    let _write_task = tokio::spawn(async move {
        let msg = Message::Request {
            index: 0,
            begin: 0,
            length: BLOCK_LENGTH,
        };
        msg.write(&mut buf_writer)
            .with_context(|| "Failed to serialize Message::Request")?;

        wr.write_all(&buf_writer)
            .await
            .with_context(|| "Failed to send Message::Request")?;
        log::debug!("{}: Sent Message::Request", &addr_writer);

        Ok::<_, anyhow::Error>(())
    });

    let mut buf = vec![0; MAX_MESSAGE_LEN];
    loop {
        rd.read_exact(&mut buf[..4])
            .await
            .with_context(|| "Failed to read from peer")?;

        log::debug!("{}: Received: data={:?}", &addr, &buf[..4]);

        let advisory_length: usize = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        log::debug!("{}: advisory_length={}", &addr, advisory_length);
        if advisory_length > buf.len() {
            anyhow::bail!(
                "Advisory length is bigger than buffer size: advisory_length={}",
                advisory_length
            );
        }

        // Keep-alive, ignore
        if advisory_length == 0 {
            continue;
        }

        rd.read_exact(&mut buf[..advisory_length])
            .await
            .with_context(|| "Failed to read from peer")?;
        let msg = parse_message(&mut buf[..advisory_length])?;
        log::debug!("{}: msg={:?}", &addr, &msg);
    }
}

fn parse_message(buf: &mut [u8]) -> Result<Message> {
    assert!(!buf.is_empty());
    assert!(buf.len() < MAX_MESSAGE_LEN);
    match buf {
        [] => unreachable!(),
        [k, ..] if *k == MessageKind::Choke as u8 => Ok(Message::Choke),
        [k, ..] if *k == MessageKind::Unchoke as u8 => Ok(Message::Unchoke),
        [k, ..] if *k == MessageKind::Interested as u8 => Ok(Message::Interested),
        [k, ..] if *k == MessageKind::NotInterested as u8 => Ok(Message::NotInterested),
        [k, ..] if *k == MessageKind::Have as u8 => {
            let mut cursor = Cursor::new(&buf[1..]); // Skip tag
            Ok(Message::Have(ReadBytesExt::read_u32::<BigEndian>(
                &mut cursor,
            )?))
        }
        [k, ..] if *k == MessageKind::Bitfield as u8 => {
            Ok(Message::Bitfield(BitVec::from_bytes(&buf[1..])))
        }
        [k, ..] if *k == MessageKind::Request as u8 => {
            let mut cursor = Cursor::new(&buf[1..]); // Skip tag
            Ok(Message::Request {
                index: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                begin: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                length: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
            })
        }
        [k, ..] if *k == MessageKind::Piece as u8 => {
            let len = buf.len();
            let mut cursor = Cursor::new(&buf[1..]); // Skip tag
            let index = ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?;
            let begin = ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?;
            let position = cursor.position().min(len as u64) as usize;
            Ok(Message::Piece {
                index,
                begin,
                data: cursor.into_inner()[position..].to_owned(),
            })
        }
        [k, ..] if *k == MessageKind::Cancel as u8 => {
            let mut cursor = Cursor::new(&buf[1..]); // Skip tag
            Ok(Message::Cancel {
                index: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                begin: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
                length: ReadBytesExt::read_u32::<BigEndian>(&mut cursor)?,
            })
        }
        _ => anyhow::bail!("Unkown message: {:?}", buf),
    }
}

#[cfg(test)]
mod tests {
    use crate::{message::Message, message::MessageKind, net::parse_message};

    #[test]
    fn parse_message_bitfield() -> Result<(), String> {
        match parse_message(&mut [MessageKind::Bitfield as u8, 0b0000_0001, 0b1000_0010]) {
            Ok(Message::Bitfield(bytes))
                if bytes.eq_vec(&[
                    false, false, false, false, false, false, false, true, true, false, false,
                    false, false, false, true, false,
                ]) =>
            {
                Ok(())
            }
            other => Err(format!("Got {:#?}", other)),
        }
    }

    #[test]
    fn parse_message_request() {
        let mut bytes = vec![MessageKind::Request as u8];
        bytes.extend_from_slice(&u32::to_be_bytes(0xcafe));
        bytes.extend_from_slice(&u32::to_be_bytes(0xabcd));
        bytes.extend_from_slice(&u32::to_be_bytes(0xef12));
        assert_eq!(
            parse_message(&mut bytes).unwrap(),
            Message::Request {
                index: 0xcafe,
                begin: 0xabcd,
                length: 0xef12,
            }
        );
    }

    #[test]
    fn parse_message_piece() {
        let mut bytes = vec![MessageKind::Piece as u8];
        bytes.extend_from_slice(&u32::to_be_bytes(0xcafe));
        bytes.extend_from_slice(&u32::to_be_bytes(0xabcd));
        bytes.extend_from_slice(&[7, 8, 9, 10, 11]);
        assert_eq!(
            parse_message(&mut bytes).unwrap(),
            Message::Piece {
                index: 0xcafe,
                begin: 0xabcd,
                data: vec![7, 8, 9, 10, 11],
            }
        );
    }

    #[test]
    fn parse_message_cancel() {
        let mut bytes = vec![MessageKind::Cancel as u8];
        bytes.extend_from_slice(&u32::to_be_bytes(0xcafe));
        bytes.extend_from_slice(&u32::to_be_bytes(0xabcd));
        bytes.extend_from_slice(&u32::to_be_bytes(0xef12));
        assert_eq!(
            parse_message(&mut bytes).unwrap(),
            Message::Cancel {
                index: 0xcafe,
                begin: 0xabcd,
                length: 0xef12,
            }
        );
    }
}
