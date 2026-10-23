//! Key diagnostics (macOS): shows what the keyboard really sends on a press,
//! including media and special keys.
//!
//! `cargo run -p k65-color-core --example keylog -- 120`

#[cfg(target_os = "macos")]
fn main() {
    use std::io::Write;
    use std::time::{Duration, Instant};

    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let start = Instant::now();
    println!("Listening for {secs} s …");
    k65_color_core::input::diagnose(
        Duration::from_secs(secs),
        Box::new(move |line| {
            println!("{:6.2}s  {line}", start.elapsed().as_secs_f32());
            let _ = std::io::stdout().flush();
        }),
    )
    .expect("diagnostics");
    println!("done");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Key diagnostics are only available on macOS.");
}
