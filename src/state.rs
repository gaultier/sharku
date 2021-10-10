pub struct DownloadState {
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
}

impl DownloadState {
    pub fn default() -> Self {
        DownloadState {
            uploaded: 0,
            downloaded: 0,
            left: 0,
        }
    }
}
