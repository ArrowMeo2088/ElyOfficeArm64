//! The message open in the reading pane (C12.10).

use std::{rc::Rc, thread};

use ely_gpui_component::documents::{HeldPicture, MarkdownBlock, Pictures};
use ely_mail_engine::{MessageId, Stored, fetch};
use gpui::{AppContext as _, Context, SharedString};

use crate::{
    letter::{self, Body, Kept, Letter, Words},
    mail::{Mail, text},
};

/// Web pictures come down this many at once.
const FETCHERS: usize = 4;

/// The pane's message; a newer read drops an older one.
#[derive(Default)]
pub struct Reading {
    pub read: u64,
    /// Store reads asked, checks of the shown one too.
    pub asked: u64,
    pub shown: Option<Shown>,
}

pub struct Shown {
    pub id: MessageId,
    /// None while it is read.
    pub letter: Option<Result<Open, String>>,
    /// The original headers are drawn.
    pub headers: bool,
    /// What the drawn letter was built from.
    pub version: Option<Version>,
}

/// What a message keeps; a change reads it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    body: bool,
    missing: Vec<String>,
}

impl Version {
    fn of(stored: &Stored) -> Self {
        Self {
            body: stored.body,
            missing: stored.missing.clone(),
        }
    }
}

/// A letter as drawn, with what it may load.
pub struct Open {
    pub letter: Letter,
    pub blocks: Rc<[MarkdownBlock]>,
    pub pictures: Rc<Pictures>,
    /// The sender's pictures always load.
    pub trusted: bool,
    /// Web pictures were asked for.
    pub fetching: bool,
}

impl Open {
    pub fn body(&self) -> Option<&Body> {
        self.letter.body.as_ref()
    }

    pub fn sender(&self) -> Option<&str> {
        self.letter.from.as_ref().map(|from| from.email.as_str())
    }
}

fn words(cx: &gpui::App) -> Words {
    Words {
        broken: text(cx, "mail.picture.broken", &[]),
        waiting: text(cx, "mail.picture.waiting", &[]),
        not_picture: text(cx, "mail.picture.not_picture", &[]),
        svg: text(cx, "mail.picture.svg", &[]),
        unnamed: text(cx, "mail.file.unnamed", &[]),
    }
}

impl Mail {
    /// Reads the one message picked, or what changed in it.
    pub fn read_letter(&mut self, cx: &mut Context<Self>) {
        let picked = self.listing.chosen.as_ref().map(|row| row.key.clone());
        let id = match picked.map(|key| key.parse::<i64>()) {
            Some(Ok(id)) => MessageId(id),
            Some(Err(_)) => return log::error!("mail: a listed key is no message id"),
            None => {
                self.reading.shown = None;
                return;
            }
        };
        let known = match &self.reading.shown {
            Some(shown) if shown.id == id => shown.version.clone(),
            _ => None,
        };
        let check = known.is_some();
        if !check {
            self.reading.read += 1;
            self.reading.shown = Some(Shown {
                id,
                letter: None,
                headers: false,
                version: None,
            });
        }
        self.reading.asked += 1;
        let (read, asked) = (self.reading.read, self.reading.asked);
        let words = words(cx);
        self.with_store(
            cx,
            move |store| {
                let stored = store.get(id)?;
                let version = Version::of(&stored);
                if known.as_ref() == Some(&version) {
                    return Ok(None);
                }
                let kept = match store.original(id)? {
                    Some(raw) => Kept::Original(raw),
                    None => Kept::Head(store.head(id)?),
                };
                let sender = stored.from.first().map(|from| from.email.clone());
                let trusted = match sender {
                    Some(sender) => store.trusts(&sender)?,
                    None => false,
                };
                Ok(Some((stored, kept, trusted, version)))
            },
            move |mail, got, cx| {
                if (mail.reading.read, mail.reading.asked) != (read, asked) {
                    return log::info!("mail: read {read} ended after a newer one");
                }
                let (stored, kept, trusted, version) = match got {
                    Ok(Some(got)) => got,
                    Ok(None) => return,
                    Err(error) => return mail.letter_failed(read, error),
                };
                let read = if check { mail.read_again(id) } else { read };
                let built = cx.background_spawn(async move {
                    letter::letter(stored, kept, &words).map(|mut letter| {
                        let body = letter.body.as_mut();
                        let blocks = body.map(|body| std::mem::take(&mut body.blocks));
                        (letter, blocks)
                    })
                });
                cx.spawn(async move |this, cx| {
                    let built = built.await;
                    this.update(cx, |mail, cx| {
                        mail.letter_built(read, built, trusted, version, cx)
                    })
                    .ok();
                })
                .detach();
            },
        );
    }

    /// The shown message changed: a new read replaces it.
    fn read_again(&mut self, id: MessageId) -> u64 {
        log::info!("mail: message {} changed; read again", id.0);
        self.reading.read += 1;
        if let Some(shown) = &mut self.reading.shown {
            shown.letter = None;
            shown.version = None;
        }
        self.reading.read
    }

    fn letter_failed(&mut self, read: u64, error: String) {
        log::warn!("mail: read {read} failed: {error}");
        if let Some(shown) = &mut self.reading.shown {
            shown.letter = Some(Err(error));
        }
    }

    fn letter_built(
        &mut self,
        read: u64,
        built: Result<(Letter, Option<Vec<MarkdownBlock>>), String>,
        trusted: bool,
        version: Version,
        cx: &mut Context<Self>,
    ) {
        if self.reading.read != read {
            return log::info!("mail: letter {read} built after a newer read");
        }
        let (mut letter, blocks) = match built {
            Ok(built) => built,
            Err(error) => return self.letter_failed(read, error),
        };
        let pictures = (letter.body.as_mut())
            .map(|body| std::mem::take(&mut body.pictures))
            .unwrap_or_default();
        if let (Some(body), Some(blocks)) = (&letter.body, &blocks) {
            log::info!(
                "mail: read {read}: {} blocks, {} web pictures, {} trackers kept back",
                blocks.len(),
                body.remote.len(),
                body.trackers
            );
        }
        let open = Open {
            letter,
            blocks: blocks.unwrap_or_default().into(),
            pictures: Rc::new(pictures),
            trusted,
            fetching: false,
        };
        let Some(shown) = &mut self.reading.shown else {
            return;
        };
        shown.letter = Some(Ok(open));
        shown.version = Some(version);
        if trusted {
            self.load_pictures(cx);
        }
        cx.notify();
    }

    /// Fetches the open letter's web pictures, once.
    pub fn load_pictures(&mut self, cx: &mut Context<Self>) {
        let read = self.reading.read;
        let Some(Shown {
            letter: Some(Ok(open)),
            ..
        }) = &mut self.reading.shown
        else {
            return;
        };
        let Some(urls) = open.body().map(|body| body.remote.clone()) else {
            return;
        };
        if open.fetching || urls.is_empty() {
            return;
        }
        open.fetching = true;
        log::info!("mail: read {read}: fetching {} web pictures", urls.len());
        let words = words(cx);
        let fetched = cx.background_spawn(async move { fetch_all(urls, &words) });
        cx.spawn(async move |this, cx| {
            let fetched = fetched.await;
            this.update(cx, |mail, cx| mail.pictures_fetched(read, fetched, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn pictures_fetched(&mut self, read: u64, fetched: Fetched, cx: &mut Context<Self>) {
        if self.reading.read != read {
            return log::info!("mail: pictures for read {read} came after a newer read");
        }
        let Some(Shown {
            letter: Some(Ok(open)),
            ..
        }) = &mut self.reading.shown
        else {
            return;
        };
        let failed = fetched.iter().filter(|(_, got)| got.is_err()).count();
        log::info!(
            "mail: read {read}: {} pictures came, {failed} did not",
            fetched.len() - failed
        );
        let mut pictures = (*open.pictures).clone();
        pictures.extend(fetched);
        open.pictures = Rc::new(pictures);
        cx.notify();
    }

    /// Always loads, or no longer loads, the sender's pictures.
    pub fn trust_sender(&mut self, trusted: bool, cx: &mut Context<Self>) {
        let Some(Shown {
            letter: Some(Ok(open)),
            ..
        }) = &self.reading.shown
        else {
            return;
        };
        let Some(sender) = open.sender().map(str::to_owned) else {
            return;
        };
        let read = self.reading.read;
        self.with_store(
            cx,
            move |store| store.set_trusted(&sender, trusted),
            move |mail, done, cx| {
                if let Err(error) = done {
                    return mail.failed("mail.error.trust", &error, cx);
                }
                if mail.reading.read != read {
                    return;
                }
                if let Some(Shown {
                    letter: Some(Ok(open)),
                    ..
                }) = &mut mail.reading.shown
                {
                    open.trusted = trusted;
                }
                if trusted {
                    mail.load_pictures(cx);
                }
            },
        );
    }

    pub fn toggle_headers(&mut self, cx: &mut Context<Self>) {
        if let Some(shown) = &mut self.reading.shown {
            shown.headers = !shown.headers;
            cx.notify();
        }
    }
}

type Fetched = Vec<(String, Result<HeldPicture, SharedString>)>;

/// Blocking fetches, a few at once; never logs addresses.
fn fetch_all(urls: Vec<String>, words: &Words) -> Fetched {
    let chunk = urls.len().div_ceil(FETCHERS);
    thread::scope(|scope| {
        let workers: Vec<_> = urls
            .chunks(chunk)
            .map(|urls| {
                scope.spawn(move || {
                    let got = urls.iter().map(|url| {
                        let picture = fetch::picture(url)
                            .map_err(|err| SharedString::from(err.to_string()))
                            .and_then(|(kind, bytes)| letter::held(&kind, bytes, words));
                        (url.clone(), picture)
                    });
                    got.collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a picture fetch panicked"))
            .collect()
    })
}
