use crate::ytdlp_errors::classify_ytdlp_stderr;
use afrasyab_domain::types::YtDlpFormat;
use anyhow::Context;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub struct YtDlpRunError {
    pub kind: crate::ytdlp_errors::YtDlpFailureKind,
    pub stderr: String,
}

impl fmt::Display for YtDlpRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "yt-dlp failed: {}", self.stderr)
    }
}

impl std::error::Error for YtDlpRunError {}

pub struct YtDlpArgs {
    pub argv: Vec<String>,
    pub output_dir: PathBuf,
}

/// Format byte limit for yt-dlp `--max-filesize` (binary G/M suffixes).
pub fn bytes_to_ytdlp_max_filesize(bytes: u64) -> String {
    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= GIB && bytes.is_multiple_of(GIB) {
        format!("{}G", bytes / GIB)
    } else if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{}M", bytes / MIB)
    } else {
        bytes.to_string()
    }
}

pub fn build_args(
    url: &str,
    output_dir: &Path,
    format: YtDlpFormat,
    max_file_bytes: u64,
) -> YtDlpArgs {
    let template = output_dir.join("%(title).200B [%(id)s].%(ext)s");
    let max_size = bytes_to_ytdlp_max_filesize(max_file_bytes);
    let mut argv = vec![
        "yt-dlp".to_string(),
        "--no-playlist".to_string(),
        "--max-filesize".to_string(),
        max_size,
        "-o".to_string(),
        template.to_string_lossy().into_owned(),
        url.to_string(),
    ];
    match format {
        YtDlpFormat::Video => {
            argv.push("-f".into());
            argv.push("bv*+ba/b".into());
        }
        YtDlpFormat::Audio => {
            argv.push("-x".into());
            argv.push("--audio-format".into());
            argv.push("mp3".into());
        }
        YtDlpFormat::Best => {}
    }
    YtDlpArgs {
        argv,
        output_dir: output_dir.to_path_buf(),
    }
}

pub async fn run_ytdlp(args: YtDlpArgs, timeout: Duration) -> anyhow::Result<PathBuf> {
    let program = args.argv.first().context("yt-dlp argv is empty")?.clone();
    let cmd_args = args.argv[1..].to_vec();
    let output_dir = args.output_dir.clone();

    let output = tokio::time::timeout(timeout, async {
        tokio::process::Command::new(&program)
            .args(&cmd_args)
            .output()
            .await
    })
    .await
    .context("yt-dlp timed out")??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let kind = classify_ytdlp_stderr(&stderr);
        return Err(YtDlpRunError { kind, stderr }.into());
    }

    newest_file_in_dir(&output_dir).await
}

async fn newest_file_in_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .with_context(|| format!("read output dir {}", dir.display()))?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &newest {
            Some((_, t)) if modified <= *t => {}
            _ => newest = Some((entry.path(), modified)),
        }
    }
    newest
        .map(|(path, _)| path)
        .context("no downloaded file found in output directory")
}

#[cfg(test)]
mod tests {
    use super::*;
    use afrasyab_domain::types::YtDlpFormat;

    #[test]
    fn bytes_to_ytdlp_max_filesize_formats_binary_units() {
        assert_eq!(
            bytes_to_ytdlp_max_filesize(2 * 1024 * 1024 * 1024),
            "2G"
        );
        assert_eq!(bytes_to_ytdlp_max_filesize(512 * 1024 * 1024), "512M");
        assert_eq!(bytes_to_ytdlp_max_filesize(1500), "1500");
    }

    #[test]
    fn build_args_includes_max_filesize() {
        let args = build_args(
            "https://youtu.be/abc",
            Path::new("/tmp"),
            YtDlpFormat::Best,
            2 * 1024 * 1024 * 1024,
        );
        let idx = args
            .argv
            .iter()
            .position(|a| a == "--max-filesize")
            .unwrap();
        assert_eq!(args.argv[idx + 1], "2G");
    }

    #[test]
    fn build_args_video_includes_format_selector() {
        let args = build_args(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            Path::new("/tmp/out"),
            YtDlpFormat::Video,
            2 * 1024 * 1024 * 1024,
        );
        assert_eq!(args.argv[0], "yt-dlp");
        assert!(args.argv.contains(&"--no-playlist".to_string()));
        assert!(args.argv.contains(&"-f".to_string()));
        assert!(args.argv.contains(&"bv*+ba/b".to_string()));
        assert!(args
            .argv
            .iter()
            .any(|a| a.contains("%(title).200B [%(id)s].%(ext)s")));
        assert!(args
            .argv
            .contains(&"https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn build_args_audio_extracts_mp3() {
        let args = build_args(
            "https://soundcloud.com/example/track",
            Path::new("/data/dl"),
            YtDlpFormat::Audio,
            2 * 1024 * 1024 * 1024,
        );
        assert!(args.argv.windows(2).any(|w| w == ["-x", "--audio-format"]));
        assert!(args.argv.contains(&"mp3".to_string()));
        assert!(!args.argv.contains(&"-f".to_string()));
    }

    #[test]
    fn build_args_best_has_no_extra_format_flags() {
        let args = build_args(
            "https://youtu.be/abc",
            Path::new("/tmp"),
            YtDlpFormat::Best,
            2 * 1024 * 1024 * 1024,
        );
        assert!(!args.argv.contains(&"-f".to_string()));
        assert!(!args.argv.contains(&"-x".to_string()));
    }

    #[test]
    fn build_args_output_template_under_output_dir() {
        let args = build_args(
            "https://example.com",
            Path::new("/var/tmp/jobs/1"),
            YtDlpFormat::Best,
            2 * 1024 * 1024 * 1024,
        );
        let o_idx = args.argv.iter().position(|a| a == "-o").unwrap();
        let template = &args.argv[o_idx + 1];
        assert!(template.starts_with("/var/tmp/jobs/1/"));
    }
}
