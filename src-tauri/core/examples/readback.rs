//! Only reads the current keyboard colors from iCUE (changes nothing).
//!
//! `cargo run -p k65-color-core --example readback -- S CapsLock F1`

use std::sync::mpsc;
use std::time::Duration;
use k65_color_core::layout::Layout;
use k65_color_core::sdk::{self, Sdk, SdkEvent};

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    let sdk = Sdk::load_or_download(&sdk::default_candidates()).expect("load iCUE SDK");
    let (tx, rx) = mpsc::channel();
    sdk.connect(Box::new(move |ev| drop(tx.send(ev)))).expect("CorsairConnect");
    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(SdkEvent::Session { state: sdk::CSS_CONNECTED, .. }) => break,
            Ok(_) => continue,
            Err(_) => panic!("no connection to iCUE"),
        }
    }
    let kb = sdk.keyboards().expect("devices").into_iter().next().expect("no keyboard");
    let layout = Layout::from_positions(&sdk.led_positions(&kb.id).expect("LED positions"));
    for name in &names {
        let Some(i) = layout.index_of(name) else {
            println!("{name}: unknown");
            continue;
        };
        let c = sdk.get_colors(&kb.id, &[layout.leds[i].luid]).expect("read colors")[0];
        println!("{name}: #{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    }
    sdk.disconnect();
}
