pub const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const DEFAULT_UPLOAD_CHUNK_BYTES: u64 = 16 * 1024 * 1024;
const MIN_CHUNK: u64 = 256 * 1024;

pub fn validate_upload_chunk_bytes(chunk: u64) -> anyhow::Result<u64> {
    anyhow::ensure!(
        chunk >= MIN_CHUNK,
        "DRIVE_UPLOAD_CHUNK_BYTES must be at least {MIN_CHUNK}"
    );
    anyhow::ensure!(
        chunk.is_multiple_of(MIN_CHUNK),
        "DRIVE_UPLOAD_CHUNK_BYTES must be a multiple of {MIN_CHUNK}"
    );
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_chunk_below_minimum() {
        assert!(validate_upload_chunk_bytes(255 * 1024).is_err());
    }

    #[test]
    fn rejects_non_multiple_of_256k() {
        assert!(validate_upload_chunk_bytes(300 * 1024).is_err());
    }

    #[test]
    fn accepts_16_mib() {
        assert_eq!(
            validate_upload_chunk_bytes(DEFAULT_UPLOAD_CHUNK_BYTES).unwrap(),
            DEFAULT_UPLOAD_CHUNK_BYTES
        );
    }
}
