use std::{env, fs, path::PathBuf};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let ready_path = env::var_os("FASTI_TAURI_BENCHMARK_READY_FILE")
                .map(PathBuf::from)
                .ok_or("FASTI_TAURI_BENCHMARK_READY_FILE is required")?;
            fs::write(ready_path, b"ready\n")?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("empty Tauri benchmark shell failed");
}
