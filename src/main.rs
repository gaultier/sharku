use anyhow::{Context, Result};
use bendy::decoding::FromBencode;
use bendy::decoding::Object;
use bendy::decoding::{Error, ResultExt};
use std::{fs::File, io::Read};

#[derive(Debug, Eq, PartialEq)]
struct TorrentFile {
    announce: String,
}

impl FromBencode for TorrentFile {
    const EXPECTED_RECURSION_DEPTH: usize = 20;

    fn decode_bencode_object(object: Object) -> std::result::Result<Self, Error> {
        let mut dict = object.try_into_dictionary()?;

        let mut announce: Option<String> = None;
        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"announce", value) => {
                    announce = String::decode_bencode_object(value)
                        .context("announce")
                        .map(Some)?;
                    break;
                }
                _ => continue,
            }
        }

        Ok(TorrentFile {
            announce: announce.ok_or_else(|| Error::missing_field("announce"))?,
        })
    }
}

fn main() -> Result<()> {
    let mut f = File::open("openbsd.torrent").context("Failed to open torrent file")?;
    let mut content = Vec::with_capacity(100_000);
    f.read_to_end(&mut content)
        .context("Failed to read torrent file")?;

    let torrent_file = TorrentFile::from_bencode(&content);

    println!("{:?}", torrent_file);
    Ok(())
}
