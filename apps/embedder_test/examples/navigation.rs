//! Runs a real local HTTP navigation/history/close smoke flow, no live accounts.

use pliant_embedder::{Engine, Error, Event, PageId};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut request = [0; 4096];
            if stream.read(&mut request).is_err() {
                continue;
            }
            let body =
                "<!doctype html><title>Pliant navigation fixture</title><p>Local fixture</p>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let (done_tx, done_rx) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("Navigation flow timed out; no success claimed.");
            std::process::exit(1);
        }
    });

    let a = format!("http://{address}/a");
    let b = format!("http://{address}/b");
    let mut page: Option<PageId> = None;
    let mut step = 0;
    let mut failure: Option<String> = None;
    let result = pliant_embedder::run(|engine, event| {
        if failure.is_some() {
            return;
        }
        let action = drive(engine, event, &a, &b, &mut page, &mut step);
        if let Err(error) = action {
            failure = Some(error);
            let _ = engine.shutdown();
        }
    });
    let _ = done_tx.send(());
    let _ = watchdog.join();
    result?;
    if let Some(error) = failure {
        return Err(error.into());
    }
    if step != 7 {
        return Err(format!("Engine stopped before completion (step {step})").into());
    }
    println!(
        "PASS: load and paint A -> load and paint B -> back -> forward -> close; all commands reject the closed page"
    );
    Ok(())
}

fn drive(
    engine: &Engine,
    event: Event,
    a: &str,
    b: &str,
    page: &mut Option<PageId>,
    step: &mut u8,
) -> Result<(), String> {
    match event {
        Event::Ready => {
            *page = Some(engine.create_page(a).map_err(|e| e.to_string())?);
        }
        Event::Navigated {
            page: id,
            url,
            can_go_back,
            can_go_forward,
        } => {
            if Some(id) != *page {
                return Err("Event targeted the wrong page".into());
            }
            // The initial blank document may commit before the requested URL.
            if *step == 0 && url == "about:blank" {
                return Ok(());
            }
            let expected = if *step == 0 || *step == 4 { a } else { b };
            if url != expected {
                return Err(format!("Expected {expected}, got {url}"));
            }
            println!("step {step}: {url}");
            let result = match *step {
                0 if !can_go_back && !can_go_forward => {
                    if engine.back(id) != Err(Error::NoHistoryEntry)
                        || engine.forward(id) != Err(Error::NoHistoryEntry)
                    {
                        return Err("Empty history did not reject traversal".into());
                    }
                    Ok(()) // Wait for a non-empty paint before leaving A.
                }
                2 if can_go_back && !can_go_forward => {
                    if engine.forward(id) != Err(Error::NoHistoryEntry) {
                        return Err("Forward traversal exceeded the history boundary".into());
                    }
                    Ok(()) // Wait for B's paint.
                }
                4 if !can_go_back && can_go_forward => engine.forward(id),
                5 if can_go_back && !can_go_forward => engine.close_page(id),
                _ => return Err("Unexpected navigation/history state".into()),
            };
            result.map_err(|e| e.to_string())?;
            *step += 1;
        }
        Event::Painted { page: id, url } if Some(id) == *page => {
            if *step == 1 && url == a {
                println!("Native window painted {url}");
                engine.load_url(id, b).map_err(|e| e.to_string())?;
                *step = 2;
            } else if *step == 3 && url == b {
                println!("Native window painted {url}");
                engine.back(id).map_err(|e| e.to_string())?;
                *step = 4;
            }
        }
        Event::Closed { page: id } if Some(id) == *page && *step == 6 => {
            for (command, result) in [
                ("load_url", engine.load_url(id, a)),
                ("back", engine.back(id)),
                ("forward", engine.forward(id)),
                ("close_page", engine.close_page(id)),
            ] {
                if result != Err(Error::InvalidPage) {
                    return Err(format!(
                        "{command} did not reject the closed page: {result:?}"
                    ));
                }
            }
            *step = 7;
            engine.shutdown().map_err(|e| e.to_string())?;
        }
        other => return Err(format!("Unexpected event: {other:?}")),
    }
    Ok(())
}
