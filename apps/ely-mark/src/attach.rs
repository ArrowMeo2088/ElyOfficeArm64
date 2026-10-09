//! Images dropped or pasted, copied beside the file (MARK-07).

use std::{
    io,
    io::Write as _,
    path::{Path, PathBuf},
};

use ely_platform::clipboard::{self, Clip};
use ely_ui::i18n::text;
use gpui::{App, Context, WeakEntity, Window};
use serde_json::json;

use crate::view::{Failed, MarkView};

/// The folder beside the file that takes its images.
pub const FOLDER: &str = "assets";

const IMAGES: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];

fn image(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    IMAGES.contains(&extension.to_lowercase().as_str())
}

/// A relative link to a file in the folder, escaped.
pub fn link(name: &str) -> String {
    format!("{FOLDER}/{}", crate::paper::segment(name))
}

/// Writes `png` into `dir` under a new name.
fn save_png(dir: &Path, png: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis();
    let mut n = 0;
    loop {
        let name = match n {
            0 => format!("pasted-{stamp}.png"),
            n => format!("pasted-{stamp}-{n}.png"),
        };
        let path = dir.join(name);
        match std::fs::File::create_new(&path) {
            Ok(mut file) => {
                file.write_all(png)?;
                file.flush()?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(error) => return Err(error),
        }
    }
}

/// The paste hook: an image on the clipboard is attached.
pub fn paste(view: WeakEntity<MarkView>) -> impl Fn(&mut Window, &mut App) -> bool {
    move |_, cx| match clipboard::read() {
        Ok(Some(Clip::Png(png))) => {
            view.update(cx, |view, cx| view.attach(Attach::Png(png), cx))
                .ok();
            true
        }
        Ok(_) => false,
        Err(error) => {
            log::error!("mark: the clipboard did not read: {error:#}");
            let why = format!("{error:#}");
            view.update(cx, |_, cx| cx.emit(Failed(why))).ok();
            false
        }
    }
}

pub enum Attach {
    Files(Vec<PathBuf>),
    Png(Vec<u8>),
}

impl MarkView {
    /// Copies into the file's folder, then links at the caret.
    pub fn attach(&mut self, what: Attach, cx: &mut Context<Self>) {
        let refused = match (&what, self.attachable, self.mode) {
            (_, false, _) => Some("mark.attach.untitled"),
            (_, _, crate::page::Mode::Preview) => Some("mark.attach.preview"),
            (Attach::Files(files), ..) if !files.iter().all(|file| image(file)) => {
                Some("mark.attach.images")
            }
            _ => None,
        };
        if let Some(key) = refused {
            return cx.emit(Failed(text(cx, key, &[]).to_string()));
        }
        let folder = self.folder.clone();
        let dir = folder.join(FOLDER);
        cx.spawn(async move |view, cx| {
            let copied = cx
                .background_executor()
                .spawn(async move {
                    match what {
                        Attach::Files(files) => files
                            .iter()
                            .map(|file| ely_files::import_attachment(file, &dir))
                            .collect(),
                        Attach::Png(png) => save_png(&dir, &png).map(|path| vec![path]),
                    }
                })
                .await;
            view.update(cx, |view, cx| view.attached(&folder, copied, cx))
                .ok();
        })
        .detach();
    }

    fn attached(
        &mut self,
        folder: &Path,
        copied: io::Result<Vec<PathBuf>>,
        cx: &mut Context<Self>,
    ) {
        let copied = match copied {
            Ok(copied) => copied,
            Err(error) => {
                log::error!("mark: an image did not copy: {error}");
                return cx.emit(Failed(error.to_string()));
            }
        };
        // Links resolve from the file's folder; it moved meanwhile.
        if self.folder != folder {
            log::warn!("mark: the file moved while images copied; no links");
            let at = folder.join(FOLDER).display().to_string();
            return cx.emit(Failed(
                text(cx, "mark.attach.moved", &[("folder", &at)]).to_string(),
            ));
        }
        log::info!("mark: attached {} images", copied.len());
        let images: Vec<_> = copied
            .iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                let alt = path.file_stem()?.to_string_lossy().into_owned();
                Some(json!({ "src": link(&name), "alt": alt }))
            })
            .collect();
        crate::page::call(self.web.read(cx), "insert", &json!(images));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_escape_and_names_never_repeat() {
        assert_eq!(super::link("a b#1.png"), "assets/a%20b%231.png");
        let dir = tempfile::tempdir().expect("a temp dir");
        let first = super::save_png(dir.path(), b"x").expect("saves");
        let second = super::save_png(dir.path(), b"y").expect("saves");
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).expect("reads"), b"x");
    }
}
