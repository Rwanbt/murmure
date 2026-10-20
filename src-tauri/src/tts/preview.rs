//! Explicit CPU-only French voice preview using the standalone Kyutai binary.
//! No user text is placed in process arguments in this first integration.
//! Future arbitrary-text synthesis MUST use a different privacy-safe transport.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

const PREVIEW_TEXT: &str = "Bonjour ! Voici la synthèse vocale française locale de Murmure.";
const POLL_INTERVAL: Duration = Duration::from_millis(80);
const MAX_SPEECH_CHARS: usize = 1_000;
const MAX_GENERATION: Duration = Duration::from_secs(600);
// Derived from the v4.0.0 official Windows archive (SHA256 of archive verified
// before extraction in scripts/verify-pocket-tts-windows.ps1).
#[cfg(target_os = "windows")]
pub(super) const WINDOWS_SIDECAR_SHA256: &str = "9b52ad9d8721869c0adcb98e62a5e89e8fbfef0abdc397b4189db411456c6023";

// Serialize model-consent revocation against queuing new requests. The worker
// remains independent: never hold this lock while generating or playing audio.
pub(crate) static MODEL_APPROVAL_LOCK: Mutex<()> = Mutex::new(());

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

/// The official x86-64 binary requires x86-64-v3 (AVX2). Refuse to
/// launch an unsupported executable instead of crashing with SIGILL.
pub(super) fn cpu_compatible() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        true
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PocketTtsRuntimeStatus {
    installed: bool,
    verified: bool,
    cpu_compatible: bool,
}

#[tauri::command]
pub async fn pocket_tts_install(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        super::install_windows::install(app).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Verified Pocket TTS installation is currently supported only on Windows".into())
    }
}

#[tauri::command]
pub fn pocket_tts_status(app: AppHandle) -> PocketTtsRuntimeStatus {
    PocketTtsRuntimeStatus {
        installed: locate_sidecar(&app).is_ok_and(|path| path.is_file()),
        verified: binary_path(&app).is_ok(),
        cpu_compatible: cpu_compatible(),
    }
}

fn locate_sidecar(app: &AppHandle) -> Result<PathBuf, String> {
    // Prefer the app's dedicated user-local install directory (never PATH).
    #[cfg(target_os = "windows")]
    {
        let user_install = app.path().app_local_data_dir()
            .map_err(|_| "Unable to locate the application data directory".to_string())?
            .join("pocket-tts").join("bin").join("pocket-tts.exe");
        if user_install.exists() {
            return Ok(user_install);
        }
    }
    let _ = app;
    // Keep compatibility with previously deployed verified sidecars.
    let executable = if cfg!(windows) { "pocket-tts.exe" } else { "pocket-tts" };
    let application = std::env::current_exe()
        .map_err(|_| "Unable to locate Murmure executable".to_string())?;
    let dir = application.parent().ok_or("Unable to locate application directory")?;
    Ok(dir.join(executable))
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    // Bounded-memory streaming hash; a corrupt or unreadable sidecar fails closed.
    let mut file = File::open(path)
        .map_err(|_| "Unable to read Pocket TTS executable for verification".to_string())?;
    let mut hash = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        let bytes = file.read(&mut block)
            .map_err(|_| "Unable to hash Pocket TTS executable".to_string())?;
        if bytes == 0 {
            break;
        }
        hash.update(&block[..bytes]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn verify_sidecar(path: &Path, expected_sha256: &str) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "Pocket TTS executable is not installed".to_string())?;
    if !metadata.file_type().is_file() {
        return Err("Pocket TTS executable must be a regular file, not a link".to_string());
    }
    if !sha256_hex(path)?.eq_ignore_ascii_case(expected_sha256) {
        return Err("Pocket TTS executable does not match the verified Kyutai release".to_string());
    }
    Ok(())
}

fn binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    if !cpu_compatible() {
        return Err("This Pocket TTS binary requires AVX2 CPU support".to_string());
    }
    // Only the official Windows binary has a vetted digest. Do not silently
    // allow an arbitrary executable on macOS/Linux until releases are pinned.
    #[cfg(target_os = "windows")]
    {
        let path = locate_sidecar(app)?;
        verify_sidecar(&path, WINDOWS_SIDECAR_SHA256)?;
        Ok(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Verified Pocket TTS binary is not yet configured for this operating system".to_string())
    }
}

fn preview_args(output: &Path, model: &str) -> Vec<std::ffi::OsString> {
    use std::ffi::OsString;
    [
        OsString::from("generate"),
        OsString::from("--language"),
        OsString::from(model),
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

fn configure_model_source(command: &mut Command, cache_dir: &Path) {
    // Isolate Murmure's model cache from system-wide Hugging Face credentials.
    // Do not inherit a personal token or an alternative untrusted model host.
    command.env("HF_HOME", cache_dir)
        .env("HF_ENDPOINT", "https://huggingface.co")
        .env_remove("HF_TOKEN")
        .env_remove("HF_TOKEN_PATH")
        .env_remove("HUGGING_FACE_HUB_TOKEN")
        .env_remove("HUGGINGFACE_HUB_CACHE")
        .env_remove("HF_HUB_CACHE");
}

fn run_speech(app: &AppHandle, rx: &Receiver<Request>, text: &str) -> Result<(), String> {
    // A request may have been queued before consent was revoked. Check again
    // in the worker before starting the external model process.
    require_tts_approval(crate::settings::load_settings(app).tts_model_download_approved)?;
    let binary = binary_path(app)?;
    let wave = TempWave(std::env::temp_dir().join(format!(
        "murmure-pocket-preview-{}.wav",
        uuid::Uuid::new_v4()
    )));

    let settings = crate::settings::load_settings(app);
    let model = if settings.tts_model == "french_24l" {
        "french_24l"
    } else {
        "french"
    };
    // The first, user-approved invocation may download the public model.
    // Restrict the CLI to a dedicated app cache with no inherited HF login.
    let cache_dir = app.path().app_cache_dir()
        .map_err(|_| "Unable to locate Murmure's model cache".to_string())?
        .join("pocket-tts");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|_| "Unable to create the local TTS model cache".to_string())?;
    let mut command = Command::new(binary);
    configure_model_source(&mut command, &cache_dir);
    emit(app, "generating", None);
    let mut child = command
        .args(preview_args(&wave.0, model))
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
    player.set_volume(f32::from(settings.tts_volume.min(100)) / 100.0);
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

fn require_tts_approval(approved: bool) -> Result<(), String> {
    if approved {
        Ok(())
    } else {
        Err("Enable Pocket TTS model downloads explicitly in Extensions before reading".to_string())
    }
}

fn send(app: &AppHandle, action: Request) -> Result<(), String> {
    let is_preview = matches!(&action, Request::Preview | Request::Speak(_));
    // Gate selection and synthesis dispatch atomically with permission changes.
    let _approval_guard = if is_preview {
        Some(MODEL_APPROVAL_LOCK.lock()
            .map_err(|_| "TTS approval state is unavailable".to_string())?)
    } else {
        None
    };
    if is_preview {
        // This also covers reading selections via a global hotkey. Never
        // start a child process that could fetch model files without opt-in.
        let settings = crate::settings::load_settings(app);
        require_tts_approval(settings.tts_model_download_approved)?;
    }
    let state = app.state::<PocketPreviewState>();
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
    #[cfg(target_os = "windows")]
    super::selection_windows::invalidate_pending_selection();
    send(&app, Request::Stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_source_does_not_inherit_user_huggingface_credentials() {
        let mut command = Command::new("pocket-tts");
        configure_model_source(&mut command, Path::new("isolated-cache"));
        let entries: std::collections::HashMap<_, _> = command.get_envs()
            .map(|(name, value)| (name.to_string_lossy().into_owned(),
                                    value.map(|v| v.to_string_lossy().into_owned())))
            .collect();
        assert_eq!(entries.get("HF_TOKEN"), Some(&None));
        assert_eq!(entries.get("HF_TOKEN_PATH"), Some(&None));
        assert_eq!(entries.get("HUGGINGFACE_HUB_CACHE"), Some(&None));
        assert_eq!(entries.get("HF_ENDPOINT"),
                   Some(&Some("https://huggingface.co".to_string())));
        assert_eq!(entries.get("HF_HOME"),
                   Some(&Some("isolated-cache".to_string())));
    }

    #[test]
    fn model_download_opt_in_is_required_before_any_generation() {
        assert!(require_tts_approval(false).is_err());
        assert!(require_tts_approval(true).is_ok());
    }

    #[test]
    fn a_second_preview_is_rejected_until_the_first_finishes() {
        let busy = AtomicBool::new(false);
        assert!(acquire_preview(&busy));
        assert!(!acquire_preview(&busy));
        busy.store(false, Ordering::Release);
        assert!(acquire_preview(&busy));
    }

    #[test]
    fn verified_sidecar_rejects_tampering_and_untrusted_files() {
        let file = std::env::temp_dir()
            .join(format!("murmure-tts-integrity-{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&file, b"abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_sidecar(&file, expected).is_ok());
        std::fs::write(&file, b"abcd").unwrap();
        assert!(verify_sidecar(&file, expected).is_err());
        std::fs::remove_file(&file).unwrap();
        assert!(verify_sidecar(&file, expected).is_err());
    }

    #[test]
    fn runtime_status_is_consistent_with_binary_preflight() {
        // The status command requires an AppHandle so it can resolve the
        // user-local runtime path; binary integrity is covered separately.
        assert_eq!(status.cpu_compatible, cpu_compatible());
        if !status.cpu_compatible {
            assert!(!status.installed);
        }
    }

    #[test]
    fn speak_filters_obvious_secrets_and_requires_text() {
        assert_eq!(checked_text("Bonjour à tous.\npassword=nepaslire").unwrap(), "Bonjour à tous.");
        assert!(checked_text("password=secret").is_err());
        assert!(checked_text(&"é".repeat(MAX_SPEECH_CHARS + 1)).is_err());
    }

    #[test]
    fn preview_is_french_cpu_only_with_no_runtime_user_text() {
        let args = preview_args(Path::new("preview.wav"), "french");
        let parts: Vec<_> = args.iter().map(|a| a.to_string_lossy()).collect();
        assert!(parts.windows(2).any(|p| p == ["--language", "french"]));
        let large = preview_args(Path::new("preview.wav"), "french_24l");
        let pieces: Vec<_> = large.iter().map(|a| a.to_string_lossy()).collect();
        assert!(pieces.windows(2).any(|p| p == ["--language", "french_24l"]));
        assert!(parts.windows(2).any(|p| p == ["--device", "cpu"]));
        assert!(parts.windows(2).any(|p| p == ["--voice", "estelle"]));
        assert!(parts.windows(2).any(|p| p == ["--text", "-"]));
    }
}
