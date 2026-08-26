#[cfg(feature = "desktop-runtime")]
fn main() {
    tauri_build::build()
}

#[cfg(not(feature = "desktop-runtime"))]
fn main() {}
