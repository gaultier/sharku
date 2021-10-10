use anyhow::*;
use memmap::{Mmap, MmapMut};
use std::fs::File;
use std::path::Path;

pub fn open(path: &Path, size: u64) -> Result<MmapMut> {
    let file = File::create(path)
        .with_context(|| format!("Failed to open file as RW: path={}", path.to_string_lossy()))?;
    file.set_len(size)
        .with_context(|| format!("Failed to truncate to size={}", size))?;
    let mmap = unsafe {
        Mmap::map(&file)
            .with_context(|| format!("Failed to mmap: path={}", path.to_string_lossy()))?
    };
    mmap.make_mut()
        .with_context(|| "Failed to obtain a mutable file mmap")
}
