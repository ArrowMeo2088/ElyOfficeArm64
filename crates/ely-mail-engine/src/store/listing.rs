//! Message lists: one mailbox or every inbox, newest first.

use std::collections::HashMap;

use rusqlite::ToSql;

use super::{MailboxId, MessageId, Store};
use crate::{Mailbox, Result};

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
    pub from: Vec<Mailbox>,
    pub subject: Option<String>,
    pub date: Option<i64>,
    pub seen: bool,
    pub flagged: bool,
    /// None until the body is here.
    pub attachments: Option<u32>,
}

// RFC 3501 §2.3.2: system flags match in any case.
const SEEN: &str = "EXISTS (SELECT 1 FROM flags f WHERE f.message = m.id \
                    AND f.flag = '\\Seen' COLLATE NOCASE)";
const FLAGGED: &str = "EXISTS (SELECT 1 FROM flags f WHERE f.message = m.id \
                       AND f.flag = '\\Flagged' COLLATE NOCASE)";

impl Store {
    /// Newest first; SQLite sorts undated last.
    pub fn list(&self, scope: Scope, filter: Filter) -> Result<Vec<Summary>> {
        let mut clauses = vec![match scope {
            Scope::Mailbox(_) => "m.mailbox = ?1".to_owned(),
            Scope::Inboxes => "b.role = 'inbox'".to_owned(),
        }];
        if filter.unread {
            clauses.push(format!("NOT {SEEN}"));
        }
        if filter.flagged {
            clauses.push(FLAGGED.to_owned());
        }
        if filter.attachments {
            clauses.push("m.attachments > 0".to_owned());
        }
        let sql = format!(
            "SELECT m.id, m.sender, m.subject, m.date, m.attachments, {SEEN}, {FLAGGED}
             FROM messages m JOIN mailboxes b ON b.id = m.mailbox
             WHERE {}
             ORDER BY m.date DESC, m.id DESC",
            clauses.join(" AND ")
        );
        let id;
        let values: Vec<&dyn ToSql> = match scope {
            Scope::Mailbox(mailbox) => {
                id = mailbox.0;
                vec![&id]
            }
            Scope::Inboxes => Vec::new(),
        };
        let mut query = self.conn.prepare(&sql)?;
        let rows = query.query_map(values.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, from, subject, date, attachments, seen, flagged) = row?;
            Ok(Summary {
                id: MessageId(id),
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
            "SELECT m.mailbox, COUNT(*) FROM messages m WHERE NOT {SEEN} GROUP BY m.mailbox"
        ))?;
        let rows = query.query_map([], |row| Ok((MailboxId(row.get(0)?), row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
