use anyhow::*;
use memmap::{Mmap, MmapMut};
use std::fs::File;
use std::path::Path;

pub fn open(path: &Path, size: u64) -> Result<MmapMut> {
    let file = File::create(path).with_context(|| "Failed to open file as RW")?;
    file.set_len(size)
        .with_context(|| format!("Failed to truncate to size={}", size))?;
    let mmap = unsafe { Mmap::map(&file).with_context(|| "Failed to mmap")? };
    mmap.make_mut()
        .with_context(|| "Failed to obtain a mutable file mmap")
}
