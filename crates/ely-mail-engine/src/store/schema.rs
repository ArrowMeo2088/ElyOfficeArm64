/// Step `n` upgrades version `n`.
pub(super) const STEPS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7, V8, V9, V10];

const V1: &str = "
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    address TEXT NOT NULL UNIQUE COLLATE NOCASE,
    settings TEXT NOT NULL
);
CREATE TABLE mailboxes (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL REFERENCES accounts ON DELETE CASCADE,
    name TEXT NOT NULL,
    UNIQUE (account, name)
);
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    mailbox INTEGER NOT NULL REFERENCES mailboxes ON DELETE CASCADE,
    uidvalidity INTEGER,
    uid INTEGER,
    uidl TEXT,
    message_id TEXT,
    in_reply_to TEXT NOT NULL,
    refs TEXT NOT NULL,
    subject TEXT,
    sender TEXT NOT NULL,
    recipients TEXT NOT NULL,
    copies TEXT NOT NULL,
    date INTEGER,
    attachments INTEGER,
    content TEXT,
    CHECK ((uid IS NULL) = (uidvalidity IS NULL) AND (uid IS NULL) <> (uidl IS NULL))
);
CREATE UNIQUE INDEX messages_imap ON messages (mailbox, uidvalidity, uid)
    WHERE uid IS NOT NULL;
CREATE UNIQUE INDEX messages_pop ON messages (mailbox, uidl) WHERE uidl IS NOT NULL;
CREATE INDEX messages_thread ON messages (message_id);
CREATE INDEX messages_content ON messages (content);
CREATE TABLE flags (
    message INTEGER NOT NULL REFERENCES messages ON DELETE CASCADE,
    flag TEXT NOT NULL,
    PRIMARY KEY (message, flag)
) WITHOUT ROWID;
CREATE INDEX flags_by_flag ON flags (flag, message);
CREATE VIRTUAL TABLE search USING fts5(
    subject, people, body, files,
    content = '', contentless_delete = 1, tokenize = 'trigram'
);
CREATE TRIGGER messages_gone AFTER DELETE ON messages BEGIN
    DELETE FROM search WHERE rowid = old.id;
END;
";

/// IMAP sync (C12.6).
const V2: &str = "
ALTER TABLE accounts ADD COLUMN synced INTEGER;
ALTER TABLE mailboxes ADD COLUMN role TEXT;
ALTER TABLE mailboxes ADD COLUMN uidvalidity INTEGER;
ALTER TABLE messages ADD COLUMN missing TEXT NOT NULL DEFAULT '[]';
";

/// Folders kept whole (C12.8).
const V3: &str = "
ALTER TABLE mailboxes ADD COLUMN offline INTEGER NOT NULL DEFAULT 0;
";

/// Ids never reused (C12.9); foreign keys are off here.
const V4: &str = "
CREATE TABLE accounts_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    address TEXT NOT NULL UNIQUE COLLATE NOCASE,
    settings TEXT NOT NULL,
    synced INTEGER
);
CREATE TABLE mailboxes_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account INTEGER NOT NULL REFERENCES accounts ON DELETE CASCADE,
    name TEXT NOT NULL,
    role TEXT,
    uidvalidity INTEGER,
    offline INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account, name)
);
CREATE TABLE messages_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mailbox INTEGER NOT NULL REFERENCES mailboxes ON DELETE CASCADE,
    uidvalidity INTEGER,
    uid INTEGER,
    uidl TEXT,
    message_id TEXT,
    in_reply_to TEXT NOT NULL,
    refs TEXT NOT NULL,
    subject TEXT,
    sender TEXT NOT NULL,
    recipients TEXT NOT NULL,
    copies TEXT NOT NULL,
    date INTEGER,
    attachments INTEGER,
    content TEXT,
    missing TEXT NOT NULL DEFAULT '[]',
    CHECK ((uid IS NULL) = (uidvalidity IS NULL) AND (uid IS NULL) <> (uidl IS NULL))
);
INSERT INTO accounts_new (id, address, settings, synced)
    SELECT id, address, settings, synced FROM accounts;
INSERT INTO mailboxes_new (id, account, name, role, uidvalidity, offline)
    SELECT id, account, name, role, uidvalidity, offline FROM mailboxes;
INSERT INTO messages_new (id, mailbox, uidvalidity, uid, uidl, message_id, in_reply_to,
        refs, subject, sender, recipients, copies, date, attachments, content, missing)
    SELECT id, mailbox, uidvalidity, uid, uidl, message_id, in_reply_to,
        refs, subject, sender, recipients, copies, date, attachments, content, missing
    FROM messages;
DROP TABLE messages;
DROP TABLE mailboxes;
DROP TABLE accounts;
ALTER TABLE accounts_new RENAME TO accounts;
ALTER TABLE mailboxes_new RENAME TO mailboxes;
ALTER TABLE messages_new RENAME TO messages;
CREATE UNIQUE INDEX messages_imap ON messages (mailbox, uidvalidity, uid)
    WHERE uid IS NOT NULL;
CREATE UNIQUE INDEX messages_pop ON messages (mailbox, uidl) WHERE uidl IS NOT NULL;
CREATE INDEX messages_thread ON messages (message_id);
CREATE INDEX messages_content ON messages (content);
CREATE TRIGGER messages_gone AFTER DELETE ON messages BEGIN
    DELETE FROM search WHERE rowid = old.id;
END;
";

/// Senders whose remote pictures load (C12.10).
const V5: &str = "
CREATE TABLE trusted (address TEXT PRIMARY KEY COLLATE NOCASE) WITHOUT ROWID;
";

/// Headers-only messages keep their header block (C12.10).
const V6: &str = "
ALTER TABLE messages ADD COLUMN head BLOB;
-- Sync fetches these again, with their headers.
DELETE FROM flags WHERE message IN (SELECT id FROM messages WHERE content IS NULL);
DELETE FROM messages WHERE content IS NULL;
";

/// Each message's own and named Message-IDs (C12.11).
const V7: &str = "
CREATE TABLE links (
    message INTEGER NOT NULL REFERENCES messages ON DELETE CASCADE,
    id TEXT NOT NULL,
    PRIMARY KEY (id, message)
) WITHOUT ROWID;
CREATE INDEX links_by_message ON links (message);
INSERT OR IGNORE INTO links (message, id)
    SELECT id, message_id FROM messages WHERE message_id <> '';
INSERT OR IGNORE INTO links (message, id)
    SELECT m.id, j.value FROM messages m, json_each(m.in_reply_to) j WHERE j.value <> '';
INSERT OR IGNORE INTO links (message, id)
    SELECT m.id, j.value FROM messages m, json_each(m.refs) j WHERE j.value <> '';
";

/// Each account's sender name and signature (C12.12).
const V8: &str = "
CREATE TABLE identities (
    account INTEGER PRIMARY KEY REFERENCES accounts ON DELETE CASCADE,
    name TEXT,
    signature TEXT NOT NULL
);
";

/// Messages being written, and their server copies (C12.13).
const V9: &str = "
ALTER TABLE accounts ADD COLUMN drafts_up INTEGER NOT NULL DEFAULT 0;
CREATE TABLE drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account INTEGER NOT NULL REFERENCES accounts ON DELETE CASCADE,
    saved INTEGER NOT NULL,
    body TEXT NOT NULL,
    remote_mailbox TEXT,
    remote_validity INTEGER,
    remote_uid INTEGER,
    CHECK ((remote_mailbox IS NULL) = (remote_uid IS NULL)
        AND (remote_uid IS NULL) = (remote_validity IS NULL))
);
CREATE TABLE draft_files (
    draft INTEGER NOT NULL REFERENCES drafts ON DELETE CASCADE,
    place INTEGER NOT NULL,
    name TEXT,
    content_type TEXT NOT NULL,
    params TEXT NOT NULL,
    content_id TEXT,
    bytes BLOB NOT NULL,
    PRIMARY KEY (draft, place)
) WITHOUT ROWID;
";

/// Messages handed to the queue, until done (C12.14).
const V10: &str = "
ALTER TABLE accounts ADD COLUMN sent_copy INTEGER;
ALTER TABLE accounts ADD COLUMN smtp_limit INTEGER;
CREATE TABLE outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account INTEGER NOT NULL REFERENCES accounts ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN
        ('queued', 'sending', 'accepted', 'failed', 'unsure', 'cancelled')),
    filing TEXT CHECK (filing IN ('waiting', 'failed')),
    message_id TEXT NOT NULL,
    body TEXT NOT NULL,
    envelope TEXT NOT NULL,
    bytes BLOB NOT NULL,
    queued INTEGER NOT NULL,
    tried INTEGER,
    error TEXT,
    CHECK ((filing IS NULL) <> (state = 'accepted'))
);
CREATE TABLE outbox_files (
    item INTEGER NOT NULL REFERENCES outbox ON DELETE CASCADE,
    place INTEGER NOT NULL,
    name TEXT,
    content_type TEXT NOT NULL,
    params TEXT NOT NULL,
    content_id TEXT,
    bytes BLOB NOT NULL,
    PRIMARY KEY (item, place)
) WITHOUT ROWID;
";
