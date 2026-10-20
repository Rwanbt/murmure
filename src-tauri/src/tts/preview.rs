//! Explicit CPU-only French voice preview using the standalone Kyutai binary.
//! No user text is placed in process arguments in this first integration.
//! Future arbitrary-text synthesis MUST use a different privacy-safe transport.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const PREVIEW_TEXT: &str = "Bonjour ! Voici la synthèse vocale française locale de Murmure.";
const POLL_INTERVAL: Duration = Duration::from_millis(80);
const MAX_SPEECH_CHARS: usize = 1_000;
const MAX_GENERATION: Duration = Duration::from_secs(600);

enum Request {
    Preview,
    Speak(String),
    Pause,
    Resume,
    Stop,
}

pub struct PocketPreviewState {
    tx: Sender<Request>,
    busy: Arc<AtomicBool>,
}

fn acquire_preview(busy: &AtomicBool) -> bool {
    busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewEvent {
    state: &'static str,
    detail: Option<String>,
}

fn emit(app: &AppHandle, state: &'static str, detail: Option<String>) {
    let _ = app.emit("pocket-tts-preview", PreviewEvent { state, detail });
}

struct TempWave(PathBuf);

impl Drop for TempWave {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn binary_path() -> Result<PathBuf, String> {
    // Require an explicitly installed sidecar beside the Murmure executable.
    // Never search PATH or invoke a shell when deciding what code to run.
    let binary = if cfg!(windows) {
        "pocket-tts.exe"
    } else {
        "pocket-tts"
    };
    let dir = std::env::current_exe()
        .map_err(|_| "Unable to locate Murmure executable".to_string())?
        .parent()
        .ok_or("Unable to locate application directory")?
        .to_path_buf();
    let path = dir.join(binary);
    if !path.is_file() {
        return Err("Pocket TTS sidecar is not installed next to Murmure".to_string());
    }
    Ok(path)
}

fn preview_args(output: &Path) -> Vec<std::ffi::OsString> {
    use std::ffi::OsString;
    [
        OsString::from("generate"),
        OsString::from("--language"),
        OsString::from("french"),
        OsString::from("--voice"),
        OsString::from("estelle"),
        OsString::from("--device"),
        OsString::from("cpu"),
        OsString::from("--text"),
        OsString::from("-"),
        OsString::from("--output-path"),
        output.as_os_str().to_os_string(),
    ]
    .into()
}

fn run_speech(app: &AppHandle, rx: &Receiver<Request>, text: &str) -> Result<(), String> {
    let binary = binary_path()?;
    let wave = TempWave(std::env::temp_dir().join(format!(
        "murmure-pocket-preview-{}.wav",
        uuid::Uuid::new_v4()
    )));

    emit(app, "generating", None);
    let mut child = Command::new(binary)
        .args(preview_args(&wave.0))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Unable to start Pocket TTS".to_string())?;
    // Kyutai's --text - reads stdin. Never put document text in argv, where
    // other processes can inspect it through process listings.
    let write_result = child.stdin.take()
        .ok_or_else(|| "Pocket TTS stdin was unavailable".to_string())
        .and_then(|mut stdin| stdin.write_all(text.as_bytes())
            .map_err(|_| "Unable to send text to Pocket TTS".to_string()));
    if let Err(reason) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(reason);
    }
    let started = Instant::now();

    loop {
        if started.elapsed() >= MAX_GENERATION {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Pocket TTS preview timed out; generation was stopped".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err("Pocket TTS generation failed".into());
            }
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Unable to monitor Pocket TTS".into());
            }
        }
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(Request::Stop) => {
                let _ = child.kill();
                let _ = child.wait();
                emit(app, "stopped", None);
                return Ok(());
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
        }
    }

    let file = File::open(&wave.0).map_err(|_| "Pocket TTS produced no WAV".to_string())?;
    let source =
        rodio::Decoder::new(file).map_err(|_| "Pocket TTS WAV could not be decoded".to_string())?;
    let builder = rodio::DeviceSinkBuilder::from_default_device()
        .map_err(|_| "No usable audio output device".to_string())?;
    let output = builder
        .open_sink_or_fallback()
        .map_err(|_| "Unable to open output device".to_string())?;
    let player = rodio::Player::connect_new(output.mixer());
    player.append(source);
    emit(app, "playing", None);

    while !player.empty() {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(Request::Pause) => {
                player.pause();
                emit(app, "paused", None);
            }
            Ok(Request::Resume) => {
                player.play();
                emit(app, "playing", None);
            }
            Ok(Request::Stop) => {
                player.stop();
                emit(app, "stopped", None);
                return Ok(());
            }
            Ok(Request::Preview) | Ok(Request::Speak(_)) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                player.stop();
                return Ok(());
            }
        }
    }
    emit(app, "idle", None);
    Ok(())
}

pub fn init(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    let busy = Arc::new(AtomicBool::new(false));
    app.manage(PocketPreviewState {
        tx,
        busy: Arc::clone(&busy),
    });
    let handle = app.clone();
    thread::spawn(move || {
        while let Ok(request) = rx.recv() {
            let speech = match request {
                Request::Preview => Some(PREVIEW_TEXT.to_string()),
                Request::Speak(text) => Some(text),
                _ => None,
            };
            if let Some(text) = speech {
                if let Err(detail) = run_speech(&handle, &rx, &text) {
                    emit(&handle, "error", Some(detail));
                }
                busy.store(false, Ordering::Release);
            }
        }
    });
}

fn send(app: &AppHandle, action: Request) -> Result<(), String> {
    let state = app.state::<PocketPreviewState>();
    let is_preview = matches!(&action, Request::Preview | Request::Speak(_));
    if is_preview && !acquire_preview(&state.busy) {
        return Err("Pocket TTS is already active".to_string());
    }
    // Late or duplicate transport controls at idle must not wait in the
    // worker queue and unexpectedly stop the user's next preview.
    if !is_preview && !state.busy.load(Ordering::Acquire) {
        return Ok(());
    }
    if state.tx.send(action).is_err() {
        if is_preview {
            state.busy.store(false, Ordering::Release);
        }
        return Err("Pocket TTS preview worker is unavailable".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn pocket_tts_preview(app: AppHandle) -> Result<(), String> {
    send(&app, Request::Preview)
}

fn checked_text(input: &str) -> Result<String, String> {
    if input.chars().count() > MAX_SPEECH_CHARS {
        return Err("Text exceeds the Pocket TTS length limit".to_string());
    }
    let segments = super::text::prepare_speech(input)
        .map_err(|_| "No readable text was provided".to_string())?;
    Ok(segments.join(" "))
}

#[tauri::command]
pub fn pocket_tts_speak(app: AppHandle, text: String) -> Result<(), String> {
    let text = checked_text(&text)?;
    send(&app, Request::Speak(text))
}

#[tauri::command]
pub fn pocket_tts_pause(app: AppHandle) -> Result<(), String> {
    send(&app, Request::Pause)
}

#[tauri::command]
pub fn pocket_tts_resume(app: AppHandle) -> Result<(), String> {
    send(&app, Request::Resume)
}

#[tauri::command]
pub fn pocket_tts_stop(app: AppHandle) -> Result<(), String> {
    send(&app, Request::Stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_preview_is_rejected_until_the_first_finishes() {
        let busy = AtomicBool::new(false);
        assert!(acquire_preview(&busy));
        assert!(!acquire_preview(&busy));
        busy.store(false, Ordering::Release);
        assert!(acquire_preview(&busy));
    }

    #[test]
    fn speak_filters_obvious_secrets_and_requires_text() {
        assert_eq!(checked_text("Bonjour à tous.\npassword=nepaslire").unwrap(), "Bonjour à tous.");
        assert!(checked_text("password=secret").is_err());
        assert!(checked_text(&"é".repeat(MAX_SPEECH_CHARS + 1)).is_err());
    }

    #[test]
    fn preview_is_french_cpu_only_with_no_runtime_user_text() {
        let args = preview_args(Path::new("preview.wav"));
        let parts: Vec<_> = args.iter().map(|a| a.to_string_lossy()).collect();
        assert!(parts.windows(2).any(|p| p == ["--language", "french"]));
        assert!(parts.windows(2).any(|p| p == ["--device", "cpu"]));
        assert!(parts.windows(2).any(|p| p == ["--voice", "estelle"]));
        assert!(parts.windows(2).any(|p| p == ["--text", "-"]));
    }
}
