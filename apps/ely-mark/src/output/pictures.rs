//! Where an exported page's pictures go.

use std::path::{Path, PathBuf};

use base64::Engine as _;

/// Where pictures go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pictures {
    /// Inside the page, as data.
    Inline,
    /// Copied into the named folder beside the page.
    Folder(String),
    /// As a folder, in formats MuPDF reads.
    Pdf(String),
}

/// Picture formats MuPDF reads from files.
const READABLE: [&str; 8] = ["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "svg"];

pub(super) struct Placed<'a> {
    beside: Option<&'a Path>,
    pictures: &'a Pictures,
    pub copies: Vec<(PathBuf, String)>,
}

impl<'a> Placed<'a> {
    pub fn new(beside: Option<&'a Path>, pictures: &'a Pictures) -> Self {
        Self {
            beside,
            pictures,
            copies: Vec::new(),
        }
    }

    /// The `src` a picture takes; else it is missing.
    pub fn place(&mut self, src: &str, after: &str, missing: &mut Vec<String>) -> String {
        let address = decoded(src);
        // markdown-rs blanks addresses it will not allow.
        if address.is_empty() {
            missing.push(alt(after));
            return String::new();
        }
        let web = address.starts_with("http://") || address.starts_with("https://");
        if web || address.starts_with("data:") {
            if web && matches!(self.pictures, Pictures::Pdf(_)) {
                missing.push(address);
                return String::new();
            }
            return attribute(&address);
        }
        let path = match (Path::new(&address).is_absolute(), self.beside) {
            (true, _) => PathBuf::from(&address),
            (false, Some(beside)) => beside.join(&address),
            (false, None) => {
                missing.push(address);
                return String::new();
            }
        };
        match self.carried(&path) {
            Some(src) => src,
            None => {
                missing.push(address);
                String::new()
            }
        }
    }

    fn carried(&mut self, path: &Path) -> Option<String> {
        let extension = path
            .extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let folder = match self.pictures {
            Pictures::Inline => {
                let bytes = std::fs::read(path).ok()?;
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                return Some(format!("data:{};base64,{data}", mime(&extension)));
            }
            Pictures::Pdf(_) if !READABLE.contains(&extension.as_str()) => return None,
            Pictures::Folder(folder) | Pictures::Pdf(folder) => folder.clone(),
        };
        if !path.is_file() {
            return None;
        }
        let name = self.named(path);
        Some(format!("{folder}/{}", encoded(&name)))
    }

    /// A name in the folder, one per picture.
    fn named(&mut self, path: &Path) -> String {
        if let Some((_, name)) = self.copies.iter().find(|(from, _)| from == path) {
            return name.clone();
        }
        let file = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let (stem, extension) = file
            .rsplit_once('.')
            .map_or((file.as_str(), ""), |(stem, ext)| (stem, ext));
        let taken = |name: &str| self.copies.iter().any(|(_, kept)| kept == name);
        let mut name = file.clone();
        let mut count = 1;
        while taken(&name) {
            count += 1;
            name = match extension {
                "" => format!("{stem}-{count}"),
                _ => format!("{stem}-{count}.{extension}"),
            };
        }
        self.copies.push((path.to_path_buf(), name.clone()));
        name
    }
}

fn mime(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

/// The `alt` after a blanked `src`.
fn alt(after: &str) -> String {
    let text = after.strip_prefix(" alt=\"").unwrap_or_default();
    super::unescape(&text[..text.find('"').unwrap_or(text.len())])
}

/// `%XX` undone; bytes that are not UTF-8 stay as written.
fn decoded(src: &str) -> String {
    let src = super::unescape(src);
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok());
        match (
            bytes[at],
            hex.and_then(|pair| u8::from_str_radix(pair, 16).ok()),
        ) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or(src)
}

/// A name as a path segment of a URL.
fn encoded(name: &str) -> String {
    name.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A value safe inside a quoted attribute.
fn attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
