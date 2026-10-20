//! IMAP folders as mailboxes, and how far they are synced.

use std::collections::{HashMap, HashSet};

use rusqlite::{Transaction, params};

use super::{AccountId, MailboxId, MessageId, Store, aside::Aside, mailbox_name};
use crate::{Error, Result, Role};

/// A local IMAP message as sync compares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub id: MessageId,
    /// The body is here, perhaps without attachments.
    pub body: bool,
    /// Every part is here.
    pub whole: bool,
    /// Sorted.
    pub flags: Vec<String>,
}

/// A local mailbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFolder {
    pub id: MailboxId,
    pub name: String,
    pub role: Option<Role>,
    /// Every message comes down whole.
    pub offline: bool,
}

/// An account's sync, as the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Synced {
    /// Unix seconds of the last full sync.
    pub at: Option<i64>,
    pub messages: u64,
    /// Messages with only their header here.
    pub uncached: u64,
    /// Messages whose attachments wait for a request.
    pub partial: u64,
}

impl Store {
    /// Keeps the listed folders; the rest go with their mail.
    pub fn set_folders(
        &mut self,
        account: AccountId,
        folders: &[(&str, Option<Role>)],
    ) -> Result<Vec<MailboxId>> {
        self.writable()?;
        let held: Vec<(i64, String)> = self
            .conn
            .prepare("SELECT id, name FROM mailboxes WHERE account = ?1")?
            .query_map([account.0], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, name) in held {
            if !folders
                .iter()
                .any(|(listed, _)| mailbox_name(listed) == name)
            {
                self.without_mail(MailboxId(id), None, |tx| {
                    tx.execute("DELETE FROM mailboxes WHERE id = ?1", [id])
                })?;
                log::info!("mail: mailbox {id} is no longer listed; removed");
            }
        }
        folders
            .iter()
            .map(|(name, role)| {
                let id = self.mailbox(account, name)?;
                self.conn.execute(
                    "UPDATE mailboxes SET role = ?1 WHERE id = ?2",
                    params![role.map(Role::name), id.0],
                )?;
                Ok(id)
            })
            .collect()
    }

    /// The account's mailboxes in name order.
    pub fn folders(&self, account: AccountId) -> Result<Vec<LocalFolder>> {
        let mut query = self.conn.prepare(
            "SELECT id, name, role, offline FROM mailboxes WHERE account = ?1 ORDER BY name",
        )?;
        let rows = query.query_map([account.0], |row| {
            Ok((
                MailboxId(row.get(0)?),
                row.get(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, role, offline) = row?;
            let role = role
                .map(|role| {
                    Role::from_name(&role)
                        .ok_or_else(|| Error::Store(format!("an unknown folder role {role:?}")))
                })
                .transpose()?;
            Ok(LocalFolder {
                id,
                name,
                role,
                offline,
            })
        })
        .collect()
    }

    pub fn account_of(&self, mailbox: MailboxId) -> Result<AccountId> {
        Ok(AccountId(self.conn.query_row(
            "SELECT account FROM mailboxes WHERE id = ?1",
            [mailbox.0],
            |row| row.get(0),
        )?))
    }

    /// Whether the mailbox's mail comes down whole.
    pub fn offline(&self, mailbox: MailboxId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT offline FROM mailboxes WHERE id = ?1",
            [mailbox.0],
            |row| row.get(0),
        )?)
    }

    pub fn set_offline(&self, mailbox: MailboxId, offline: bool) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE mailboxes SET offline = ?1 WHERE id = ?2",
            params![offline, mailbox.0],
        )?;
        if changed != 1 {
            return Err(Error::Store(format!("no mailbox {}", mailbox.0)));
        }
        log::info!("mail: mailbox {} offline: {offline}", mailbox.0);
        Ok(())
    }

    /// Held messages by UID; mail under another UIDVALIDITY goes.
    pub fn open_mailbox(
        &mut self,
        mailbox: MailboxId,
        uidvalidity: u32,
    ) -> Result<HashMap<u32, Held>> {
        self.writable()?;
        let known: Option<u32> = self.conn.query_row(
            "SELECT uidvalidity FROM mailboxes WHERE id = ?1",
            [mailbox.0],
            |row| row.get(0),
        )?;
        if known != Some(uidvalidity) {
            if known.is_some() {
                log::warn!("mail: mailbox {} has a new UIDVALIDITY; rebuilt", mailbox.0);
            }
            self.without_mail(mailbox, Some(uidvalidity), |tx| {
                tx.execute(
                    "UPDATE mailboxes SET uidvalidity = ?1 WHERE id = ?2",
                    params![uidvalidity, mailbox.0],
                )
            })?;
        }
        let mut held: HashMap<u32, Held> = HashMap::new();
        let mut query = self.conn.prepare(
            "SELECT id, uid, content IS NOT NULL, content IS NOT NULL AND missing = '[]'
             FROM messages WHERE mailbox = ?1 AND uidvalidity = ?2",
        )?;
        let rows = query.query_map(params![mailbox.0, uidvalidity], |row| {
            let id = MessageId(row.get(0)?);
            Ok((row.get::<_, u32>(1)?, id, row.get(2)?, row.get(3)?))
        })?;
        for row in rows {
            let (uid, id, body, whole) = row?;
            held.insert(
                uid,
                Held {
                    id,
                    body,
                    whole,
                    flags: Vec::new(),
                },
            );
        }
        let mut query = self.conn.prepare(
            "SELECT messages.uid, flags.flag FROM flags JOIN messages ON flags.message = messages.id
             WHERE messages.mailbox = ?1 AND messages.uidvalidity = ?2 ORDER BY flags.flag",
        )?;
        let rows = query.query_map(params![mailbox.0, uidvalidity], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (uid, flag) = row?;
            held.get_mut(&uid)
                .ok_or_else(|| Error::Store(format!("a flag for no message {uid}")))?
                .flags
                .push(flag);
        }
        Ok(held)
    }

    /// The UIDLs a POP3 mailbox holds.
    pub fn uidls(&self, mailbox: MailboxId) -> Result<HashSet<String>> {
        let mut query = self
            .conn
            .prepare("SELECT uidl FROM messages WHERE mailbox = ?1 AND uidl IS NOT NULL")?;
        let rows = query.query_map([mailbox.0], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_synced(&self, account: AccountId, at: i64) -> Result<()> {
        self.writable()?;
        let changed = self.conn.execute(
            "UPDATE accounts SET synced = ?1 WHERE id = ?2",
            params![at, account.0],
        )?;
        if changed != 1 {
            return Err(Error::Store(format!("no account {}", account.0)));
        }
        Ok(())
    }

    pub fn synced(&self, account: AccountId) -> Result<Synced> {
        let at = self.conn.query_row(
            "SELECT synced FROM accounts WHERE id = ?1",
            [account.0],
            |row| row.get(0),
        )?;
        let (messages, cached, partial): (i64, i64, i64) = self.conn.query_row(
            "SELECT count(*), count(content), count(CASE WHEN missing <> '[]' THEN 1 END)
             FROM messages JOIN mailboxes ON messages.mailbox = mailboxes.id
             WHERE mailboxes.account = ?1",
            [account.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let count =
            |n: i64| u64::try_from(n).map_err(|_| Error::Store(format!("a negative count: {n}")));
        Ok(Synced {
            at,
            messages: count(messages)?,
            uncached: count(messages - cached)?,
            partial: count(partial)?,
        })
    }

    /// Deletes the mailbox's messages not under `keep`, with `change`.
    fn without_mail(
        &mut self,
        mailbox: MailboxId,
        keep: Option<u32>,
        change: impl FnOnce(&Transaction) -> rusqlite::Result<usize>,
    ) -> Result<()> {
        let hashes: Vec<String> = self
            .conn
            .prepare(&format!(
                "SELECT DISTINCT content FROM messages WHERE {STALE} AND content IS NOT NULL"
            ))?
            .query_map(params![mailbox.0, keep], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let tx = self.conn.transaction()?;
        let mut aside = Aside::new(&self.dir);
        let done = (|| -> Result<()> {
            tx.execute(
                &format!("DELETE FROM messages WHERE {STALE}"),
                params![mailbox.0, keep],
            )?;
            change(&tx)?;
            for hash in &hashes {
                aside.drop_unused(&tx, hash)?;
            }
            Ok(tx.commit()?)
        })();
        aside.settle(done)
    }
}

const STALE: &str = "mailbox = ?1 AND (?2 IS NULL OR uidvalidity IS NOT ?2)";
