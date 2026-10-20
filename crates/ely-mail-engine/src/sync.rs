//! IMAP sync: folders, flags, deletions and the 90-day window (§12.3).

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard},
};

use crate::{
    Account, AccountId, Error, Fetched, Got, Held, Imap, Key, MailboxId, Result, Secret, Store,
    TlsConnector, Vault, oauth,
};

/// Bodies come down for mail this recent.
pub const BODY_DAYS: i64 = 90;

/// UIDs per FETCH.
const BATCH: usize = 100;

/// What one sync changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub folders: usize,
    pub headers: usize,
    pub bodies: usize,
    pub removed: usize,
}

/// The password, or an access token from the refresh token.
pub async fn secret(account: &Account, vault: &Vault) -> Result<Secret> {
    let name = account.secret_name();
    let kept = vault
        .get(&name)?
        .ok_or_else(|| Error::Vault(format!("no saved secret for {name}")))?;
    let Some(provider) = account.auth.provider() else {
        return Ok(Secret::Password(kept));
    };
    let tokens = oauth::refresh(&provider?, &kept).await?;
    if let Some(next) = &tokens.refresh {
        vault.put(&name, next);
    }
    Ok(Secret::Token(tokens.access))
}

/// One pass over every folder; `now` is Unix seconds.
pub async fn imap(
    account: &Account,
    id: AccountId,
    secret: Secret,
    tls: &TlsConnector,
    store: &Mutex<Store>,
    now: i64,
) -> Result<Report> {
    let mut imap = Imap::connect(&account.incoming, &account.login(secret), tls).await?;
    let folders: Vec<_> = imap
        .folders()
        .await?
        .into_iter()
        .filter(|folder| folder.selectable)
        .collect();
    let named: Vec<_> = folders
        .iter()
        .map(|folder| (folder.name.as_str(), folder.role))
        .collect();
    let boxes = lock(store)?.set_folders(id, &named)?;
    let day = since(now)?;
    let mut report = Report {
        folders: folders.len(),
        ..Report::default()
    };
    for (folder, mailbox) in folders.iter().zip(boxes) {
        one(&mut imap, store, mailbox, &folder.name, &day, &mut report).await?;
    }
    imap.logout().await?;
    lock(store)?.set_synced(id, now)?;
    log::info!(
        "mail: account {} synced: {} folders, {} headers, {} bodies, {} removed",
        id.0,
        report.folders,
        report.headers,
        report.bodies,
        report.removed
    );
    Ok(report)
}

async fn one(
    imap: &mut Imap,
    store: &Mutex<Store>,
    mailbox: MailboxId,
    name: &str,
    day: &str,
    report: &mut Report,
) -> Result<()> {
    let opened = imap.examine(name).await?;
    let held = lock(store)?.open_mailbox(mailbox, opened.uidvalidity)?;
    if opened.exists == 0 {
        return remove(store, held.values(), report);
    }
    let server: HashMap<u32, Vec<String>> = imap.flags().await?.into_iter().collect();
    let gone = held
        .iter()
        .filter(|(uid, _)| !server.contains_key(uid))
        .map(|(_, held)| held);
    remove(store, gone, report)?;
    for (uid, held) in &held {
        match server.get(uid) {
            Some(flags) if *flags != held.flags => {
                lock(store)?.set_flags(held.id, &strs(flags))?;
            }
            _ => {}
        }
    }
    let recent: HashSet<u32> = imap.since(day).await?.into_iter().collect();
    let mut headers = Vec::new();
    let mut bodies = Vec::new();
    for uid in server.keys() {
        match (held.get(uid), recent.contains(uid)) {
            (None | Some(Held { body: false, .. }), true) => bodies.push(*uid),
            (None, false) => headers.push(*uid),
            _ => {}
        }
    }
    headers.sort_unstable();
    bodies.sort_unstable();
    let key = |uid| Key::Imap {
        uidvalidity: opened.uidvalidity,
        uid,
    };
    for batch in headers.chunks(BATCH) {
        for got in imap.headers(batch).await? {
            put(
                store,
                mailbox,
                key(got.uid),
                &got,
                Fetched::Headers(&got.bytes),
            )?;
            report.headers += 1;
        }
    }
    for batch in bodies.chunks(BATCH) {
        let (whole, partial): (Vec<_>, Vec<_>) = imap
            .structures(batch)
            .await?
            .into_iter()
            .partition(|(_, node)| node.complete());
        let whole: Vec<u32> = whole.into_iter().map(|(uid, _)| uid).collect();
        if !whole.is_empty() {
            for got in imap.bodies(&whole).await? {
                put(
                    store,
                    mailbox,
                    key(got.uid),
                    &got,
                    Fetched::Whole(&got.bytes),
                )?;
                report.bodies += 1;
            }
        }
        // §12.3: attachments wait for a request.
        for (uid, node) in partial {
            let Some(got) = imap.text(uid, &node).await? else {
                continue;
            };
            let missing = node.missing();
            let text = Fetched::Text {
                bytes: &got.bytes,
                missing: &missing,
            };
            put(store, mailbox, key(uid), &got, text)?;
            report.bodies += 1;
        }
    }
    Ok(())
}

fn put(
    store: &Mutex<Store>,
    mailbox: MailboxId,
    key: Key,
    got: &Got,
    fetched: Fetched,
) -> Result<()> {
    lock(store)?.put(mailbox, &key, &strs(&got.flags), fetched)?;
    Ok(())
}

fn remove<'a>(
    store: &Mutex<Store>,
    gone: impl Iterator<Item = &'a Held>,
    report: &mut Report,
) -> Result<()> {
    for held in gone {
        lock(store)?.remove(held.id)?;
        report.removed += 1;
    }
    Ok(())
}

fn strs(flags: &[String]) -> Vec<&str> {
    flags.iter().map(String::as_str).collect()
}

/// `SINCE` takes a date in English: `19-Jul-2026`.
fn since(now: i64) -> Result<String> {
    let start = jiff::Timestamp::from_second(now - BODY_DAYS * 86_400)
        .map_err(|err| Error::Store(format!("a sync time: {err}")))?;
    Ok(start
        .to_zoned(jiff::tz::TimeZone::UTC)
        .strftime("%-d-%b-%Y")
        .to_string())
}

fn lock(store: &Mutex<Store>) -> Result<MutexGuard<'_, Store>> {
    store
        .lock()
        .map_err(|_| Error::Store("a store task panicked".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_starts_ninety_days_back() {
        // 2026-10-19 12:00 UTC
        assert_eq!(super::since(1_792_411_200).unwrap(), "21-Jul-2026");
    }
}
