//! Local TTS components, independent from Murmure's microphone/STT engine.
pub mod text;
#[cfg(target_os = "windows")]
pub mod selection_windows;
pub mod preview;
