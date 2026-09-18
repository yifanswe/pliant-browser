use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;

use pliant_browser::FixtureServer;

fn page(fixture: &FixtureServer, path: &str) -> String {
    let url = fixture.url(path);
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
    response.split_once("\r\n\r\n").unwrap().1.to_owned()
}

fn script(html: &str) -> &str {
    html.split_once("<script>")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0
}

fn run_node(script: &str, harness: &str) {
    let output = Command::new("node")
        .args(["-e", harness])
        .env("FIXTURE_SCRIPT", script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "node failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fixture_a_script_initializes_storage_controls_and_lifecycle_handlers() {
    let fixture = FixtureServer::start().unwrap();
    let html = page(&fixture, "/a");

    run_node(
        script(&html),
        r#"
const assert = require("node:assert/strict");
const vm = require("node:vm");
const listeners = {};
const documentListeners = {};
const beacons = [];
const fetched = [];
function element() {
  return {
    textContent: "",
    dataset: {},
    onclick: undefined,
    listeners: {},
    addEventListener(type, callback) { this.listeners[type] = callback; }
  };
}
const elements = new Map([
  ["lifecycle", element()],
  ["storage", element()],
  ["set-storage", element()],
  ["reset-events", element()]
]);
const values = new Map();
global.document = {
  cookie: "",
  visibilityState: "visible",
  getElementById(id) { return elements.get(id); },
  addEventListener(type, callback) { documentListeners[type] = callback; }
};
global.localStorage = {
  getItem(key) { return values.get(key) ?? null; },
  setItem(key, value) { values.set(key, value); }
};
global.addEventListener = (type, callback) => { listeners[type] = callback; };
Object.defineProperty(global, "navigator", {
  configurable: true,
  value: { sendBeacon(url) { beacons.push(url); return true; } }
});
global.fetch = async (url, options) => { fetched.push([url, options]); };

(async () => {
  vm.runInThisContext(process.env.FIXTURE_SCRIPT);
  const lifecycle = elements.get("lifecycle");
  const storage = elements.get("storage");
  const setStorage = elements.get("set-storage");
  const resetEvents = elements.get("reset-events");
  assert.equal(storage.textContent, "cookie=(empty); localStorage=(empty)");
  assert.equal(typeof setStorage.onclick, "function");
  assert.equal(typeof resetEvents.onclick, "function");
  assert.equal(typeof listeners.pagehide, "function");
  assert.equal(typeof documentListeners.visibilitychange, "function");
  setStorage.onclick();
  assert.equal(values.get("pliant-marker"), "main");
  assert.match(document.cookie, /pliant-marker=main/);
  assert.match(storage.textContent, /localStorage=main/);
  await resetEvents.onclick();
  assert.deepEqual(fetched, [["/reset-events", {method: "POST"}]]);
  assert.equal(lifecycle.textContent, "event log reset");
  listeners.pagehide();
  assert.equal(lifecycle.textContent, "pagehide");
  document.visibilityState = "hidden";
  documentListeners.visibilitychange();
  assert.equal(lifecycle.dataset.visibility, "hidden");
  assert.deepEqual(beacons, [
    "/event?kind=pagehide",
    "/event?kind=visibilitychange-hidden"
  ]);
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
"#,
    );
}

#[test]
fn fixture_confirm_script_cancels_only_dirty_close_and_allow_close_resets_it() {
    let fixture = FixtureServer::start().unwrap();
    let html = page(&fixture, "/confirm");

    run_node(
        script(&html),
        r#"
const assert = require("node:assert/strict");
const vm = require("node:vm");
const listeners = {};
function element() {
  return {
    textContent: "",
    onclick: undefined,
    listeners: {},
    addEventListener(type, callback) { this.listeners[type] = callback; }
  };
}
const elements = new Map([
  ["protected-draft", element()],
  ["attempts", element()],
  ["allow-close", element()]
]);
global.document = { getElementById(id) { return elements.get(id); } };
global.addEventListener = (type, callback) => { listeners[type] = callback; };

vm.runInThisContext(process.env.FIXTURE_SCRIPT);
const draftElement = elements.get("protected-draft");
const attemptsElement = elements.get("attempts");
const allowCloseElement = elements.get("allow-close");
assert.equal(typeof draftElement.listeners.input, "function");
assert.equal(typeof allowCloseElement.onclick, "function");
assert.equal(typeof listeners.beforeunload, "function");

const clean = {prevented: false, preventDefault() { this.prevented = true; }};
listeners.beforeunload(clean);
assert.equal(clean.prevented, false);

draftElement.listeners.input();
const dirtyEvent = {prevented: false, preventDefault() { this.prevented = true; }};
listeners.beforeunload(dirtyEvent);
assert.equal(dirtyEvent.prevented, true);
assert.equal(dirtyEvent.returnValue, "");
assert.equal(attemptsElement.textContent, "Dirty close was refused");

allowCloseElement.onclick();
assert.equal(attemptsElement.textContent, "Close is now allowed");
const allowed = {prevented: false, preventDefault() { this.prevented = true; }};
listeners.beforeunload(allowed);
assert.equal(allowed.prevented, false);
"#,
    );
}
