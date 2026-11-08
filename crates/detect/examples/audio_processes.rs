//! Diagnóstico: dispositivos de audio y procesos cliente de Core Audio.
//! Uso: `cargo run -p juanola-detect --example audio_processes`.
fn main() {
    println!("dispositivos:");
    for d in juanola_detect::describe_devices() {
        println!("  {d}");
    }
    println!("micrófono predeterminado en uso: {}", juanola_detect::default_input_running());
    println!("procesos en marcha o con entrada:");
    for p in juanola_detect::audio_processes() {
        if p.running || p.input {
            let (ins, outs) = juanola_detect::process_devices(p.pid);
            println!("  {:>6} run={} in={} out={} in_devs={ins:?} out_devs={outs:?} {}", p.pid, p.running as u8, p.input as u8, p.output as u8, p.bundle_id);
        }
    }
    let users = juanola_detect::mic_users(std::process::id() as i32);
    println!("mic_users: {:?}", users.iter().map(|u| &u.bundle_id).collect::<Vec<_>>());
}
