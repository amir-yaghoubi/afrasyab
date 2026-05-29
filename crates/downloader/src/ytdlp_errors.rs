#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YtDlpFailureKind {
    JsRuntime,
    DiskFull,
    Unavailable,
    FileTooLarge,
    Other,
}

pub fn classify_ytdlp_stderr(stderr: &str) -> YtDlpFailureKind {
    let s = stderr.to_ascii_lowercase();
    if s.contains("no supported javascript runtime")
        || s.contains("js-runtimes")
        || s.contains("ejs")
    {
        return YtDlpFailureKind::JsRuntime;
    }
    if s.contains("no space left on device") || s.contains("errno 28") {
        return YtDlpFailureKind::DiskFull;
    }
    if s.contains("video unavailable")
        || s.contains("private video")
        || s.contains("age-restricted")
        || s.contains("sign in")
    {
        return YtDlpFailureKind::Unavailable;
    }
    if s.contains("max-filesize") || s.contains("file is larger than") {
        return YtDlpFailureKind::FileTooLarge;
    }
    YtDlpFailureKind::Other
}

pub fn user_message_for_kind(kind: YtDlpFailureKind) -> &'static str {
    match kind {
        YtDlpFailureKind::JsRuntime => {
            "YouTube needs a server update. Try again later or send a direct file link."
        }
        YtDlpFailureKind::DiskFull => {
            "Server storage is full. Try again in a few minutes."
        }
        YtDlpFailureKind::Unavailable => {
            "This video isn't available (private, removed, or restricted)."
        }
        YtDlpFailureKind::FileTooLarge => "File is too large (max 2 GB).",
        YtDlpFailureKind::Other => {
            "Download failed. The link may be unsupported or temporarily unavailable."
        }
    }
}

pub fn failure_kind_slug(kind: YtDlpFailureKind) -> &'static str {
    match kind {
        YtDlpFailureKind::JsRuntime => "js_runtime",
        YtDlpFailureKind::DiskFull => "disk_full",
        YtDlpFailureKind::Unavailable => "unavailable",
        YtDlpFailureKind::FileTooLarge => "file_too_large",
        YtDlpFailureKind::Other => "other",
    }
}

pub fn friendly_ytdlp_message(stderr: &str) -> String {
    user_message_for_kind(classify_ytdlp_stderr(stderr)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_js_runtime_warning() {
        let stderr = "WARNING: [youtube] No supported JavaScript runtime could be found.";
        assert_eq!(classify_ytdlp_stderr(stderr), YtDlpFailureKind::JsRuntime);
    }

    #[test]
    fn classifies_disk_full() {
        assert_eq!(
            classify_ytdlp_stderr("ERROR: unable to write data: [Errno 28] No space left on device"),
            YtDlpFailureKind::DiskFull
        );
    }

    #[test]
    fn classifies_unavailable() {
        assert_eq!(
            classify_ytdlp_stderr("ERROR: Video unavailable"),
            YtDlpFailureKind::Unavailable
        );
    }

    #[test]
    fn classifies_file_too_large() {
        assert_eq!(
            classify_ytdlp_stderr("ERROR: File is larger than max-filesize (2G)"),
            YtDlpFailureKind::FileTooLarge
        );
    }

    #[test]
    fn classifies_other() {
        assert_eq!(
            classify_ytdlp_stderr("ERROR: Unsupported URL"),
            YtDlpFailureKind::Other
        );
    }
}
