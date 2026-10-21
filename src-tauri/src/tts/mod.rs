//! Local TTS components, independent from Murmure's microphone/STT engine.
// The official release publishes a build per desktop target, so the
// installer is not a Windows privilege. Only one of the two is compiled
// for a given target.
#[cfg(not(target_os = "windows"))]
mod install_posix;
#[cfg(target_os = "windows")]
mod install_windows;
pub mod models;
pub mod preview;
#[cfg(target_os = "windows")]
pub mod selection_windows;
pub mod text;
