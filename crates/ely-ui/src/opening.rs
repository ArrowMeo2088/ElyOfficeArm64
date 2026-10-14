//! Files handed to the app: arguments, Finder, second copies.

use std::{io, path::PathBuf};

use ely_platform::instance::Door;
use futures::{StreamExt, channel::mpsc};
use gpui::{App, Application, Global};

/// A request to the running app.
pub(crate) enum Ask {
    Open(PathBuf),
    /// A second copy started: come to the front.
    Raise,
}

struct Asked(Option<mpsc::UnboundedReceiver<Ask>>);

impl Global for Asked {}

/// Launch arguments, relative to where the app was started.
pub(crate) fn launch_args() -> io::Result<Vec<PathBuf>> {
    std::env::args_os().skip(1).map(std::path::absolute).collect()
}

/// Queues `files`, then files the system or copies ask for.
pub(crate) fn listen(application: &Application, files: Vec<PathBuf>, door: Door) -> io::Result<mpsc::UnboundedReceiver<Ask>> {
    let (sender, asked) = mpsc::unbounded();
    for path in files {
        sender.unbounded_send(Ask::Open(path)).ok();
    }
    let handed = sender.clone();
    door.serve(move |files| {
        for path in files {
            handed.unbounded_send(Ask::Open(path)).ok();
        }
        handed.unbounded_send(Ask::Raise).ok();
    })?;
    application.on_open_urls(move |urls| {
        for url in urls {
            match file_path(&url) {
                Some(path) => {
                    log::info!("run: the system asks to open a file");
                    sender.unbounded_send(Ask::Open(path)).ok();
                }
                None => log::warn!("run: not a file URL, ignored"),
            }
        }
    });
    Ok(asked)
}

pub(crate) fn keep(asked: mpsc::UnboundedReceiver<Ask>, cx: &mut App) {
    cx.set_global(Asked(Some(asked)));
}

/// No files asked at launch, for tests.
#[cfg(any(test, feature = "test-support"))]
pub fn install_no_launch_files(cx: &mut App) {
    keep(mpsc::unbounded().1, cx);
}

/// Files asked before the app started: arguments and Finder's.
pub fn launch_files(cx: &mut App) -> Vec<PathBuf> {
    let asked = cx.global_mut::<Asked>().0.as_mut();
    let asked = asked.expect("launch_files comes before on_open");
    let pending = std::iter::from_fn(|| asked.try_recv().ok());
    // The window opens in front anyway.
    pending.filter_map(|ask| if let Ask::Open(path) = ask { Some(path) } else { None }).collect()
}

/// Hands each file asked from now on to `open`.
pub fn on_open(cx: &mut App, mut open: impl FnMut(PathBuf, &mut App) + 'static) {
    let mut asked = cx
        .global_mut::<Asked>()
        .0
        .take()
        .expect("on_open runs once per app");
    cx.spawn(async move |cx| {
        while let Some(ask) = asked.next().await {
            cx.update(|cx| match ask {
                Ask::Open(path) => open(path, cx),
                Ask::Raise => raise(cx),
            });
        }
    })
    .detach();
}

/// Brings the main window, the first opened, to the front.
fn raise(cx: &mut App) {
    cx.activate(true);
    let Some(window) = cx.windows().into_iter().next() else {
        log::warn!("run: no window to bring to the front");
        return;
    };
    if let Err(error) = window.update(cx, |_, window, _| window.activate_window()) {
        log::warn!("run: the window could not come to the front: {error}");
    }
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
