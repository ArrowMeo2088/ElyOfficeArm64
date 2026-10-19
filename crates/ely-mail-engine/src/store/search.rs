use rusqlite::{Transaction, params};

use super::{MessageId, Store, html, messages::load_at};
use crate::{Mail, Mailbox, Result, read};

/// Replaces the message's row in the full-text index.
pub(super) fn index(tx: &Transaction, id: MessageId, mail: &Mail, whole: bool) -> Result<()> {
    let people = people([&mail.from, &mail.to, &mail.cc]);
    let (body, files) = if whole {
        contents(id, mail)
    } else {
        Default::default()
    };
    put(tx, id, mail.subject.as_deref(), &people, &body, &files)
}

fn put(
    tx: &Transaction,
    id: MessageId,
    subject: Option<&str>,
    people: &str,
    body: &str,
    files: &str,
) -> Result<()> {
    tx.execute("DELETE FROM search WHERE rowid = ?1", [id.0])?;
    tx.execute(
        "INSERT INTO search (rowid, subject, people, body, files) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id.0, subject, people, body, files],
    )?;
    Ok(())
}

impl Store {
    /// Rebuilt from the index rows and the originals.
    pub fn rebuild_search(&mut self) -> Result<usize> {
        self.writable()?;
        let ids: Vec<i64> = {
            let mut query = self.conn.prepare("SELECT id FROM messages")?;
            query
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let tx = self.conn.transaction()?;
        // A damaged index may not delete.
        let made: String = tx.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search'",
            [],
            |row| row.get(0),
        )?;
        tx.execute_batch(&format!("DROP TABLE search; {made};"))?;
        for &id in &ids {
            let (subject, from, to, cc, content): (
                Option<String>,
                String,
                String,
                String,
                Option<String>,
            ) = tx.query_row(
                "SELECT subject, sender, recipients, copies, content FROM messages
                     WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;
            let lists: [Vec<Mailbox>; 3] = [
                serde_json::from_str(&from)?,
                serde_json::from_str(&to)?,
                serde_json::from_str(&cc)?,
            ];
            let (body, files) = match content {
                Some(hash) => contents(MessageId(id), &read(&load_at(&self.dir, &hash)?)?),
                None => Default::default(),
            };
            let people = people([&lists[0], &lists[1], &lists[2]]);
            put(
                &tx,
                MessageId(id),
                subject.as_deref(),
                &people,
                &body,
                &files,
            )?;
        }
        let count = ids.len();
        tx.commit()?;
        log::info!("mail: search index rebuilt for {count} messages");
        Ok(count)
    }
}

fn people(lists: [&[Mailbox]; 3]) -> String {
    let mut out = String::new();
    for person in lists.into_iter().flatten() {
        if let Some(name) = &person.name {
            out.push_str(name);
            out.push(' ');
        }
        out.push_str(&person.email);
        out.push('\n');
    }
    out
}

/// The body text and the attachment names.
fn contents(id: MessageId, mail: &Mail) -> (String, String) {
    let body = match (&mail.text, &mail.html) {
        (Some(text), _) => text.clone(),
        (None, Some(html)) => {
            let (text, short) = html::text(html);
            if short {
                log::warn!(
                    "mail: message {} nests HTML too deep; its later text is not indexed",
                    id.0
                );
            }
            text
        }
        (None, None) => String::new(),
    };
    let files = mail
        .attachments
        .iter()
        .filter_map(|part| part.name.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    (body, files)
}
