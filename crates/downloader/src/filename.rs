use reqwest::header::{HeaderMap, CONTENT_DISPOSITION, CONTENT_TYPE};
use std::path::{Path, PathBuf};

/// Last non-empty, decoded path segment (e.g. `report.pdf` from `/files/report.pdf`).
pub fn filename_from_url_path(url: &url::Url) -> Option<String> {
    let segment = url
        .path_segments()?
        .rfind(|s| !s.is_empty() && *s != "." && *s != "..")?;
    let name = percent_decode(segment).unwrap_or_else(|| segment.to_string());
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Prefer `Content-Disposition` filename; add extension from `Content-Type` when missing.
pub fn resolve_direct_dest(hint: &Path, headers: &HeaderMap) -> PathBuf {
    let parent = hint.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = parent.unwrap_or_else(|| Path::new("."));

    let mut name = hint
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("download")
        .to_string();

    if let Some(cd) = headers
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_content_disposition)
    {
        name = cd;
    }

    if !name.contains('.') {
        if let Some(ext) = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(extension_from_content_type)
        {
            name = format!("{name}.{ext}");
        }
    }

    parent.join(sanitize_filename(&name))
}

fn filename_from_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let encoded = rest
                .split_once("''")
                .map(|(_, enc)| enc)
                .unwrap_or(rest)
                .trim()
                .trim_matches('"');
            if let Some(decoded) = percent_decode(encoded) {
                let name = decoded.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            continue;
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            let name = rest.trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn percent_decode(input: &str) -> Option<String> {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    let mime_str = content_type.split(';').next()?.trim();
    let mime: mime_guess::Mime = mime_str.parse().ok()?;
    mime_guess::get_mime_extensions(&mime)?.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_from_url_path_uses_last_segment() {
        let url = url::Url::parse("https://cdn.example.com/a/report.pdf").unwrap();
        assert_eq!(
            filename_from_url_path(&url).as_deref(),
            Some("report.pdf")
        );
    }

    #[test]
    fn filename_from_url_path_decodes_percent_encoding() {
        let url = url::Url::parse("https://cdn.example.com/my%2520doc.pdf").unwrap();
        assert_eq!(
            filename_from_url_path(&url).as_deref(),
            Some("my%20doc.pdf")
        );
        let url = url::Url::parse("https://cdn.example.com/my%20doc.pdf").unwrap();
        assert_eq!(
            filename_from_url_path(&url).as_deref(),
            Some("my doc.pdf")
        );
    }

    #[test]
    fn filename_from_url_path_empty_for_root() {
        let url = url::Url::parse("https://example.com/").unwrap();
        assert!(filename_from_url_path(&url).is_none());
    }

    #[test]
    fn resolve_direct_dest_uses_content_disposition() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_DISPOSITION,
            "attachment; filename=\"invoice.pdf\""
                .parse()
                .unwrap(),
        );
        let hint = Path::new("/tmp/download.bin");
        let dest = resolve_direct_dest(hint, &headers);
        assert_eq!(dest, Path::new("/tmp/invoice.pdf"));
    }

    #[test]
    fn resolve_direct_dest_adds_extension_from_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/pdf".parse().unwrap());
        let hint = Path::new("/tmp/download");
        let dest = resolve_direct_dest(hint, &headers);
        assert_eq!(dest, Path::new("/tmp/download.pdf"));
    }
}
