use std::{fs, io::ErrorKind, path::Path};

use rusqlite::{Connection, OptionalExtension, Row, ToSql, Transaction, params};
use sha2::{Digest, Sha256};

use super::{CONTENT, MailboxId, MessageId, Store, disk, search};
use crate::{Error, Mail, Mailbox, Result, read};

/// RFC 3501 §2.3.1.1: identity is mailbox, UIDVALIDITY and UID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Imap {
        uidvalidity: u32,
        uid: u32,
    },
    /// POP3 has one mailbox and UIDL names.
    Pop {
        uidl: String,
    },
}

/// What a sync brought down.
pub enum Fetched<'a> {
    Headers(&'a [u8]),
    Whole(&'a [u8]),
}

/// A message as indexed; the original is read apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub id: MessageId,
    pub mailbox: MailboxId,
    pub key: Key,
    pub flags: Vec<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Vec<String>,
    pub references: Vec<String>,
    pub subject: Option<String>,
    pub from: Vec<Mailbox>,
    pub to: Vec<Mailbox>,
    pub cc: Vec<Mailbox>,
    pub date: Option<i64>,
    /// None until the whole message is here.
    pub attachments: Option<u32>,
    pub whole: bool,
}

const COLUMNS: &str = "id, mailbox, uidvalidity, uid, uidl, message_id, in_reply_to, refs, \
                       subject, sender, recipients, copies, date, attachments, content";

impl Store {
    pub fn find(&self, mailbox: MailboxId, key: &Key) -> Result<Option<MessageId>> {
        let found = match key {
            Key::Imap { uidvalidity, uid } => self.conn.query_row(
                "SELECT id FROM messages WHERE mailbox = ?1 AND uidvalidity = ?2 AND uid = ?3",
                params![mailbox.0, uidvalidity, uid],
                |row| row.get(0),
            ),
            Key::Pop { uidl } => self.conn.query_row(
                "SELECT id FROM messages WHERE mailbox = ?1 AND uidl = ?2",
                params![mailbox.0, uidl],
                |row| row.get(0),
            ),
        };
        Ok(found.optional()?.map(MessageId))
    }

    /// One key, one message; headers again only set flags.
    pub fn put(
        &mut self,
        mailbox: MailboxId,
        key: &Key,
        flags: &[&str],
        fetched: Fetched,
    ) -> Result<MessageId> {
        self.writable()?;
        let existing = self.find(mailbox, key)?;
        let bytes = match (fetched, existing) {
            (Fetched::Headers(_), Some(id)) => {
                self.set_flags(id, flags)?;
                return Ok(id);
            }
            (Fetched::Headers(bytes), None) => {
                let mail = read(bytes)?;
                return self.write(mailbox, key, flags, &mail, None, None);
            }
            (Fetched::Whole(bytes), _) => bytes,
        };
        let mail = read(bytes)?;
        let (content, made) = self.keep(bytes)?;
        let written = self.write(mailbox, key, flags, &mail, Some(&content), existing);
        if written.is_err()
            && made
            && let Err(gone) = drop_unused(&self.conn, &self.dir, &content)
        {
            log::error!("mail: an unused original stays after a failed write: {gone}");
        }
        written
    }

    fn write(
        &mut self,
        mailbox: MailboxId,
        key: &Key,
        flags: &[&str],
        mail: &Mail,
        content: Option<&str>,
        existing: Option<MessageId>,
    ) -> Result<MessageId> {
        let (uidvalidity, uid, uidl) = match key {
            Key::Imap { uidvalidity, uid } => (Some(*uidvalidity), Some(*uid), None),
            Key::Pop { uidl } => (None, None, Some(uidl.as_str())),
        };
        let attachments = content.map(|_| mail.attachments.len() as u32);
        let json = [
            serde_json::to_string(&mail.in_reply_to)?,
            serde_json::to_string(&mail.references)?,
            serde_json::to_string(&mail.from)?,
            serde_json::to_string(&mail.to)?,
            serde_json::to_string(&mail.cc)?,
        ];
        let mut values: Vec<&dyn ToSql> = vec![
            &mail.message_id,
            &json[0],
            &json[1],
            &mail.subject,
            &json[2],
            &json[3],
            &json[4],
            &mail.date,
            &attachments,
            &content,
        ];
        let tx = self.conn.transaction()?;
        let mut old: Option<String> = None;
        let id = match existing {
            Some(id) => {
                old = tx.query_row(
                    "SELECT content FROM messages WHERE id = ?1",
                    [id.0],
                    |row| row.get(0),
                )?;
                values.push(&id.0);
                tx.execute(
                    "UPDATE messages SET message_id = ?1, in_reply_to = ?2, refs = ?3,
                     subject = ?4, sender = ?5, recipients = ?6, copies = ?7, date = ?8,
                     attachments = ?9, content = ?10 WHERE id = ?11",
                    values.as_slice(),
                )?;
                id
            }
            None => {
                values.extend::<[&dyn ToSql; 4]>([&mailbox.0, &uidvalidity, &uid, &uidl]);
                tx.execute(
                    "INSERT INTO messages (message_id, in_reply_to, refs, subject, sender,
                     recipients, copies, date, attachments, content, mailbox, uidvalidity,
                     uid, uidl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     ?13, ?14)",
                    values.as_slice(),
                )?;
                MessageId(tx.last_insert_rowid())
            }
        };
        replace_flags(&tx, id, flags)?;
        search::index(&tx, id, mail, content.is_some())?;
        if let Some(old) = old.filter(|old| Some(old.as_str()) != content) {
            drop_unused(&tx, &self.dir, &old)?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn set_flags(&mut self, id: MessageId, flags: &[&str]) -> Result<()> {
        self.writable()?;
        self.content(id)?;
        let tx = self.conn.transaction()?;
        replace_flags(&tx, id, flags)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: MessageId) -> Result<Stored> {
        let mut stored = self.conn.query_row(
            &format!("SELECT {COLUMNS} FROM messages WHERE id = ?1"),
            [id.0],
            |found| Ok(row(found)),
        )??;
        let mut query = self
            .conn
            .prepare("SELECT flag FROM flags WHERE message = ?1 ORDER BY flag")?;
        stored.flags = query
            .query_map([id.0], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(stored)
    }

    /// The original, checked against its hash; None for headers only.
    pub fn original(&self, id: MessageId) -> Result<Option<Vec<u8>>> {
        self.content(id)?
            .map(|hash| load_at(&self.dir, &hash))
            .transpose()
    }

    pub fn remove(&mut self, id: MessageId) -> Result<()> {
        self.writable()?;
        let content = self.content(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM messages WHERE id = ?1", [id.0])?;
        if let Some(hash) = content {
            drop_unused(&tx, &self.dir, &hash)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn content(&self, id: MessageId) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT content FROM messages WHERE id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::Store(format!("no message {}", id.0)))
    }

    /// The hash, and whether the file is new.
    fn keep(&self, bytes: &[u8]) -> Result<(String, bool)> {
        let hash = hex(&Sha256::digest(bytes));
        let path = self.dir.join(CONTENT).join(&hash[..2]).join(&hash);
        let made = match fs::read(&path) {
            Ok(held) if hex(&Sha256::digest(&held)) == hash => return Ok((hash, false)),
            Ok(_) => {
                log::warn!("mail: original {hash} was damaged; rewritten from the server copy");
                false
            }
            Err(err) if err.kind() == ErrorKind::NotFound => true,
            Err(err) => return Err(disk("checking an original", err)),
        };
        let shard = path.parent().expect("a shard folder");
        fs::create_dir_all(shard).map_err(|err| disk("storing an original", err))?;
        let staged = shard.join(format!(".{hash}.part"));
        if let Err(err) = fs::write(&staged, bytes).and_then(|()| fs::rename(&staged, &path)) {
            return Err(match fs::remove_file(&staged) {
                Ok(()) => disk("storing an original", err),
                Err(gone) if gone.kind() == ErrorKind::NotFound => disk("storing an original", err),
                Err(left) => Error::Store(format!(
                    "storing an original: {err}; the staged copy stayed: {left}"
                )),
            });
        }
        Ok((hash, made))
    }
}

/// Deletes the file once no row names it; runs before commit.
fn drop_unused(conn: &Connection, dir: &Path, hash: &str) -> Result<()> {
    let used: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM messages WHERE content = ?1)",
        [hash],
        |row| row.get(0),
    )?;
    if used {
        return Ok(());
    }
    let path = dir.join(CONTENT).join(&hash[..2]).join(hash);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            log::warn!("mail: original {hash} was already gone");
            Ok(())
        }
        Err(err) => Err(disk("deleting an original", err)),
    }
}

fn replace_flags(tx: &Transaction, id: MessageId, flags: &[&str]) -> Result<()> {
    tx.execute("DELETE FROM flags WHERE message = ?1", [id.0])?;
    for flag in flags {
        tx.execute(
            "INSERT OR IGNORE INTO flags (message, flag) VALUES (?1, ?2)",
            params![id.0, flag],
        )?;
    }
    Ok(())
}

fn row(row: &Row) -> Result<Stored> {
    let key = match row.get::<_, Option<String>>(4)? {
        Some(uidl) => Key::Pop { uidl },
        None => Key::Imap {
            uidvalidity: row.get(2)?,
            uid: row.get(3)?,
        },
    };
    let json = |at: usize| -> Result<String> { Ok(row.get(at)?) };
    Ok(Stored {
        id: MessageId(row.get(0)?),
        mailbox: MailboxId(row.get(1)?),
        key,
        flags: Vec::new(),
        message_id: row.get(5)?,
        in_reply_to: serde_json::from_str(&json(6)?)?,
        references: serde_json::from_str(&json(7)?)?,
        subject: row.get(8)?,
        from: serde_json::from_str(&json(9)?)?,
        to: serde_json::from_str(&json(10)?)?,
        cc: serde_json::from_str(&json(11)?)?,
        date: row.get(12)?,
        attachments: row.get(13)?,
        whole: row.get::<_, Option<String>>(14)?.is_some(),
    })
}

pub(super) fn load_at(dir: &Path, hash: &str) -> Result<Vec<u8>> {
    let path = dir.join(CONTENT).join(&hash[..2]).join(hash);
    let bytes = fs::read(&path).map_err(|err| disk("reading an original", err))?;
    if hex(&Sha256::digest(&bytes)) != hash {
        return Err(Error::Store(format!("original {hash} is damaged")));
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
