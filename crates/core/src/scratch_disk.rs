use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone)]
pub struct ScratchThresholds {
    pub min_free_bytes: u64,
    pub resume_free_bytes: u64,
}

pub fn parse_scratch_thresholds_mb(min_mb: u64, resume_mb: u64) -> ScratchThresholds {
    let min_free_bytes = min_mb.saturating_mul(1024 * 1024);
    let mut resume_free_bytes = resume_mb.saturating_mul(1024 * 1024);
    if resume_free_bytes <= min_free_bytes {
        resume_free_bytes = min_free_bytes.saturating_add(256 * 1024 * 1024);
        tracing::warn!(
            min_free_mb = min_mb,
            resume_free_mb = resume_free_bytes / (1024 * 1024),
            "JOB_SCRATCH_RESUME_FREE_MB was <= MIN; bumped resume threshold"
        );
    }
    ScratchThresholds {
        min_free_bytes,
        resume_free_bytes,
    }
}

#[cfg(unix)]
pub fn free_bytes_at(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    };
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path with nul"))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut stat) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(not(unix))]
pub fn free_bytes_at(_path: &Path) -> std::io::Result<u64> {
    Ok(u64::MAX)
}

pub fn is_low(free_bytes: u64, min_free_bytes: u64) -> bool {
    free_bytes < min_free_bytes
}

/// `state`: 0 = normal, 1 = pressured (enter logged).
pub struct DiskPressureGate {
    state: AtomicU8,
}

impl DiskPressureGate {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
        }
    }

    pub fn is_pressured(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }

    pub fn force_pressure(&self) {
        self.state.store(1, Ordering::Release);
    }

    /// Returns true if workers should skip claiming.
    pub fn evaluate(&self, free_bytes: u64, thresholds: &ScratchThresholds) -> bool {
        if free_bytes < thresholds.min_free_bytes {
            if self
                .state
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                tracing::error!(
                    free_bytes,
                    min_free_bytes = thresholds.min_free_bytes,
                    "scratch disk pressure: workers paused"
                );
            }
            return true;
        }
        if self.state.load(Ordering::Acquire) == 1
            && free_bytes >= thresholds.resume_free_bytes
            && self
                .state
                .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            tracing::error!(
                free_bytes,
                resume_free_bytes = thresholds.resume_free_bytes,
                "scratch disk pressure cleared: workers resumed"
            );
        }
        self.state.load(Ordering::Acquire) == 1
    }
}

impl Default for DiskPressureGate {
    fn default() -> Self {
        Self::new()
    }
}

pub fn disk_full_in_message(msg: &str) -> bool {
    let s = msg.to_ascii_lowercase();
    s.contains("no space left on device") || s.contains("errno 28")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bumps_resume_when_not_greater_than_min() {
        let t = parse_scratch_thresholds_mb(512, 512);
        assert!(t.resume_free_bytes > t.min_free_bytes);
    }

    #[test]
    fn is_low_boundary() {
        assert!(is_low(100, 101));
        assert!(!is_low(101, 101));
    }

    #[test]
    fn disk_full_detection() {
        assert!(disk_full_in_message("[Errno 28] No space left on device"));
    }
}
