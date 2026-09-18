use std::io::{Read, Write};
use std::net::TcpStream;

use pliant_browser::FixtureServer;

fn get(url: &str) -> String {
    let authority_and_path = url.strip_prefix("http://").unwrap();
    let (authority, path) = authority_and_path.split_once('/').unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    write!(
        stream,
        "GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn fixture_serves_distinct_form_and_lifecycle_pages() {
    let fixture = FixtureServer::start().unwrap();

    let a = get(&fixture.url("/a"));
    let b = get(&fixture.url("/b"));
    let confirm = get(&fixture.url("/confirm"));

    assert!(a.starts_with("HTTP/1.1 200 OK"));
    assert!(a.contains("<title>Pliant fixture A</title>"));
    assert!(a.contains("id=\"draft\""));
    assert!(a.contains("pliant-marker"));
    assert!(a.contains("pagehide"));
    assert!(a.contains("visibilitychange"));
    assert!(b.contains("<title>Pliant fixture B</title>"));
    assert!(!b.contains("id=\"draft\""));
    assert!(confirm.contains("beforeunload"));
    assert!(confirm.contains("Dirty close was refused"));
    assert_ne!(a, b);
}

#[test]
fn fixture_records_lifecycle_events_on_the_same_origin() {
    let fixture = FixtureServer::start().unwrap();

    let event = get(&fixture.url("/event?kind=pagehide"));
    let events = get(&fixture.url("/events"));

    assert!(event.starts_with("HTTP/1.1 204 No Content"));
    assert!(events.contains("<pre id=\"events\">pagehide</pre>"));
}
