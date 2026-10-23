//! Input test (macOS): starts the event tap and sends a synthetic left Shift,
//! which the tap should report as `LeftShift`.
//!
//! `cargo run -p k65-color-core --example taptest`

use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use k65_color_core::input::{self, InputStatus};

fn main() {
    println!("Input Monitoring granted: {:?}", input::permission());
    let status = Arc::new(Mutex::new(InputStatus::Starting));
    let (tx, rx) = mpsc::channel();
    input::start(
        Arc::new(move |name| {
            let _ = tx.send(name);
        }),
        status.clone(),
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    while *status.lock().unwrap() != InputStatus::Running && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("Listener: {:?}", status.lock().unwrap());

    #[cfg(target_os = "macos")]
    {
        input::post_test_shift();
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(key) => println!("received: {key:?}"),
            Err(_) => println!("nothing received (posting may need the Accessibility permission)"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    drop(rx);
}
