//! Verifies the safe free-function entry point rejects worker threads around
//! the complete native runtime lifetime.

use pliant_embedder::{Error, Event};

fn worker_result() -> Result<(), Error> {
    std::thread::spawn(|| pliant_embedder::with_engine(|_| ()))
        .join()
        .map_err(|_| Error::CallbackPanicked)?
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if worker_result() != Err(Error::WrongThread) {
        return Err("worker-thread with_engine was not rejected before startup".into());
    }

    let mut checked_during_runtime = false;
    pliant_embedder::run(|engine, event| {
        if matches!(event, Event::Ready) {
            checked_during_runtime = worker_result() == Err(Error::WrongThread);
            let _ = engine.shutdown();
        }
    })?;
    if !checked_during_runtime {
        return Err("worker-thread with_engine was not rejected during runtime".into());
    }
    if worker_result() != Err(Error::WrongThread) {
        return Err("worker-thread with_engine was not rejected after teardown".into());
    }

    println!("PASS: worker-thread with_engine rejected before, during, and after runtime");
    Ok(())
}
