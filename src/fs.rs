use anyhow::*;
use memmap::MmapMut;
use std::fs::OpenOptions;
use std::path::Path;

pub fn open(path: &Path, size: u64) -> Result<MmapMut> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .with_context(|| {
            format!(
                "Failed to open file as RW: path={} len={}",
                path.to_string_lossy(),
                size
            )
        })?;

    file.set_len(size).with_context(|| {
        format!(
            "Failed to open file as RW: path={} len={}",
            path.to_string_lossy(),
            size
        )
    })?;

    let mmap = unsafe {
        MmapMut::map_mut(&file)
            .with_context(|| format!("Failed to mmap: path={}", path.to_string_lossy()))?
    };
    Ok(mmap)
}
