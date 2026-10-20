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
pub(super) const WINDOWS_SIDECAR_SHA256: &str =
    "9b52ad9d8721869c0adcb98e62a5e89e8fbfef0abdc397b4189db411456c6023";

// Serialize model-consent revocation against queuing new requests. The worker
// remains independent: never hold this lock while generating or playing audio.
pub(crate) static MODEL_APPROVAL_LOCK: Mutex<()> = Mutex::new(());

/// Model currently being synthesized, so its state can be reported.
static ACTIVE_MODEL: Mutex<Option<String>> = Mutex::new(None);

/// Clears the active model when the generation ends, on every path.
struct ActiveModelGuard;

impl Drop for ActiveModelGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_MODEL.lock() {
            *active = None;
        }
    }
}

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
pub fn pocket_tts_cancel_install() -> bool {
    #[cfg(target_os = "windows")]
    {
        super::install_windows::cancel_install()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
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

/// Root of everything Murmure owns for Pocket TTS under the app cache.
///
/// On Windows the Tauri cache root and the local data root are the same
/// directory, so the model cache must live in its own child directory:
/// clearing the models must never destroy the installed runtime, which is
/// kept in the sibling `bin` directory.
fn pocket_tts_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|_| "Unable to locate Murmure's model cache".to_string())
        .map(|root| root.join("pocket-tts"))
}

/// The Hugging Face model cache owned by Murmure, never the shared user cache.
fn pocket_tts_model_cache(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(pocket_tts_root(app)?.join("models"))
}

fn locate_sidecar(app: &AppHandle) -> Result<PathBuf, String> {
    // Prefer the app's dedicated user-local install directory (never PATH).
    #[cfg(target_os = "windows")]
    {
        let user_install = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "Unable to locate the application data directory".to_string())?
            .join("pocket-tts")
            .join("bin")
            .join("pocket-tts.exe");
        if user_install.exists() {
            return Ok(user_install);
        }
    }
    let _ = app;
    // Keep compatibility with previously deployed verified sidecars.
    let executable = if cfg!(windows) {
        "pocket-tts.exe"
    } else {
        "pocket-tts"
    };
    let application =
        std::env::current_exe().map_err(|_| "Unable to locate Murmure executable".to_string())?;
    let dir = application
        .parent()
        .ok_or("Unable to locate application directory")?;
    Ok(dir.join(executable))
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    // Bounded-memory streaming hash; a corrupt or unreadable sidecar fails closed.
    let mut file = File::open(path)
        .map_err(|_| "Unable to read Pocket TTS executable for verification".to_string())?;
    let mut hash = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        let bytes = file
            .read(&mut block)
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
        let _ = app;
        Err(
            "Verified Pocket TTS binary is not yet configured for this operating system"
                .to_string(),
        )
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
    command
        .env("HF_HOME", cache_dir)
        .env("HF_ENDPOINT", "https://huggingface.co")
        .env_remove("HF_TOKEN")
        .env_remove("HF_TOKEN_PATH")
        .env_remove("HUGGING_FACE_HUB_TOKEN")
        .env_remove("HUGGINGFACE_HUB_CACHE")
        .env_remove("HF_HUB_CACHE");
}

/// How a supervised generation ended.
enum GenerationOutcome {
    /// The Pocket TTS process finished successfully.
    Completed,
    /// The user explicitly stopped the generation.
    Stopped,
    /// The worker channel closed, so the request was abandoned.
    Cancelled,
}

/// Apply exactly one transport action to the live playback engine.
/// Keeping it separate from Tauri events allows deterministic unit tests with
/// Rodio's in-memory queue, without assuming that CI has an audio device.
#[derive(Debug, PartialEq, Eq)]
enum PlayerTransition {
    Paused,
    Playing,
    Stopped,
    Noop,
}

fn apply_player_request(player: &rodio::Player, request: &Request) -> PlayerTransition {
    match request {
        Request::Pause => {
            player.pause();
            PlayerTransition::Paused
        }
        Request::Resume => {
            player.play();
            PlayerTransition::Playing
        }
        Request::Stop => {
            player.stop();
            PlayerTransition::Stopped
        }
        Request::Preview | Request::Speak(_) => PlayerTransition::Noop,
    }
}

/// Run the Pocket TTS binary until it finishes, is stopped or reaches the
/// generation bound, and report how it ended.
///
/// The command is fully supplied by the caller so the supervision can be
/// exercised without the real runtime.
fn supervise_generation(
    binary: &Path,
    args: &[std::ffi::OsString],
    text: &str,
    cache_dir: &Path,
    rx: &Receiver<Request>,
    timeout: Duration,
    mut on_state: impl FnMut(&'static str),
) -> Result<GenerationOutcome, String> {
    let mut command = Command::new(binary);
    configure_model_source(&mut command, cache_dir);
    on_state("generating");
    let mut child = command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Unable to start Pocket TTS".to_string())?;
    // Kyutai's --text - reads stdin. Never put document text in argv, where
    // other processes can inspect it through process listings.
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "Pocket TTS stdin was unavailable".to_string())
        .and_then(|mut stdin| {
            stdin
                .write_all(text.as_bytes())
                .map_err(|_| "Unable to send text to Pocket TTS".to_string())
        });
    if let Err(reason) = write_result {
        // The runtime may have exited before reading the text, for example on
        // an unusable CPU or a broken model. Report the generation failure
        // instead of a misleading transport error.
        let exited = child.try_wait().ok().flatten();
        let _ = child.kill();
        let _ = child.wait();
        if exited.is_some_and(|status| !status.success()) {
            return Err("Pocket TTS generation failed".into());
        }
        return Err(reason);
    }
    let started = Instant::now();

    loop {
        if started.elapsed() >= timeout {
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
                on_state("stopped");
                return Ok(GenerationOutcome::Stopped);
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(GenerationOutcome::Cancelled);
            }
        }
    }
    Ok(GenerationOutcome::Completed)
}

/// Prove that the generated file is playable audio before it reaches the
/// audio device, so a missing or corrupted output is reported instead of
/// being silently swallowed by the decoder.
fn verify_generated_wave(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|_| "Pocket TTS produced no WAV".to_string())?;
    rodio::Decoder::new(file).map_err(|_| "Pocket TTS WAV could not be decoded".to_string())?;
    Ok(())
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
    if let Ok(mut active) = ACTIVE_MODEL.lock() {
        *active = Some(model.to_string());
    }
    let _active_guard = ActiveModelGuard;
    // The first, user-approved invocation may download the public model.
    // Restrict the CLI to a dedicated app cache with no inherited HF login.
    let cache_dir = pocket_tts_model_cache(app)?;
    std::fs::create_dir_all(&cache_dir)
        .map_err(|_| "Unable to create the local TTS model cache".to_string())?;
    match supervise_generation(
        &binary,
        &preview_args(&wave.0, model),
        text,
        &cache_dir,
        rx,
        MAX_GENERATION,
        |state| emit(app, state, None),
    )? {
        GenerationOutcome::Completed => {}
        GenerationOutcome::Stopped | GenerationOutcome::Cancelled => return Ok(()),
    }

    verify_generated_wave(&wave.0)?;
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
            Ok(action) => match apply_player_request(&player, &action) {
                PlayerTransition::Paused => emit(app, "paused", None),
                PlayerTransition::Playing => emit(app, "playing", None),
                PlayerTransition::Stopped => {
                    emit(app, "stopped", None);
                    return Ok(());
                }
                PlayerTransition::Noop => {}
            },
            Err(RecvTimeoutError::Timeout) => {}
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
        Some(
            MODEL_APPROVAL_LOCK
                .lock()
                .map_err(|_| "TTS approval state is unavailable".to_string())?,
        )
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

/// Describe the models Murmure supports and their local state.
///
/// The catalog is exact and the local state is only reported when it can be
/// established: a model is never presented as ready when its presence in
/// the cache is ambiguous.
#[tauri::command]
pub fn pocket_tts_models(
    app: AppHandle,
) -> Result<Vec<super::models::PocketTtsModelStatus>, String> {
    let cache = pocket_tts_model_cache(&app)?;
    let in_use = ACTIVE_MODEL.lock().ok().and_then(|active| active.clone());
    Ok(super::models::model_statuses(&cache, in_use.as_deref()))
}

/// Remove only Murmure's isolated Pocket TTS model cache, never the user's
/// shared HF cache and never the installed runtime.
#[tauri::command]
pub fn pocket_tts_clear_model_cache(app: AppHandle) -> Result<bool, String> {
    let _guard = MODEL_APPROVAL_LOCK
        .lock()
        .map_err(|_| "TTS model cache state is unavailable".to_string())?;
    if app
        .state::<PocketPreviewState>()
        .busy
        .load(Ordering::Acquire)
    {
        return Err("Stop Pocket TTS playback before clearing its model cache".into());
    }
    // A running installation publishes the verified runtime into the same
    // Pocket TTS root, so it must never overlap with a cleanup.
    if install_active() {
        return Err(
            "Wait for the Pocket TTS installation to finish before clearing its model cache".into(),
        );
    }
    let root = pocket_tts_root(&app)?;
    clear_owned_model_cache(&root)
}

/// Legacy Hugging Face folders written by earlier versions directly into the
/// Pocket TTS root. They are unambiguous model artifacts and never the
/// installed runtime, which lives in the sibling `bin` directory.
const LEGACY_MODEL_FOLDERS: [&str; 5] = ["hub", "modules", "xet", "datasets", "assets"];

/// Remove the model directories Murmure owns and nothing else.
///
/// The installed runtime lives in the sibling `bin` directory and is never
/// a deletion candidate, so clearing the models can never break a verified
/// installation.
fn clear_owned_model_cache(root: &Path) -> Result<bool, String> {
    let mut targets = vec![root.join("models")];
    for legacy in LEGACY_MODEL_FOLDERS {
        targets.push(root.join(legacy));
    }
    // Validate every target before removing anything, so a hostile entry
    // cannot make the cleanup delete part of the cache and then fail.
    for target in &targets {
        check_removable_directory(target)?;
    }
    let mut removed = false;
    for target in &targets {
        if std::fs::symlink_metadata(target).is_ok() {
            std::fs::remove_dir_all(target)
                .map_err(|_| "Unable to clear the Pocket TTS model cache".to_string())?;
            removed = true;
        }
    }
    Ok(removed)
}

/// Prove that a path is a plain directory that Murmure may remove.
///
/// A symbolic link, a Windows junction or any other reparse point could
/// redirect the deletion outside the Murmure cache, so those are refused.
fn check_removable_directory(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("Unable to inspect the Pocket TTS model cache".into()),
    };
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
    {
        return Err("Pocket TTS model cache is not a regular directory".into());
    }
    Ok(())
}

/// Windows marks junctions and mount points as reparse points. Rust reports
/// them as ordinary directories, so the attribute has to be checked directly.
#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

/// Whether a verified runtime installation is currently running.
fn install_active() -> bool {
    #[cfg(target_os = "windows")]
    {
        super::install_windows::install_active()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
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
    fn clearing_owned_model_cache_preserves_sibling_data_and_is_idempotent() {
        let root =
            std::env::temp_dir().join(format!("murmure-cache-test-{}", uuid::Uuid::new_v4()));
        let owned = root.join("pocket-tts");
        let sibling = root.join("other-app");
        std::fs::create_dir_all(owned.join("models")).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(owned.join("models").join("model.bin"), b"model").unwrap();
        std::fs::write(sibling.join("important.bin"), b"keep").unwrap();
        assert_eq!(clear_owned_model_cache(&owned).unwrap(), true);
        assert!(!owned.join("models").exists());
        assert_eq!(clear_owned_model_cache(&owned).unwrap(), false);
        assert_eq!(
            std::fs::read(sibling.join("important.bin")).unwrap(),
            b"keep"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Regression: the Pocket TTS runtime and the model cache used to share
    /// the same directory on Windows, so clearing the models deleted the
    /// installed, verified runtime and forced a new download.
    #[test]
    fn clearing_the_model_cache_never_removes_the_installed_runtime() {
        let root =
            std::env::temp_dir().join(format!("murmure-runtime-test-{}", uuid::Uuid::new_v4()));
        let runtime = root.join("bin").join("pocket-tts.exe");
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join("models").join("hub")).unwrap();
        std::fs::write(&runtime, b"verified-runtime").unwrap();
        std::fs::write(root.join("models").join("hub").join("model.bin"), b"model").unwrap();

        assert_eq!(clear_owned_model_cache(&root).unwrap(), true);
        assert!(
            !root.join("models").join("hub").join("model.bin").exists(),
            "the model cache must be removed"
        );
        assert!(
            runtime.exists(),
            "clearing the model cache must never delete the installed runtime"
        );
        assert_eq!(std::fs::read(&runtime).unwrap(), b"verified-runtime");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clearing_the_model_cache_removes_the_legacy_huggingface_folders() {
        let root =
            std::env::temp_dir().join(format!("murmure-legacy-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("hub")).unwrap();
        std::fs::create_dir_all(root.join("modules")).unwrap();
        std::fs::write(root.join("hub").join("blob.bin"), b"blob").unwrap();
        assert_eq!(clear_owned_model_cache(&root).unwrap(), true);
        assert!(!root.join("hub").exists());
        assert!(!root.join("modules").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clearing_the_model_cache_is_a_no_op_when_no_cache_exists() {
        let root =
            std::env::temp_dir().join(format!("murmure-empty-test-{}", uuid::Uuid::new_v4()));
        assert_eq!(clear_owned_model_cache(&root).unwrap(), false);
        assert!(!root.exists());
    }

    #[cfg(unix)]
    #[test]
    fn clearing_the_model_cache_refuses_a_symlinked_model_root() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("murmure-link-test-{}", uuid::Uuid::new_v4()));
        let shared = root.join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("keep.bin"), b"keep").unwrap();
        symlink(&shared, root.join("models")).unwrap();
        assert!(clear_owned_model_cache(&root).is_err());
        assert_eq!(std::fs::read(shared.join("keep.bin")).unwrap(), b"keep");
        std::fs::remove_dir_all(root).unwrap();
    }

    /// A Windows junction is reported by Rust as an ordinary directory, so the
    /// reparse attribute has to be inspected explicitly.
    #[cfg(windows)]
    #[test]
    fn clearing_the_model_cache_refuses_a_junction_at_the_model_root() {
        let root =
            std::env::temp_dir().join(format!("murmure-junction-test-{}", uuid::Uuid::new_v4()));
        let shared = root.join("shared");
        let models = root.join("models");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("keep.bin"), b"keep").unwrap();
        let created = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                models.to_str().unwrap(),
                shared.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(created.success(), "creating the junction must succeed");
        assert!(
            clear_owned_model_cache(&root).is_err(),
            "a junction at the model root must be refused"
        );
        assert_eq!(std::fs::read(shared.join("keep.bin")).unwrap(), b"keep");
        let _ = std::fs::remove_dir(&models);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn clearing_cache_refuses_root_file() {
        let root = std::env::temp_dir().join(format!("murmure-file-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("models"), b"not a cache directory").unwrap();
        assert!(clear_owned_model_cache(&root).is_err());
        assert_eq!(
            std::fs::read(root.join("models")).unwrap(),
            b"not a cache directory"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    /// A command that runs for a while, used to exercise the generation
    /// bound and the user stop without the real Pocket TTS runtime.
    ///
    /// The Windows variant is a busy loop inside cmd.exe itself: it never
    /// spawns a grandchild, so stopping it cannot leave an orphan behind.
    #[cfg(windows)]
    fn long_running_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("cmd.exe"),
            vec![
                std::ffi::OsString::from("/c"),
                std::ffi::OsString::from(
                    "for /L %i in (1,1,100000) do @for /L %j in (1,1,100000) do @rem",
                ),
            ],
        )
    }

    #[cfg(unix)]
    fn long_running_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("sh"),
            vec![
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from("sleep 30"),
            ],
        )
    }

    /// A command that consumes the text and then succeeds without producing
    /// any output file. It must stay alive long enough to accept the text.
    #[cfg(windows)]
    fn silent_success_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("cmd.exe"),
            vec![
                std::ffi::OsString::from("/c"),
                std::ffi::OsString::from("more > nul"),
            ],
        )
    }

    #[cfg(unix)]
    fn silent_success_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("sh"),
            vec![
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from("cat > /dev/null"),
            ],
        )
    }

    /// A command that fails immediately, without reading the text.
    #[cfg(windows)]
    fn failing_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("cmd.exe"),
            vec![
                std::ffi::OsString::from("/c"),
                std::ffi::OsString::from("exit 1"),
            ],
        )
    }

    #[cfg(unix)]
    fn failing_command() -> (PathBuf, Vec<std::ffi::OsString>) {
        (
            PathBuf::from("sh"),
            vec![
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from("exit 1"),
            ],
        )
    }

    #[test]
    fn a_generation_that_produces_no_wave_is_reported() {
        let (binary, args) = silent_success_command();
        let (_sender, receiver) = mpsc::channel();
        let wave =
            std::env::temp_dir().join(format!("murmure-no-wave-{}.wav", uuid::Uuid::new_v4()));
        let outcome = supervise_generation(
            &binary,
            &args,
            "texte",
            Path::new("cache"),
            &receiver,
            Duration::from_secs(30),
            |_| {},
        );
        assert!(matches!(outcome, Ok(GenerationOutcome::Completed)));
        assert!(
            verify_generated_wave(&wave).is_err(),
            "a missing WAV output must be reported"
        );
    }

    #[test]
    fn a_corrupted_wave_output_is_reported() {
        let wave =
            std::env::temp_dir().join(format!("murmure-bad-wave-{}.wav", uuid::Uuid::new_v4()));
        std::fs::write(&wave, b"this is not a RIFF container at all").unwrap();
        assert!(
            verify_generated_wave(&wave).is_err(),
            "a corrupted WAV output must be reported"
        );
        std::fs::remove_file(&wave).unwrap();
    }

    #[test]
    fn a_generation_that_exceeds_its_bound_is_stopped() {
        let (binary, args) = long_running_command();
        let (_sender, receiver) = mpsc::channel();
        let outcome = supervise_generation(
            &binary,
            &args,
            "texte",
            Path::new("cache"),
            &receiver,
            Duration::from_millis(300),
            |_| {},
        );
        assert!(
            outcome.is_err(),
            "a generation that exceeds its bound must be stopped and reported"
        );
    }

    #[test]
    fn a_user_stop_terminates_the_generation() {
        let (binary, args) = long_running_command();
        let (sender, receiver) = mpsc::channel();
        let stopper = sender.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let _ = stopper.send(Request::Stop);
        });
        let outcome = supervise_generation(
            &binary,
            &args,
            "texte",
            Path::new("cache"),
            &receiver,
            Duration::from_secs(30),
            |_| {},
        );
        assert!(
            matches!(outcome, Ok(GenerationOutcome::Stopped)),
            "an explicit stop must end the generation instead of failing it"
        );
    }

    #[test]
    fn a_generation_that_fails_immediately_is_reported_as_a_failure() {
        let (binary, args) = failing_command();
        let (_sender, receiver) = mpsc::channel();
        let outcome = supervise_generation(
            &binary,
            &args,
            "texte",
            Path::new("cache"),
            &receiver,
            Duration::from_secs(30),
            |_| {},
        );
        assert!(
            outcome.is_err(),
            "a runtime that exits with a failure must be reported as a generation failure"
        );
    }

    #[test]
    fn actual_audio_queue_obeys_pause_resume_and_stop_without_a_sound_card() {
        use rodio::buffer::SamplesBuffer;
        use std::num::{NonZeroU16, NonZeroU32};

        // Rodio v0.22 samples control flags through periodic_access every 5 ms,
        // not every sample. At 24 kHz that is about 120 frames. Verify that
        // pause/resume/stop take effect within four callback intervals, rather
        // than claiming an impossible sample-exact transition.
        const MAX_CONTROL_LATENCY_FRAMES: usize = 480;
        let (player, mut output) = rodio::Player::new();
        let mono = NonZeroU16::new(1).unwrap();
        let sample_rate = NonZeroU32::new(24_000).unwrap();
        player.append(SamplesBuffer::new(mono, sample_rate, vec![0.5_f32; 4_096]));
        assert_eq!(output.next(), Some(0.5));

        assert_eq!(
            apply_player_request(&player, &Request::Pause),
            PlayerTransition::Paused
        );
        assert!(player.is_paused());
        assert!(
            (0..MAX_CONTROL_LATENCY_FRAMES).any(|_| output.next() == Some(0.0)),
            "Rodio did not silence playback within 20 ms after Pause"
        );
        for _ in 0..8 {
            assert_eq!(output.next(), Some(0.0), "paused playback leaked samples");
        }

        assert_eq!(
            apply_player_request(&player, &Request::Resume),
            PlayerTransition::Playing
        );
        assert!(!player.is_paused());
        assert!(
            (0..MAX_CONTROL_LATENCY_FRAMES).any(|_| output.next() == Some(0.5)),
            "Rodio did not resume within 20 ms"
        );

        assert_eq!(
            apply_player_request(&player, &Request::Stop),
            PlayerTransition::Stopped
        );
        assert!(
            (0..MAX_CONTROL_LATENCY_FRAMES).any(|_| output.next() == Some(0.0)),
            "Rodio did not silence playback within 20 ms after Stop"
        );
        assert!(player.empty(), "Stop did not empty the playback queue");
    }

    #[test]
    fn spurious_generate_command_does_not_interrupt_existing_player() {
        use rodio::buffer::SamplesBuffer;
        use std::num::NonZeroU32;
        use std::num::{NonZeroU16, NonZeroU32};

        let (player, mut output) = rodio::Player::new();
        player.append(SamplesBuffer::new(
            NonZeroU16::new(1).unwrap(),
            NonZeroU32::new(24_000).unwrap(),
            vec![0.25, -0.25, 0.2],
        ));
        assert_eq!(
            apply_player_request(&player, &Request::Preview),
            PlayerTransition::Noop
        );
        assert_eq!(
            apply_player_request(&player, &Request::Speak("unused".into())),
            PlayerTransition::Noop
        );
        assert_eq!(output.next(), Some(0.25));
    }

    #[test]
    fn model_source_does_not_inherit_user_huggingface_credentials() {
        let mut command = Command::new("pocket-tts");
        configure_model_source(&mut command, Path::new("isolated-cache"));
        let entries: std::collections::HashMap<_, _> = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(entries.get("HF_TOKEN"), Some(&None));
        assert_eq!(entries.get("HF_TOKEN_PATH"), Some(&None));
        assert_eq!(entries.get("HUGGINGFACE_HUB_CACHE"), Some(&None));
        assert_eq!(
            entries.get("HF_ENDPOINT"),
            Some(&Some("https://huggingface.co".to_string()))
        );
        assert_eq!(
            entries.get("HF_HOME"),
            Some(&Some("isolated-cache".to_string()))
        );
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
        let file = std::env::temp_dir().join(format!(
            "murmure-tts-integrity-{}.bin",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&file, b"abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_sidecar(&file, expected).is_ok());
        std::fs::write(&file, b"abcd").unwrap();
        assert!(verify_sidecar(&file, expected).is_err());
        std::fs::remove_file(&file).unwrap();
        assert!(verify_sidecar(&file, expected).is_err());
    }

    #[test]
    fn installed_sidecar_identity_fails_closed_without_a_real_file() {
        assert!(verify_sidecar(
            Path::new("nonexistent-murmure-tts-runtime-for-test.exe"),
            "invalid",
        )
        .is_err());
        #[cfg(target_os = "windows")]
        {
            assert_eq!(WINDOWS_SIDECAR_SHA256.len(), 64);
            assert!(WINDOWS_SIDECAR_SHA256
                .bytes()
                .all(|b| b.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn speak_filters_obvious_secrets_and_requires_text() {
        assert_eq!(
            checked_text("Bonjour à tous.\npassword=nepaslire").unwrap(),
            "Bonjour à tous."
        );
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
