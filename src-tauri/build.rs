//! Build script of the application.
//!
//! The framework embeds the application manifest that declares the common
//! controls version 6, which its window menu library needs on Windows. That
//! manifest belongs to the application, and it is embedded by the build
//! helper, so nothing has to be added here.

fn main() {
    tauri_build::build();
}
