use anyhow::*;
use memmap::{Mmap, MmapMut};
use std::fs::File;
use std::path::Path;

pub fn open(path: &Path) -> Result<MmapMut> {
    let file = File::create(path).with_context(|| "Failed to open file as RW")?;
    let mmap = unsafe { Mmap::map(&file).with_context(|| "Failed to mmap")? };
    mmap.make_mut()
        .with_context(|| "Failed to obtain a mutable file mmap")
}
