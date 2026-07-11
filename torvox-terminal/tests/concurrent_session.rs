//! Concurrent access tests using standard threads and synchronisation.
//! Tests that GhosttyTerminal operations are safe under concurrent
//! read/write access behind a Mutex.

use std::sync::{Arc, Mutex};
use torvox_terminal::ghostty_terminal::GhosttyTerminal;

#[test]
fn concurrent_vt_write_and_snapshot() {
    let terminal = Arc::new(Mutex::new(
        GhosttyTerminal::new(24, 80, 10_000).expect("terminal"),
    ));

    let t1 = Arc::clone(&terminal);
    let t2 = Arc::clone(&terminal);

    let producer = std::thread::spawn(move || {
        for _ in 0..100 {
            let data: Vec<u8> = (0..10).map(|i| b'a' + (i % 26)).collect();
            t1.lock().unwrap().vt_write(&data);
        }
    });

    let consumer = std::thread::spawn(move || {
        for _ in 0..50 {
            t2.lock().unwrap().take_snapshot();
        }
    });

    producer.join().expect("producer");
    consumer.join().expect("consumer");
}

#[test]
fn concurrent_resize_and_write() {
    let terminal = Arc::new(Mutex::new(
        GhosttyTerminal::new(24, 80, 10_000).expect("terminal"),
    ));

    let t1 = Arc::clone(&terminal);
    let t2 = Arc::clone(&terminal);

    let writer = std::thread::spawn(move || {
        for i in 0..50 {
            let line = format!("line {}\n", i);
            t1.lock().unwrap().vt_write(line.as_bytes());
        }
    });

    let resizer = std::thread::spawn(move || {
        for _ in 0..10 {
            t2.lock().unwrap().resize(40, 120);
        }
    });

    writer.join().expect("writer");
    resizer.join().expect("resizer");
}
