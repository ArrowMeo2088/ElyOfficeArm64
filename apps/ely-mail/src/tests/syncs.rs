use ely_mail_engine::{Protocol, Store};
use gpui::TestAppContext;

use super::{Keys, account, closed_port, settle, window, window_with};

#[test]
fn an_imap_account_syncs_on_open_and_says_why_it_failed() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut ann = account("ann@example.com");
    ann.incoming.port = closed_port();
    let ann = store.add_account(&ann).unwrap();
    let pop = ely_mail_engine::Account {
        protocol: Protocol::Pop,
        ..account("bo@example.com")
    };
    let bo = store.add_account(&pop).unwrap();
    drop(store);
    let keys = Keys::default();
    keys.kept
        .lock()
        .unwrap()
        .insert("account:ann@example.com".into(), "pw".into());
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), keys);
    settle(cx, |app| {
        mail.read(app)
            .syncs
            .get(&ann)
            .is_some_and(|each| each.task.is_none() && each.synced.is_some())
    });
    cx.update(|_, app| {
        let mail = mail.read(app);
        let syncing = &mail.syncs[&ann];
        let failed = syncing.failed.as_deref().unwrap();
        assert!(failed.starts_with("cannot connect"), "{failed}");
        assert_eq!(syncing.synced.unwrap().at, None);
        assert!(!mail.syncs.contains_key(&bo), "POP3 is collected apart");
    });
}

#[test]
fn an_account_without_its_secret_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let ann = Store::open(dir.path())
        .unwrap()
        .add_account(&account("ann@example.com"))
        .unwrap();
    let mut cx = TestAppContext::single();
    let (mail, cx) = window(&mut cx, dir.path(), Keys::default());
    settle(cx, |app| {
        mail.read(app)
            .syncs
            .get(&ann)
            .is_some_and(|each| each.failed.is_some())
    });
    cx.update(|_, app| {
        let failed = mail.read(app).syncs[&ann].failed.clone().unwrap();
        assert_eq!(
            failed,
            "credential store failed: no saved secret for account:ann@example.com"
        );
    });
}

#[test]
fn a_missing_tls_is_the_sync_failure() {
    let dir = tempfile::tempdir().unwrap();
    let ann = Store::open(dir.path())
        .unwrap()
        .add_account(&account("ann@example.com"))
        .unwrap();
    let mut cx = TestAppContext::single();
    let tls = Err("platform trust store unavailable".to_owned());
    let (mail, cx) = window_with(&mut cx, dir.path(), Keys::default(), tls);
    settle(cx, |app| {
        mail.read(app)
            .syncs
            .get(&ann)
            .is_some_and(|each| each.synced.is_some())
    });
    cx.update(|_, app| {
        let syncing = &mail.read(app).syncs[&ann];
        assert_eq!(
            syncing.failed.as_deref(),
            Some("platform trust store unavailable")
        );
        assert!(syncing.task.is_none());
    });
}
