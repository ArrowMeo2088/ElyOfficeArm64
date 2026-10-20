/// Step `n` upgrades version `n`.
pub(super) const STEPS: &[&str] = &[V1];

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
