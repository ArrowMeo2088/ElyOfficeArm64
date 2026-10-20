mod fake;

use base64::{Engine, engine::general_purpose::STANDARD};
use ely_mail_engine::{Error, Secret, Security};
use fake::{Step, account, certs, expect, send, serve, smtp::logged_in};

const EHLO: &str = "250-mx\n250 AUTH PLAIN\n";

fn handed() -> Vec<Step> {
    vec![
        expect("MAIL FROM:<ann@example.com>"),
        send("250 ok\n"),
        expect("RCPT TO:<bo@example.com>"),
        send("250 ok\n"),
        expect("DATA"),
        send("354 go\n"),
        Step::Data("Subject: x\n\nx".into()),
    ]
}

#[tokio::test]
async fn a_lost_answer_after_the_message_is_unsure() {
    let (mut smtp, script) = logged_in(EHLO, handed()).await;
    let err = smtp
        .send(
            "ann@example.com",
            &["bo@example.com".to_owned()],
            b"Subject: x\r\n\r\nx\r\n",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Unsure(_)), "{err}");
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_refusal_after_the_message_is_a_failure() {
    let mut steps = handed();
    steps.extend([
        send("554 5.7.1 Message rejected\n"),
        expect("RSET"),
        send("250 ok\n"),
    ]);
    let (mut smtp, script) = logged_in(EHLO, steps).await;
    let err = smtp
        .send(
            "ann@example.com",
            &["bo@example.com".to_owned()],
            b"Subject: x\r\n\r\nx\r\n",
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Server(text) if text.contains("554") && text.contains("rejected")),
        "{err}"
    );
    script.await.unwrap().unwrap();
}

#[tokio::test]
async fn an_account_sends_to_every_envelope_address_and_ignores_a_failed_logout() {
    let certs = certs();
    let (server, script) = serve(
        &certs,
        Security::Tls,
        vec![
            send("220 mx ready\n"),
            expect("EHLO [127.0.0.1]"),
            send(EHLO),
            expect(&format!(
                "AUTH PLAIN {}",
                STANDARD.encode("\0ann@example.com\0pw")
            )),
            send("235 ok\n"),
            expect("MAIL FROM:<ann@example.com>"),
            send("250 ok\n"),
            expect("RCPT TO:<bo@example.com>"),
            send("250 ok\n"),
            expect("RCPT TO:<hidden@example.com>"),
            send("250 ok\n"),
            expect("DATA"),
            send("354 go\n"),
            Step::Data("Subject: x\n\nx".into()),
            send("250 queued\n"),
            expect("QUIT"),
        ],
    )
    .await;
    let mut sender = account("ann@example.com");
    sender.outgoing = server;
    sender
        .send(
            Secret::Password("pw".into()),
            &["bo@example.com".to_owned(), "hidden@example.com".to_owned()],
            b"Subject: x\r\n\r\nx\r\n",
            &certs.trusted,
        )
        .await
        .unwrap();
    script.await.unwrap().unwrap();
}
