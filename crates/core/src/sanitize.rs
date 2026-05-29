use regex::Regex;
use std::sync::LazyLock;

static HTTP_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://\S+").expect("http url regex"));
static TG_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:tg://|t\.me/)\S+").expect("tg url regex"));
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)bearer\s+[A-Za-z0-9._-]+").expect("bearer regex"));
static FILE_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)file_id[=:\s]+[A-Za-z0-9_-]{20,}").expect("file_id regex")
});

pub fn sanitize_for_log(input: &str) -> String {
    let mut s = input.to_string();
    s = HTTP_URL.replace_all(&s, "[url]").into_owned();
    s = TG_URL.replace_all(&s, "[url]").into_owned();
    s = BEARER.replace_all(&s, "[token]").into_owned();
    s = FILE_ID.replace_all(&s, "[file_id]").into_owned();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_https_url() {
        let out = sanitize_for_log("GET failed: https://example.com/video?id=1");
        assert!(!out.contains("example.com"));
        assert!(out.contains("[url]"));
    }

    #[test]
    fn redacts_multiple_urls() {
        let out = sanitize_for_log("a https://a.com x http://b.org/y");
        assert_eq!(out.matches("[url]").count(), 2);
    }

    #[test]
    fn leaves_plain_errors() {
        let msg = "HTTP 403 for host";
        assert_eq!(sanitize_for_log(msg), msg);
    }

    #[test]
    fn redacts_tme_link() {
        let out = sanitize_for_log("see t.me/somechannel/123");
        assert!(out.contains("[url]"));
    }
}
