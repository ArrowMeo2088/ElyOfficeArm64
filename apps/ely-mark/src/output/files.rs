//! An export written to disk.

use std::{
    io,
    path::{Path, PathBuf},
};

use super::{Page, Pictures, body, document, plain};

/// What an export writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// One HTML file, pictures inside.
    Page,
    /// HTML and a folder of its pictures.
    Linked,
    Pdf,
    Text,
}

/// Where one export goes, and what it leaves alone.
pub struct Target<'a> {
    pub path: &'a Path,
    /// The tab's own file, never written over.
    pub session: Option<&'a Path>,
    pub revision: u64,
    /// A scratch folder of the tab's.
    pub work: &'a Path,
}

/// `text` written as `format`; what it left out.
pub fn write(
    format: Format,
    text: &str,
    beside: Option<&Path>,
    target: &Target,
) -> io::Result<Page> {
    let title = target
        .path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    let page = match format {
        Format::Page => {
            let page = body(text, beside, &Pictures::Inline)?;
            export(target, document(&title, &page.html).as_bytes())?;
            page
        }
        Format::Linked => linked(text, beside, target, &title)?,
        Format::Pdf => pdf(text, beside, target, &title)?,
        Format::Text => {
            export(target, plain(text)?.as_bytes())?;
            Page::default()
        }
    };
    log::info!("mark: exported {format:?}, revision {}", target.revision);
    Ok(page)
}

fn export(target: &Target, bytes: &[u8]) -> io::Result<()> {
    ely_files::export(target.path, target.session, bytes, target.revision).map(|_| ())
}

/// The page beside a new folder of its pictures.
fn linked(text: &str, beside: Option<&Path>, target: &Target, title: &str) -> io::Result<Page> {
    let name = format!("{title}_files");
    let page = body(text, beside, &Pictures::Folder(name.clone()))?;
    if page.copies.is_empty() {
        export(target, document(title, &page.html).as_bytes())?;
        return Ok(page);
    }
    let folder = target.path.with_file_name(&name);
    // A folder already there is someone's: refused.
    std::fs::create_dir(&folder).map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => io::Error::new(error.kind(), folder.display().to_string()),
        _ => error,
    })?;
    let written = copy(&page.copies, &folder)
        .and_then(|()| export(target, document(title, &page.html).as_bytes()));
    if let Err(error) = written {
        if let Err(left) = std::fs::remove_dir_all(&folder) {
            log::error!("mark: the half-made picture folder stays: {left}");
        }
        return Err(error);
    }
    Ok(page)
}

/// The page set by MuPDF, from a scratch folder.
fn pdf(text: &str, beside: Option<&Path>, target: &Target, title: &str) -> io::Result<Page> {
    let dir = target.work.join("pdf");
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(dir.join("files"))?;
    let set = (|| -> io::Result<Page> {
        let page = body(text, beside, &Pictures::Pdf("files".into()))?;
        copy(&page.copies, &dir.join("files"))?;
        std::fs::write(dir.join("page.html"), document(title, &page.html))?;
        let out = dir.join("out.pdf");
        ely_pdf_engine::Worker::spawn()?.html_pdf(&dir, "page.html", &out)?;
        export(target, &std::fs::read(&out)?)?;
        Ok(page)
    })();
    let cleaned = std::fs::remove_dir_all(&dir);
    let page = set?;
    cleaned?;
    Ok(page)
}

fn copy(copies: &[(PathBuf, String)], folder: &Path) -> io::Result<()> {
    for (from, name) in copies {
        std::fs::copy(from, folder.join(name))?;
    }
    Ok(())
}
