use super::client::{HelperError, Outcome, ask, ask_gpg};
use super::prompt::PromptKind;
use super::protocol::GpgAsk;
use super::server::{Event, MAX_CONNECTIONS, REQUEST_TIMEOUT, Server, start_with};
use super::{Reply, Secret};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(20);

fn server(timeout: Duration) -> (Arc<Server>, Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let s = start_with(timeout, move |e| {
        let _ = tx.lock().unwrap().send(e);
    })
    .unwrap();
    (s, rx)
}

/// Asks `prompt` on another thread, returning the client's result through a channel.
fn ask_async(s: &Server, token: &str, prompt: &str) -> Receiver<Result<Outcome, HelperError>> {
    let (tx, rx) = mpsc::channel();
    let (addr, token, prompt) = (s.addr(), token.to_owned(), prompt.to_owned());
    std::thread::spawn(move || {
        let _ = tx.send(ask(&addr, &token, &prompt, None, WAIT));
    });
    rx
}

fn asked(events: &Receiver<Event>) -> (u64, super::Prompt) {
    match events.recv_timeout(WAIT).expect("an event") {
        Event::Asked { id, prompt } => (id, prompt),
        other => panic!("expected a prompt, got {other:?}"),
    }
}

#[test]
fn answers_reach_the_client() {
    let (s, events) = server(Duration::from_secs(60));
    assert!(s.addr().starts_with("127.0.0.1:"));
    let result = ask_async(&s, s.token(), "Password for 'https://ada@github.com': ");
    let (id, prompt) = asked(&events);
    assert_eq!(prompt.kind, PromptKind::Password { target: "https://ada@github.com".into(), user: Some("ada".into()) });
    assert!(s.answer(id, Reply::Answer(Secret::new("hunter2"))));
    match result.recv_timeout(WAIT).unwrap() {
        Ok(Outcome::Answer(a)) => assert_eq!(a.expose(), "hunter2"),
        other => panic!("expected an answer, got {other:?}"),
    }
    assert!(!s.answer(id, Reply::Cancel), "answered once");
}

#[test]
fn signing_requests_become_gpg_prompts() {
    let (s, events) = server(Duration::from_secs(60));
    let ask = GpgAsk { key: "ABCDEF0123456789".into(), user: Some("Ada <ada@example.com>".into()), retry: true };
    for (reply, want_secret) in [(Reply::UseSystem, None), (Reply::Answer(Secret::new("right")), Some("right"))] {
        let (tx, result) = mpsc::channel();
        let (addr, token, a) = (s.addr(), s.token().to_owned(), ask.clone());
        std::thread::spawn(move || {
            let _ = tx.send(ask_gpg(&addr, &token, &a, WAIT));
        });
        let (id, prompt) = asked(&events);
        let kind = PromptKind::GpgPassphrase { key: "ABCDEF0123456789".into(), user: Some("Ada <ada@example.com>".into()), retry: true };
        assert_eq!(prompt.kind, kind);
        assert_eq!(super::host(&prompt.kind), None);
        assert!(s.answer(id, reply));
        match (result.recv_timeout(WAIT).unwrap(), want_secret) {
            (Ok(Outcome::UseSystem), None) => {}
            (Ok(Outcome::Answer(a)), Some(want)) => assert_eq!(a.expose(), want),
            (other, _) => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn cancel_and_timeout() {
    let (s, events) = server(Duration::from_secs(60));
    let result = ask_async(&s, s.token(), "Username for 'https://github.com': ");
    let (id, _) = asked(&events);
    assert!(s.answer(id, Reply::Cancel));
    assert!(matches!(result.recv_timeout(WAIT).unwrap(), Ok(Outcome::Cancelled)));

    let (s, events) = server(Duration::from_millis(200));
    let result = ask_async(&s, s.token(), "Username for 'https://github.com': ");
    let (id, _) = asked(&events);
    assert!(matches!(result.recv_timeout(WAIT).unwrap(), Ok(Outcome::TimedOut)));
    assert_eq!(events.recv_timeout(WAIT).unwrap(), Event::Expired { id });
    assert!(!s.answer(id, Reply::Cancel), "expired");
}

#[test]
fn strangers_get_nothing() {
    let (s, events) = server(Duration::from_secs(60));
    let wrong = "0".repeat(32);
    let result = ask_async(&s, &wrong, "Username for 'https://github.com': ");
    assert!(matches!(result.recv_timeout(WAIT).unwrap(), Err(HelperError::Closed | HelperError::Protocol(_))));

    // Garbage is closed without a reply.
    let mut raw = TcpStream::connect(s.addr()).unwrap();
    raw.set_read_timeout(Some(WAIT)).unwrap();
    raw.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    let mut buf = Vec::new();
    let _ = raw.read_to_end(&mut buf);
    assert!(buf.is_empty());
    assert!(events.recv_timeout(Duration::from_millis(300)).is_err(), "no event for either");

    // Nothing listening.
    let addr = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().to_string()
    };
    assert!(matches!(ask(&addr, "t", "p", None, WAIT), Err(HelperError::Unreachable(_))));
}

#[test]
fn connections_beyond_the_cap_are_closed() {
    let (s, events) = server(Duration::from_secs(60));
    // Idle connections hold every slot until they send a request or time out.
    let idle: Vec<TcpStream> = (0..MAX_CONNECTIONS).map(|_| TcpStream::connect(s.addr()).unwrap()).collect();
    let mut extra = TcpStream::connect(s.addr()).unwrap();
    extra.set_read_timeout(Some(WAIT)).unwrap();
    let started = std::time::Instant::now();
    let mut buf = Vec::new();
    let _ = extra.read_to_end(&mut buf);
    assert!(buf.is_empty());
    assert!(started.elapsed() < REQUEST_TIMEOUT / 2, "closed at once, not after the request timeout");

    // Once the idle ones go away, prompts work again.
    drop(idle);
    let mut answered = false;
    for _ in 0..100 {
        let _result = ask_async(&s, s.token(), "Username for 'https://github.com': ");
        if let Ok(Event::Asked { id, .. }) = events.recv_timeout(Duration::from_millis(100)) {
            assert!(s.answer(id, Reply::Cancel));
            answered = true;
            break;
        }
    }
    assert!(answered, "slots were freed");
}
