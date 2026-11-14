//! Pure helpers: URL validation, file naming, formatting, categories. All unit-tested.

use std::time::Duration;

pub fn validate_url(raw: &str) -> Result<(), &'static str> {
    let u = raw.trim();
    if u.is_empty() || u.chars().any(char::is_whitespace) {
        return Err("Enter a URL without spaces");
    }
    let rest = ["http://", "https://"]
        .iter()
        .find_map(|p| {
            u.get(..p.len())
                .filter(|s| s.eq_ignore_ascii_case(p))
                .map(|_| &u[p.len()..])
        })
        .ok_or("Only http:// and https:// links are supported")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or("");
    if host.trim_start_matches('[').is_empty() || host.starts_with(':') {
        return Err("The link has no host name");
    }
    Ok(())
}

pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host.rsplit('@').next().unwrap_or(host)
}

/// Last path segment, percent-decoded for the common `%20` case, sanitised for filesystems.
pub fn file_name_from_url(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let seg = path
        .split_once('/')
        .map_or("", |(_, p)| p)
        .rsplit('/')
        .next()
        .unwrap_or("");
    let name: String = seg
        .replace("%20", " ")
        .chars()
        .map(|c| {
            if c.is_control() || "\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_matches('.').to_string();
    if name.is_empty() {
        "download".into()
    } else if is_windows_reserved_name(&name) {
        format!("_{name}")
    } else {
        name
    }
}

/// Windows reserves these device names even when followed by an extension.
pub fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ').to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9" | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FileKind {
    Video,
    Audio,
    Archive,
    Document,
    Program,
    Image,
    Other,
}

impl FileKind {
    pub const ALL: [FileKind; 7] = [
        Self::Video,
        Self::Audio,
        Self::Archive,
        Self::Document,
        Self::Program,
        Self::Image,
        Self::Other,
    ];
    pub fn of(file_name: &str) -> Self {
        let ext = file_name
            .rsplit_once('.')
            .map_or("", |(_, e)| e)
            .to_ascii_lowercase();
        match ext.as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "webm" => Self::Video,
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" => Self::Audio,
            "zip" | "gz" | "tgz" | "7z" | "rar" | "tar" | "xz" | "bz2" | "iso" => Self::Archive,
            "pdf" | "doc" | "docx" | "txt" | "epub" | "xls" | "xlsx" | "ppt" | "pptx" => {
                Self::Document
            }
            "exe" | "msi" | "dmg" | "pkg" | "deb" | "rpm" | "appimage" => Self::Program,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => Self::Image,
            _ => Self::Other,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Archive => "Archives",
            Self::Document => "Documents",
            Self::Program => "Programs",
            Self::Image => "Images",
            Self::Other => "Other",
        }
    }
}

pub fn format_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

pub fn format_speed(bps: u64) -> String {
    format!("{}/s", format_bytes(bps))
}

pub fn format_eta(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation() {
        for ok in [
            "https://a.com/x.zip",
            "HTTP://a.com",
            "https://a.com:8080?x=1",
        ] {
            assert!(validate_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "  ",
            "a.com/x",
            "magnet:?xt=urn:btih:1",
            "https://",
            "https:///x",
            "https://a b.com",
            "file:///etc/passwd",
            "ftp://host/file",
            "https://:80/x",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn file_names() {
        assert_eq!(
            file_name_from_url("https://a.com/dir/My%20File.zip?token=1#x"),
            "My File.zip"
        );
        assert_eq!(file_name_from_url("https://a.com/"), "download");
        assert_eq!(file_name_from_url("https://a.com"), "download");
        assert_eq!(file_name_from_url("https://a.com/a/b:c?.zip"), "b_c");
        assert_eq!(file_name_from_url("https://a.com/.."), "download");
        assert_eq!(file_name_from_url("https://a.com/CON.txt"), "_CON.txt");
        assert_eq!(file_name_from_url("https://a.com/lpt1"), "_lpt1");
        assert_eq!(file_name_from_url("https://a.com/console.txt"), "console.txt");
    }

    #[test]
    fn hosts_and_kinds() {
        assert_eq!(host_of("https://user@cdn.a.com:81/x"), "cdn.a.com:81");
        assert_eq!(FileKind::of("Movie.MKV"), FileKind::Video);
        assert_eq!(FileKind::of("noext"), FileKind::Other);
    }

    #[test]
    fn formatting() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
        assert_eq!(format_eta(Duration::from_secs(59)), "59s");
        assert_eq!(format_eta(Duration::from_secs(125)), "2m 05s");
        assert_eq!(format_eta(Duration::from_secs(3 * 3600 + 7 * 60)), "3h 07m");
    }
}
