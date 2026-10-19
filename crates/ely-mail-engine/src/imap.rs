use base64::{Engine, engine::general_purpose::STANDARD};
use imap_proto::{AttributeValue, MailboxDatum, Response, Status};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

use crate::{
    Error, Result,
    net::{self, Login, Secret, Security, Server, Stream, within},
    wire::{Wire, words},
};

/// A logged-in IMAP session over TLS.
pub struct Imap {
    conn: Conn<Stream>,
}

/// One command argument.
enum Arg<'a> {
    /// Written as is.
    Atom(&'a str),
    /// Quoted, or a literal when quotes cannot carry it.
    Text(&'a str),
}

/// OK, or the words of NO or BAD.
type Outcome = std::result::Result<(), String>;

impl Imap {
    pub async fn connect(server: &Server, login: &Login, tls: &TlsConnector) -> Result<Self> {
        let tcp = net::tcp(server).await?;
        let stream = match server.security {
            Security::Tls => net::tls(tcp, server, tls).await?,
            Security::StartTls => {
                let mut plain = Conn::new(tcp);
                within("waiting for the greeting", plain.greeting()).await??;
                within("starting TLS", plain.run("STARTTLS", &[], |_, _| {}))
                    .await??
                    .map_err(Error::Server)?;
                net::tls(plain.wire.into_inner()?, server, tls).await?
            }
        };
        let mut conn = Conn::new(stream);
        if server.security == Security::Tls {
            within("waiting for the greeting", conn.greeting()).await??;
        }
        let outcome = match &login.secret {
            Secret::Password(password) => {
                let args = [Arg::Text(&login.user), Arg::Text(password)];
                within("logging in", conn.run("LOGIN", &args, |_, _| {})).await??
            }
            Secret::Token(token) => {
                within("logging in", conn.xoauth2(&login.xoauth2(token))).await??
            }
        };
        outcome.map_err(Error::Rejected)?;
        log::info!("mail: IMAP login on {}", server.host);
        Ok(Self { conn })
    }

    /// Every mailbox name the server lists.
    pub async fn mailboxes(&mut self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let args = [Arg::Text(""), Arg::Atom("*")];
        within(
            "listing mailboxes",
            self.conn.run("LIST", &args, |response, raw| {
                if let Response::MailboxData(MailboxDatum::List(list)) = response {
                    names.push(mailbox_name(&list.name, raw));
                }
            }),
        )
        .await??
        .map_err(Error::Server)?;
        Ok(names)
    }

    /// The raw message; `None` once the UID is gone.
    pub async fn fetch(&mut self, mailbox: &str, uid: u32) -> Result<Option<Vec<u8>>> {
        let examine = [Arg::Text(mailbox)];
        within(
            "opening the mailbox",
            self.conn.run("EXAMINE", &examine, |_, _| {}),
        )
        .await??
        .map_err(Error::Server)?;
        let number = uid.to_string();
        let args = [Arg::Atom(&number), Arg::Atom("BODY.PEEK[]")];
        let mut found = None;
        within(
            "fetching the message",
            self.conn.run("UID FETCH", &args, |response, _| {
                if let Response::Fetch(_, values) = response
                    && values.contains(&AttributeValue::Uid(uid))
                {
                    found = Some(body(values));
                }
            }),
        )
        .await??
        .map_err(Error::Server)?;
        match found {
            None => Ok(None),
            Some(None) => Err(Error::Protocol(format!("UID {uid} came without a body"))),
            Some(Some(body)) => Ok(Some(body)),
        }
    }

    pub async fn logout(mut self) -> Result<()> {
        within("logging out", self.conn.run("LOGOUT", &[], |_, _| {}))
            .await??
            .map_err(Error::Server)
    }
}

/// imap-proto keeps quoted escapes; a literal is as sent.
fn mailbox_name(name: &str, raw: &[u8]) -> String {
    let line = raw.strip_suffix(b"\r\n").unwrap_or(raw);
    let name_end = |end: &[u8], mark: &[u8]| {
        end.strip_suffix(name.as_bytes())
            .is_some_and(|rest| rest.ends_with(mark))
    };
    let literal = name_end(line, b"}\r\n");
    let quoted = line
        .strip_suffix(b"\"")
        .is_some_and(|end| name_end(end, b"\""));
    if literal || !quoted {
        return name.to_owned();
    }
    let mut plain = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(char) = chars.next() {
        match char {
            '\\' => plain.extend(chars.next()),
            _ => plain.push(char),
        }
    }
    plain
}

fn body(values: &[AttributeValue]) -> Option<Vec<u8>> {
    values.iter().find_map(|value| match value {
        AttributeValue::BodySection {
            section: None,
            data: Some(data),
            ..
        } => Some(data.to_vec()),
        _ => None,
    })
}

struct Conn<S> {
    wire: Wire<S>,
    tags: u32,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Conn<S> {
    fn new(io: S) -> Self {
        Self {
            wire: Wire::new(io),
            tags: 0,
        }
    }

    /// `* BYE` ends with the server's words.
    async fn greeting(&mut self) -> Result<()> {
        self.wire
            .next(|response, raw| match response {
                Response::Data {
                    status: Status::Ok, ..
                } => Ok(()),
                Response::Data {
                    status: Status::Bye,
                    ..
                } => Err(Error::Server(words(raw))),
                Response::Data { status, .. } => Err(Error::Protocol(format!(
                    "the greeting was {status:?}, not OK"
                ))),
                _ => Err(Error::Protocol("the first line was not a greeting".into())),
            })
            .await?
    }

    fn tag(&mut self) -> String {
        self.tags += 1;
        format!("A{}", self.tags)
    }

    /// Sends a command; untagged responses go to `each`.
    async fn run(
        &mut self,
        verb: &str,
        args: &[Arg<'_>],
        mut each: impl FnMut(&Response, &[u8]),
    ) -> Result<Outcome> {
        let tag = self.tag();
        log::debug!("mail: IMAP {tag} {verb}");
        let mut line = format!("{tag} {verb}").into_bytes();
        for arg in args {
            line.push(b' ');
            match arg {
                Arg::Atom(atom) => line.extend_from_slice(atom.as_bytes()),
                Arg::Text(text) => match quoted(text)? {
                    Some(quoted) => line.extend_from_slice(quoted.as_bytes()),
                    None => {
                        line.extend_from_slice(format!("{{{}}}\r\n", text.len()).as_bytes());
                        self.wire.write(&line).await?;
                        line.clear();
                        if let Err(words) = self.proceed(&tag, &mut each).await? {
                            return Ok(Err(words));
                        }
                        line.extend_from_slice(text.as_bytes());
                    }
                },
            }
        }
        line.extend_from_slice(b"\r\n");
        self.wire.write(&line).await?;
        loop {
            match self.step(&tag, &mut each).await? {
                Step::Done(outcome) => return Ok(outcome),
                Step::Untagged => {}
                Step::Continue => {
                    return Err(Error::Protocol(
                        "the server asked for more than was sent".into(),
                    ));
                }
            }
        }
    }

    /// Waits for `+` before a literal.
    async fn proceed(
        &mut self,
        tag: &str,
        each: &mut impl FnMut(&Response, &[u8]),
    ) -> Result<Outcome> {
        loop {
            match self.step(tag, each).await? {
                Step::Continue => return Ok(Ok(())),
                Step::Done(Ok(())) => {
                    return Err(Error::Protocol(
                        "the command ended before its literal".into(),
                    ));
                }
                Step::Done(refused) => return Ok(refused),
                Step::Untagged => {}
            }
        }
    }

    /// Answers the first challenge; empty after, so a refusal ends.
    async fn xoauth2(&mut self, initial: &str) -> Result<Outcome> {
        let tag = self.tag();
        self.wire
            .write(format!("{tag} AUTHENTICATE XOAUTH2\r\n").as_bytes())
            .await?;
        let mut answer = Some(STANDARD.encode(initial));
        loop {
            match self.step(&tag, &mut |_, _| {}).await? {
                Step::Continue => {
                    let line = answer.take().unwrap_or_default() + "\r\n";
                    self.wire.write(line.as_bytes()).await?;
                }
                Step::Done(outcome) => return Ok(outcome),
                Step::Untagged => {}
            }
        }
    }

    async fn step(&mut self, tag: &str, each: &mut impl FnMut(&Response, &[u8])) -> Result<Step> {
        self.wire
            .next(|response, raw| match response {
                Response::Continue(_) => Ok(Step::Continue),
                Response::Done {
                    tag: done, status, ..
                } => {
                    if done.0 != tag {
                        return Err(Error::Protocol(format!(
                            "a reply for {} came while waiting for {tag}",
                            done.0
                        )));
                    }
                    match status {
                        Status::Ok => Ok(Step::Done(Ok(()))),
                        Status::No | Status::Bad => Ok(Step::Done(Err(words(raw)))),
                        other => Err(Error::Protocol(format!("{tag} ended with {other:?}"))),
                    }
                }
                untagged => {
                    each(untagged, raw);
                    Ok(Step::Untagged)
                }
            })
            .await?
    }
}

enum Step {
    Continue,
    Untagged,
    Done(Outcome),
}

/// `None` when only a literal can carry the text.
fn quoted(text: &str) -> Result<Option<String>> {
    if text.contains('\0') {
        return Err(Error::Invalid("IMAP cannot carry a NUL byte".into()));
    }
    if !text
        .bytes()
        .all(|byte| byte.is_ascii() && byte != b'\r' && byte != b'\n')
    {
        return Ok(None);
    }
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for char in text.chars() {
        if char == '"' || char == '\\' {
            quoted.push('\\');
        }
        quoted.push(char);
    }
    quoted.push('"');
    Ok(Some(quoted))
}
