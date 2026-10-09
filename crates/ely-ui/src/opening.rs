//! Files the system hands the app: arguments, Finder.

use std::path::PathBuf;

use futures::{StreamExt, channel::mpsc};
use gpui::{App, Application, Global};

struct Asked(Option<mpsc::UnboundedReceiver<PathBuf>>);

impl Global for Asked {}

/// Queues launch arguments and every later open request.
pub(crate) fn listen(application: &Application) -> mpsc::UnboundedReceiver<PathBuf> {
    let (sender, asked) = mpsc::unbounded();
    // Relative to where the app was started.
    for path in std::env::args_os().skip(1) {
        match std::path::absolute(&path) {
            Ok(path) => {
                sender.unbounded_send(path).ok();
            }
            Err(error) => log::error!("run: a launch argument has no place: {error}"),
        }
    }
    application.on_open_urls(move |urls| {
        for url in urls {
            match file_path(&url) {
                Some(path) => {
                    log::info!("run: the system asks to open a file");
                    sender.unbounded_send(path).ok();
                }
                None => log::warn!("run: not a file URL, ignored"),
            }
        }
    });
    asked
}

pub(crate) fn keep(asked: mpsc::UnboundedReceiver<PathBuf>, cx: &mut App) {
    cx.set_global(Asked(Some(asked)));
}

/// Files asked before the app started: arguments and Finder's.
pub fn launch_files(cx: &mut App) -> Vec<PathBuf> {
    let asked = cx.global_mut::<Asked>().0.as_mut();
    let asked = asked.expect("launch_files comes before on_open");
    std::iter::from_fn(|| asked.try_recv().ok()).collect()
}

/// Hands each file asked from now on to `open`.
pub fn on_open(cx: &mut App, mut open: impl FnMut(PathBuf, &mut App) + 'static) {
    let mut asked = cx
        .global_mut::<Asked>()
        .0
        .take()
        .expect("on_open runs once per app");
    cx.spawn(async move |cx| {
        while let Some(path) = asked.next().await {
            cx.update(|cx| open(path, cx));
        }
    })
    .detach();
}

/// A `file://` URL's path, percent escapes decoded.
fn file_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut raw = rest.bytes();
    while let Some(byte) = raw.next() {
        if byte != b'%' {
            bytes.push(byte);
            continue;
        }
        let hex = [raw.next()?, raw.next()?];
        bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        // `file:///C:/a` holds `/C:/a`.
        let text = String::from_utf8(bytes).ok()?;
        Some(PathBuf::from(text.strip_prefix('/').unwrap_or(&text)))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::file_path;
    use std::path::PathBuf;

    #[test]
    fn file_urls_decode_and_others_do_not() {
        assert_eq!(file_path("file:///Users/a/b%20c%23.md"), Some(PathBuf::from("/Users/a/b c#.md")));
        assert_eq!(file_path("file://localhost/tmp/x"), Some(PathBuf::from("/tmp/x")));
        assert_eq!(file_path("file:///a/%E4%BD%A0.md"), Some(PathBuf::from("/a/你.md")));
        assert_eq!(file_path("https://example.com/a"), None);
        assert_eq!(file_path("file:///a/%zz"), None);
        assert_eq!(file_path("file:///a/%4"), None);
    }
}
