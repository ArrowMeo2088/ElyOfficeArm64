//! Message lists: one mailbox or every inbox, newest first.

use std::collections::HashMap;

use rusqlite::ToSql;

use super::{AccountId, MailboxId, MessageId, Store};
use crate::{Error, Mailbox, Result, Role};

/// Which messages a list holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Mailbox(MailboxId),
    /// Every account's inbox together.
    Inboxes,
}

/// Each set field narrows the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub unread: bool,
    pub flagged: bool,
    pub attachments: bool,
}

/// A message as a list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub id: MessageId,
    pub account: AccountId,
    /// Where it shows, a move not yet made included.
    pub mailbox: MailboxId,
    pub role: Option<Role>,
    pub pop: bool,
    pub from: Vec<Mailbox>,
    pub subject: Option<String>,
    pub date: Option<i64>,
    pub seen: bool,
    pub flagged: bool,
    /// None until the body is here.
    pub attachments: Option<u32>,
}

impl Filter {
    /// The narrowing, as SQL over `m`.
    pub(super) fn clauses(&self) -> Vec<String> {
        let mut clauses = Vec::new();
        if self.unread {
            clauses.push(format!("NOT {SEEN}"));
        }
        if self.flagged {
            clauses.push(FLAGGED.to_owned());
        }
        if self.attachments {
            clauses.push("m.attachments > 0".to_owned());
        }
        clauses
    }
}

// RFC 3501 §2.3.2: system flags match in any case.
const SEEN: &str = "EXISTS (SELECT 1 FROM flags f WHERE f.message = m.id \
                    AND f.flag = '\\Seen' COLLATE NOCASE)";
const FLAGGED: &str = "EXISTS (SELECT 1 FROM flags f WHERE f.message = m.id \
                       AND f.flag = '\\Flagged' COLLATE NOCASE)";

impl Store {
    /// Newest first; SQLite sorts undated last.
    pub fn list(&self, scope: Scope, filter: Filter) -> Result<Vec<Summary>> {
        let mut clauses = vec!["NOT m.erasing".to_owned()];
        clauses.push(match scope {
            Scope::Mailbox(_) => "coalesce(m.moving, m.mailbox) = ?1".to_owned(),
            Scope::Inboxes => "b.role = 'inbox'".to_owned(),
        });
        clauses.extend(filter.clauses());
        let id;
        let values: Vec<&dyn ToSql> = match scope {
            Scope::Mailbox(mailbox) => {
                id = mailbox.0;
                vec![&id]
            }
            Scope::Inboxes => Vec::new(),
        };
        self.summaries(&clauses.join(" AND "), &values)
    }

    /// Messages meeting `clauses`, newest first.
    pub(super) fn summaries(&self, clauses: &str, values: &[&dyn ToSql]) -> Result<Vec<Summary>> {
        let sql = format!(
            "SELECT m.id, m.sender, m.subject, m.date, m.attachments, {SEEN}, {FLAGGED},
                    b.account, b.id, b.role, m.uidl IS NOT NULL
             FROM messages m JOIN mailboxes b ON b.id = coalesce(m.moving, m.mailbox)
             WHERE {clauses}
             ORDER BY m.date DESC, m.id DESC"
        );
        let mut query = self.conn.prepare(&sql)?;
        let rows = query.query_map(values, |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                (
                    row.get(7)?,
                    row.get(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get(10)?,
                ),
            ))
        })?;
        rows.map(|row| {
            let (id, from, subject, date, attachments, seen, flagged, place) = row?;
            let (account, mailbox, role, pop) = place;
            let role = role
                .map(|role| {
                    Role::from_name(&role)
                        .ok_or_else(|| Error::Store(format!("an unknown folder role {role:?}")))
                })
                .transpose()?;
            Ok(Summary {
                id: MessageId(id),
                account: AccountId(account),
                mailbox: MailboxId(mailbox),
                role,
                pop,
                from: serde_json::from_str(&from)?,
                subject,
                date,
                seen,
                flagged,
                attachments,
            })
        })
        .collect()
    }

    /// Unread messages per mailbox; mailboxes with none are left out.
    pub fn unread(&self) -> Result<HashMap<MailboxId, u32>> {
        let mut query = self.conn.prepare(&format!(
            "SELECT coalesce(m.moving, m.mailbox) AS shown, COUNT(*) FROM messages m
             WHERE NOT m.erasing AND NOT {SEEN} GROUP BY shown"
        ))?;
        let rows = query.query_map([], |row| Ok((MailboxId(row.get(0)?), row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
