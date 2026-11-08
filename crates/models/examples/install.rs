//! Descarga los modelos que falten de los slots indicados, con el instalador de
//! la app. Uso: cargo run -p juanola-models --example install -- <raíz> <slot>...
use std::sync::atomic::AtomicBool;

use juanola_models::install::install;
use juanola_models::{Catalog, Machine, Platform};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::PathBuf::from(args.first().expect("falta la raíz"));
    let slots: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    let catalog = Catalog::parse(include_str!("../../../models.toml"))?;
    let machine = Machine { platform: Platform::current(), ram_gb: 16 };
    for planned in catalog.plan(&slots, &machine, &root)? {
        println!("{} → {} (instalado: {})", planned.slot, planned.dir.display(), planned.status().installed);
        let mut last = 0;
        install(&planned, &AtomicBool::new(false), |p| {
            if p.done == p.total || p.done - last > 512 * 1024 {
                last = p.done;
                println!("  {} {}/{}", p.file, p.done, p.total);
            }
        })
        .await?;
        println!("  instalado: {}", planned.status().installed);
    }
    Ok(())
}
