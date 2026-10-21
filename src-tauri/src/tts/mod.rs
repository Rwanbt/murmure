//! Local TTS components, independent from Murmure's microphone/STT engine.
#[cfg(target_os = "windows")]
mod install_windows;
pub mod preview;
#[cfg(target_os = "windows")]
pub mod selection_windows;
pub mod text;
