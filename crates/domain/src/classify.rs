use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    YtDlp,
    DirectHttp,
}

pub fn classify_url(raw: &str) -> Option<(LinkKind, Url)> {
    let url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_lowercase();
    let kind = if is_yt_dlp_host(&host) {
        LinkKind::YtDlp
    } else {
        LinkKind::DirectHttp
    };
    Some((kind, url))
}

fn is_yt_dlp_host(host: &str) -> bool {
    matches!(
        host,
        "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "youtu.be"
            | "music.youtube.com"
            | "soundcloud.com"
            | "www.soundcloud.com"
    ) || host.ends_with(".soundcloud.com")
}

pub fn extract_urls(text: &str) -> Vec<Url> {
    let re = regex::Regex::new(r"https?://[^\s<>]+").expect("url regex");
    re.find_iter(text)
        .filter_map(|m| Url::parse(m.as_str().trim_end_matches(')')).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_is_yt_dlp() {
        let (kind, _) = classify_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
        assert_eq!(kind, LinkKind::YtDlp);
    }

    #[test]
    fn soundcloud_is_yt_dlp() {
        let (kind, _) = classify_url("https://soundcloud.com/artist/track").unwrap();
        assert_eq!(kind, LinkKind::YtDlp);
    }

    #[test]
    fn pdf_is_direct() {
        let (kind, _) = classify_url("https://example.com/file.pdf").unwrap();
        assert_eq!(kind, LinkKind::DirectHttp);
    }
}
