//! Hardware test: colors the real keyboard via the light logic and reads the
//! colors back via iCUE. Takes a few seconds; afterwards iCUE takes over again.
//!
//! `cargo run -p k65-color-core --example hwtest`

use std::sync::mpsc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use k65_color_core::config::{parse_hex, Ambient, AmbientMode, Config};
use k65_color_core::engine::Engine;
use k65_color_core::layout::Layout;
use k65_color_core::sdk::{self, Sdk, SdkEvent};

fn main() {
    let sdk = Sdk::load_or_download(&sdk::default_candidates()).expect("load iCUE SDK");
    let (tx, rx) = mpsc::channel();
    sdk.connect(Box::new(move |ev| drop(tx.send(ev)))).expect("CorsairConnect");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(SdkEvent::Session { state: sdk::CSS_CONNECTED, icue_version }) => {
                println!("connected to iCUE {}", icue_version.unwrap_or_default());
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("no connection to iCUE"),
        }
    }
    sdk.set_layer_priority(200).expect("layer priority");

    let kb = sdk.keyboards().expect("devices").into_iter().next().expect("no keyboard found");
    let layout = Layout::from_positions(&sdk.led_positions(&kb.id).expect("LED positions"));
    println!("Keyboard: {} ({} LEDs)", kb.model, layout.leds.len());
    let luids: Vec<u32> = layout.leds.iter().map(|l| l.luid).collect();

    // Without background effect, so the colors read back are exactly comparable.
    let cfg = Config { ambient: Ambient { mode: AmbientMode::Off, ..Ambient::default() }, ..Config::default() };
    let [base, primary, secondary] =
        [&cfg.palette.base, &cfg.palette.primary, &cfg.palette.secondary].map(|h| parse_hex(h));
    let mut engine = Engine::new(layout.clone(), cfg, Instant::now());

    let push = |engine: &Engine| {
        let colors = engine.colors(Instant::now());
        let leds: Vec<(u32, [u8; 3])> = luids.iter().copied().zip(colors).collect();
        sdk.set_colors(&kb.id, &leds, 255).expect("send colors");
    };
    let settle = |engine: &Engine| {
        while engine.animating(Instant::now()) {
            push(engine);
            sleep(Duration::from_millis(16));
        }
        push(engine);
        sleep(Duration::from_millis(150));
    };
    let read = |name: &str| -> [u8; 3] {
        let luid = layout.leds[layout.index_of(name).expect(name)].luid;
        sdk.get_colors(&kb.id, &[luid]).expect("read colors")[0]
    };
    let mut failures = 0;
    let mut expect = |step: &str, name: &str, want: [u8; 3]| {
        let got = read(name);
        let ok = got.iter().zip(want).all(|(g, w)| (*g as i32 - w as i32).abs() <= 3);
        println!("  [{}] {step}: {name} = {got:?} (expected {want:?})", if ok { "ok" } else { "ERROR" });
        failures += (!ok) as u32;
    };

    println!("1. Base color");
    settle(&engine);
    expect("base", "S", base);
    expect("base", "F1", base);

    println!("2. Press D → ring primary");
    engine.press("D", Instant::now());
    settle(&engine);
    expect("ring", "S", primary);
    expect("ring", "R", primary);
    expect("pressed key", "D", base);
    expect("other zone", "3", base);
    sleep(Duration::from_millis(1200));

    println!("3. Press F (already colored) → F base, ring secondary");
    engine.press("F", Instant::now());
    settle(&engine);
    expect("back to base", "F", base);
    expect("ring", "G", secondary);
    expect("ring", "D", secondary);
    expect("old ring stays", "S", primary);
    sleep(Duration::from_millis(1200));

    println!("4. Space → letters reset");
    engine.press("Space", Instant::now());
    settle(&engine);
    expect("reset", "G", base);
    expect("reset", "S", base);

    println!("5. Transparent layer (pause) → iCUE shows through");
    let clear: Vec<(u32, [u8; 3])> = luids.iter().map(|&l| (l, [0, 0, 0])).collect();
    sdk.set_colors(&kb.id, &clear, 0).expect("transparent");
    sleep(Duration::from_millis(200));
    println!("  S now shows iCUE color {:?}", read("S"));

    sdk.disconnect();
    println!("disconnected, iCUE takes over again. Errors: {failures}");
    std::process::exit(failures.min(1) as i32);
}
