//! Muestra cada segundo qué procesos tienen el micrófono abierto y los eventos
//! del `MicWatcher`. Uso: `cargo run -p juanola-detect --example mic_users`.

use std::time::{Duration, Instant};

use juanola_detect::{classify, mic_users, MicWatcher, WatcherConfig};

fn main() {
    let t0 = Instant::now();
    let mut watcher = MicWatcher::new(WatcherConfig::default());
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(30);
    while t0.elapsed() < Duration::from_secs(secs) {
        let users = mic_users(std::process::id() as i32);
        let now = t0.elapsed();
        let list: Vec<String> = users
            .iter()
            .map(|u| format!("{} ({}) → {:?}", u.bundle_id, u.pid, classify(&u.bundle_id).map(|a| a.name)))
            .collect();
        println!("{:>5.1}s  {}", now.as_secs_f32(), if list.is_empty() { "—".into() } else { list.join(", ") });
        for ev in watcher.update(now, &users) {
            println!("        evento: {ev:?}");
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
