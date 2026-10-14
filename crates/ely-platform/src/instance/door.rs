//! The running copy's door: a second copy hands it files.

use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const FILE: &str = "instance.door";
const TOKEN: u8 = 1;
const PATH: u8 = 2;
const DONE: u8 = 3;
const REFUSED: u8 = 4;
/// How long a stalled caller may hold the door.
const PATIENCE: Duration = Duration::from_secs(5);

/// The running copy's listener; its token keeps others out.
pub struct Door {
    listener: TcpListener,
    token: [u8; 32],
}

impl Door {
    /// Opens a local door; its address goes in `dir`.
    pub(super) fn open(dir: &Path) -> io::Result<Door> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let mut token = [0; 32];
        getrandom::fill(&mut token).map_err(|_| io::Error::other("system randomness failed"))?;
        let port = listener.local_addr()?.port();
        let text = format!("{port} {} {}\n", std::process::id(), hex(&token));
        // Whole or absent to a reader.
        let fresh = dir.join("instance.door.new");
        write_private(&fresh, text.as_bytes())?;
        fs::rename(&fresh, dir.join(FILE))?;
        Ok(Door { listener, token })
    }

    /// Hands each caller's files to `deliver`, on its own thread.
    pub fn serve(self, deliver: impl Fn(Vec<PathBuf>) + Send + 'static) -> io::Result<()> {
        thread::Builder::new()
            .name("instance door".into())
            .spawn(move || {
                for stream in self.listener.incoming() {
                    if let Err(error) = stream.and_then(|stream| self.answer(stream, &deliver)) {
                        log::warn!("instance: a hand-over failed: {error}");
                    }
                }
            })?;
        Ok(())
    }

    fn answer(&self, mut stream: TcpStream, deliver: &impl Fn(Vec<PathBuf>)) -> io::Result<()> {
        stream.set_read_timeout(Some(PATIENCE))?;
        let (tag, token) = next(&mut stream)?;
        if tag != TOKEN || !same(&token, &self.token) {
            ely_ipc::write_frame(&mut stream, REFUSED, 0, &[])?;
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a caller without the token",
            ));
        }
        let mut files = Vec::new();
        loop {
            match next(&mut stream)? {
                (PATH, bytes) => files.push(path(bytes)?),
                (DONE, _) => break,
                (tag, _) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("frame {tag} is not a hand-over's"),
                    ));
                }
            }
        }
        log::info!("instance: another copy handed over {} files", files.len());
        deliver(files);
        ely_ipc::write_frame(&mut stream, DONE, 0, &[])
    }
}

/// Hands `files` to `dir`'s door, waiting up to `wait`.
pub(super) fn hand(dir: &Path, files: &[PathBuf], wait: Duration) -> io::Result<()> {
    let deadline = Instant::now() + wait;
    loop {
        match knock(dir) {
            Ok((stream, token)) => match give(stream, &token, files)? {
                true => return Ok(()),
                // An old door file; the new one is coming.
                false if Instant::now() < deadline => {}
                false => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "it refused the door's token",
                    ));
                }
            },
            Err(_) if Instant::now() < deadline => {}
            Err(error) => return Err(error),
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Connects to `dir`'s door; nothing sent, so retries are safe.
fn knock(dir: &Path) -> io::Result<(TcpStream, Vec<u8>)> {
    let text = fs::read_to_string(dir.join(FILE))?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, format!("an unreadable {FILE}"));
    let mut words = text.split_whitespace();
    let port: u16 = words
        .next()
        .and_then(|word| word.parse().ok())
        .ok_or_else(bad)?;
    let pid: u32 = words
        .next()
        .and_then(|word| word.parse().ok())
        .ok_or_else(bad)?;
    let token = words.next().and_then(unhex).ok_or_else(bad)?;
    let stream =
        TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), PATIENCE)?;
    lend_foreground(pid);
    Ok((stream, token))
}

/// Sends the files; false when the door refuses the token.
fn give(mut stream: TcpStream, token: &[u8], files: &[PathBuf]) -> io::Result<bool> {
    stream.set_read_timeout(Some(PATIENCE))?;
    ely_ipc::write_frame(&mut stream, TOKEN, 0, token)?;
    for file in files {
        ely_ipc::write_frame(&mut stream, PATH, 0, &bytes(file))?;
    }
    ely_ipc::write_frame(&mut stream, DONE, 0, &[])?;
    match next(&mut stream)? {
        (DONE, _) => Ok(true),
        (REFUSED, _) => Ok(false),
        (tag, _) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("it answered frame {tag}"),
        )),
    }
}

fn next(stream: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    match ely_ipc::read_frame(stream)? {
        Some((tag, _, payload)) => Ok((tag, payload)),
        None => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the other copy hung up",
        )),
    }
}

/// Lets the running copy bring its window to the front.
#[cfg(windows)]
fn lend_foreground(pid: u32) {
    // Refused when not in front; then nothing to lend.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid) };
}

#[cfg(not(windows))]
fn lend_foreground(_: u32) {}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

/// Equal tokens, compared in constant time.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |diff, (x, y)| diff | (x ^ y)) == 0
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let pairs = text.as_bytes().chunks(2);
    pairs
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

#[cfg(unix)]
fn bytes(path: &Path) -> Vec<u8> {
    std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec()
}

#[cfg(unix)]
fn path(bytes: Vec<u8>) -> io::Result<PathBuf> {
    Ok(<OsString as std::os::unix::ffi::OsStringExt>::from_vec(bytes).into())
}

/// UTF-16 units, little-endian, so any Windows name survives.
#[cfg(windows)]
fn bytes(path: &Path) -> Vec<u8> {
    std::os::windows::ffi::OsStrExt::encode_wide(path.as_os_str())
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path(bytes: Vec<u8>) -> io::Result<PathBuf> {
    if !bytes.len().is_multiple_of(2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a cut UTF-16 path",
        ));
    }
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(<OsString as std::os::windows::ffi::OsStringExt>::from_wide(&wide).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(dir: &Path) -> std::sync::mpsc::Receiver<Vec<PathBuf>> {
        let (sender, heard) = std::sync::mpsc::channel();
        let door = Door::open(dir).expect("a door");
        door.serve(move |files| sender.send(files).expect("heard"))
            .expect("served");
        heard
    }

    #[test]
    fn a_hand_over_brings_every_path_whole() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let heard = opened(dir.path());
        let files = vec![
            PathBuf::from("C:/a b/笔记 #1.md"),
            PathBuf::from("/tmp/x.md"),
        ];
        hand(dir.path(), &files, Duration::from_secs(2)).expect("handed");
        assert_eq!(heard.recv_timeout(Duration::from_secs(2)), Ok(files));
        hand(dir.path(), &[], Duration::from_secs(2)).expect("an empty hand-over raises");
        assert_eq!(heard.recv_timeout(Duration::from_secs(2)), Ok(vec![]));
    }

    #[test]
    fn a_wrong_token_hands_nothing_over() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let heard = opened(dir.path());
        let door = dir.path().join(FILE);
        let text = fs::read_to_string(&door).expect("the door");
        let (head, _) = text.rsplit_once(' ').expect("a token");
        fs::write(&door, format!("{head} {}\n", hex(&[7; 32]))).expect("a forged door");
        let refused = hand(
            dir.path(),
            &[PathBuf::from("x.md")],
            Duration::from_millis(300),
        );
        assert_eq!(
            refused.map_err(|error| error.kind()),
            Err(io::ErrorKind::PermissionDenied)
        );
        assert!(heard.try_recv().is_err(), "nothing reached the copy");
    }

    #[test]
    fn no_door_fails_after_the_wait() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let started = Instant::now();
        let missing = hand(dir.path(), &[], Duration::from_millis(300));
        assert_eq!(
            missing.map_err(|error| error.kind()),
            Err(io::ErrorKind::NotFound)
        );
        assert!(started.elapsed() >= Duration::from_millis(300));
    }
}
