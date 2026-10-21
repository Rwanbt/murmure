//! Installation of the verified Pocket TTS runtime on Linux and macOS.
//!
//! The official Kyutai release publishes a build for every desktop target, so
//! the runtime is not a Windows privilege. The archive is downloaded from the
//! official release, its published digest is checked before anything is
//! extracted, the executable is checked again once extracted, and it is
//! published under its final name with a hard link, which cannot replace a
//! file that already exists.
//!
//! This module is the counterpart of `install_windows`. The two are mutually
//! exclusive: only one of them is compiled for a given target, so the control
//! state of one platform cannot ever interfere with the other.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

/// The archive of the target this build runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Release {
    archive: &'static str,
    archive_sha256: &'static str,
    executable_sha256: &'static str,
}

/// The release that is pinned for the current target.
///
/// A target that is absent here was never qualified, so the installer refuses
/// to run instead of trusting a binary whose digest nobody checked.
fn release_for_current_target() -> Option<Release> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some(Release {
            archive: "pocket-tts-3.3.0-x86_64-unknown-linux-gnu.tar.gz",
            archive_sha256: "1336a77e60ce11650a5a3dc354770155a345bfa797f7a05d10bc5b1bcfe0d7e3",
            executable_sha256: "a79b84ee22bd197bd54e00e8835d774284592f00813845133d4d0e77bcb72bc3",
        });
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some(Release {
            archive: "pocket-tts-3.3.0-aarch64-unknown-linux-gnu.tar.gz",
            archive_sha256: "644a9478d7cbd7d432f5a4898007aa7d178a897a65aef7ebc64557e0b1a1739a",
            executable_sha256: "60ad23a367ea35f0ceb144ee081db4f533e93e8011c44ac4dfd3d1bff3d06ff8",
        });
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some(Release {
            archive: "pocket-tts-3.3.0-x86_64-apple-darwin.tar.gz",
            archive_sha256: "3a908c524d1ca976eb5f699c523fbdd47a5fa029ccc3bd9f6197b8f8d91f01c1",
            executable_sha256: "6531c6d752c7f1c711869f6ecf9ee970f9f19e0348db93bdf01ec4af943de0cb",
        });
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some(Release {
            archive: "pocket-tts-3.3.0-aarch64-apple-darwin.tar.gz",
            archive_sha256: "e209feca849d53df18eba89f5bd4c253c1be62a6c54794db5654635c95e02990",
            executable_sha256: "6a7a6cb0acf1f0a216027bad2a6910d3523e5344cf29f6b2f9083effe8d795ee",
        });
    }
    #[allow(unreachable_code)]
    None
}

/// The digest that the runtime of this target has to match.
///
/// The speech path verifies it before every launch, so a binary that was
/// replaced after the installation is never executed.
pub fn executable_sha256_for_current_target() -> Option<&'static str> {
    release_for_current_target().map(|release| release.executable_sha256)
}

const RELEASE_BASE: &str = "https://github.com/kyutai-labs/pocket-tts/releases/download/v4.0.0";
/// The archives are between twenty and thirty megabytes. The bound leaves
/// room for a larger build of the same release without allowing a runaway
/// download.
const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;

static INSTALL_ACTIVE: AtomicBool = AtomicBool::new(false);
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
// The control is held only for the short transitions, never across the
// download or the extraction, so cancellation can never be lost.
static INSTALL_CONTROL: Mutex<()> = Mutex::new(());
static PUBLISHED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    received_bytes: usize,
    total_bytes: Option<u64>,
}

pub fn cancel_install() -> bool {
    let Ok(_guard) = INSTALL_CONTROL.lock() else {
        return false;
    };
    if !INSTALL_ACTIVE.load(Ordering::Acquire) || PUBLISHED.load(Ordering::Acquire) {
        return false;
    }
    CANCEL_REQUESTED.store(true, Ordering::Release);
    true
}

/// Whether an installation is running. The model cache cleanup must never
/// overlap with the publication of the verified runtime.
pub fn install_active() -> bool {
    INSTALL_ACTIVE.load(Ordering::Acquire)
}

fn check_cancelled() -> Result<(), String> {
    if CANCEL_REQUESTED.load(Ordering::Acquire) {
        Err("Pocket TTS installation cancelled".to_string())
    } else {
        Ok(())
    }
}

struct ActiveInstall;
impl ActiveInstall {
    fn acquire() -> Result<Self, String> {
        let _guard = INSTALL_CONTROL
            .lock()
            .map_err(|_| "TTS installer state is unavailable".to_string())?;
        INSTALL_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A Pocket TTS installation is already running".to_string())?;
        CANCEL_REQUESTED.store(false, Ordering::Release);
        PUBLISHED.store(false, Ordering::Release);
        Ok(Self)
    }
}
impl Drop for ActiveInstall {
    fn drop(&mut self) {
        if let Ok(_guard) = INSTALL_CONTROL.lock() {
            INSTALL_ACTIVE.store(false, Ordering::Release);
            CANCEL_REQUESTED.store(false, Ordering::Release);
            PUBLISHED.store(false, Ordering::Release);
        }
    }
}

struct PendingFile(PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn digest_of(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Extracts the single executable of the release, refusing anything that is
/// not exactly one file named `pocket-tts`.
fn extract_executable(archive_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_bytes));
    let mut archive = tar::Archive::new(decoder);
    let mut found: Option<Vec<u8>> = None;
    let entries = archive
        .entries()
        .map_err(|_| "The Pocket TTS archive cannot be read".to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|_| "The Pocket TTS archive is damaged".to_string())?;
        // Only regular files are considered; a symbolic link in the archive
        // could otherwise be published as if it were the runtime.
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|_| "The Pocket TTS archive holds an unreadable path".to_string())?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        if name != "pocket-tts" {
            continue;
        }
        if found.is_some() {
            return Err("The Pocket TTS archive holds more than one executable".into());
        }
        if entry.size() > MAX_EXECUTABLE_BYTES {
            return Err("The Pocket TTS executable exceeds its size limit".into());
        }
        let mut bytes = Vec::new();
        entry
            .take(MAX_EXECUTABLE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The Pocket TTS executable cannot be read".to_string())?;
        if bytes.len() as u64 > MAX_EXECUTABLE_BYTES {
            return Err("The Pocket TTS executable exceeds its size limit".into());
        }
        found = Some(bytes);
    }
    found.ok_or_else(|| "The Pocket TTS release does not hold the executable".to_string())
}

fn install_verified_executable(
    archive_bytes: Vec<u8>,
    directory: &Path,
    release: Release,
) -> Result<(), String> {
    if digest_of(&archive_bytes) != release.archive_sha256 {
        return Err("Pocket TTS archive SHA-256 mismatch".into());
    }
    std::fs::create_dir_all(directory)
        .map_err(|_| "Cannot create the Pocket TTS installation directory".to_string())?;
    if !std::fs::symlink_metadata(directory)
        .map_err(|_| "Cannot inspect the installation directory".to_string())?
        .file_type()
        .is_dir()
    {
        return Err("The Pocket TTS installation directory is not a real directory".into());
    }

    let target = directory.join("pocket-tts");
    if std::fs::symlink_metadata(&target).is_ok() {
        // Never overwrite an executable, a symbolic link or a stale object.
        return super::preview::verify_sidecar(&target, release.executable_sha256);
    }

    let executable = extract_executable(&archive_bytes)?;
    if digest_of(&executable) != release.executable_sha256 {
        return Err("The extracted Pocket TTS executable SHA-256 mismatch".into());
    }

    // The staging name is fixed to the destination directory and never comes
    // from the archive, so an entry cannot escape the installation directory.
    let temporary =
        PendingFile(directory.join(format!(".pocket-tts-{}.partial", uuid::Uuid::new_v4())));
    {
        use std::io::Write;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary.0)
            .map_err(|_| "Unable to stage the Pocket TTS executable".to_string())?;
        output
            .write_all(&executable)
            .map_err(|_| "Unable to extract the Pocket TTS executable".to_string())?;
        output
            .flush()
            .map_err(|_| "Unable to flush the Pocket TTS executable".to_string())?;
        output
            .sync_all()
            .map_err(|_| "Unable to save the Pocket TTS executable".to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary.0, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| "Unable to make the Pocket TTS executable runnable".to_string())?;
    }
    super::preview::verify_sidecar(&temporary.0, release.executable_sha256)?;

    let _guard = INSTALL_CONTROL
        .lock()
        .map_err(|_| "TTS installer state is unavailable".to_string())?;
    check_cancelled()?;
    // A hard link cannot replace a file that already exists, including one
    // created concurrently after the check above.
    std::fs::hard_link(&temporary.0, &target)
        .map_err(|_| "Unable to publish Pocket TTS without replacing existing files".to_string())?;
    PUBLISHED.store(true, Ordering::Release);
    Ok(())
}

pub async fn install(app: AppHandle) -> Result<(), String> {
    let release = release_for_current_target().ok_or_else(|| {
        "The official Pocket TTS release has no build for this platform, so it was never qualified"
            .to_string()
    })?;
    let _active = ActiveInstall::acquire()?;
    if !super::preview::cpu_compatible() {
        return Err("This Pocket TTS binary requires an AVX2 CPU".into());
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Unable to locate the application data directory".to_string())?
        .join("pocket-tts")
        .join("bin");
    let target = directory.join("pocket-tts");
    if std::fs::symlink_metadata(&target).is_ok() {
        return super::preview::verify_sidecar(&target, release.executable_sha256);
    }

    let url = format!("{RELEASE_BASE}/{}", release.archive);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Unable to initialize the Pocket TTS downloader".to_string())?;
    let fetch = client.get(url).send();
    tokio::pin!(fetch);
    let response = loop {
        tokio::select! {
            result = &mut fetch => break result
                .map_err(|_| "Unable to download the official Pocket TTS release".to_string())?,
            _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => check_cancelled()?,
        }
    }
    .error_for_status()
    .map_err(|_| "The official Pocket TTS release is unavailable".to_string())?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ARCHIVE_BYTES as u64)
    {
        return Err("The Pocket TTS archive is larger than permitted".into());
    }
    let total_bytes = response.content_length();
    let mut data = Vec::new();
    {
        use futures_util::StreamExt;
        let mut chunks = response.bytes_stream();
        loop {
            let next = tokio::select! {
                chunk = chunks.next() => chunk,
                _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {
                    check_cancelled()?;
                    continue;
                }
            };
            let Some(chunk) = next else { break };
            check_cancelled()?;
            let chunk =
                chunk.map_err(|_| "The Pocket TTS archive download was interrupted".to_string())?;
            if chunk.len() > MAX_ARCHIVE_BYTES.saturating_sub(data.len()) {
                return Err("The Pocket TTS archive exceeded its download limit".into());
            }
            data.extend_from_slice(&chunk);
            let _ = app.emit(
                "pocket-tts-install-progress",
                InstallProgress {
                    received_bytes: data.len(),
                    total_bytes,
                },
            );
        }
    }
    check_cancelled()?;

    // The extraction and the file system work are blocking, so they do not
    // run on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        install_verified_executable(data, &directory, release)
    })
    .await
    .map_err(|_| "The Pocket TTS installation task failed".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_target_has_a_pinned_release() {
        #[cfg(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        {
            let release = release_for_current_target().expect("the target must be pinned");
            assert_eq!(release.archive_sha256.len(), 64);
            assert_eq!(release.executable_sha256.len(), 64);
            assert!(release.archive.ends_with(".tar.gz"));
        }
    }

    #[test]
    fn a_corrupted_archive_is_refused_before_anything_is_written() {
        let release = Release {
            archive: "x.tar.gz",
            archive_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            executable_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        };
        let directory =
            std::env::temp_dir().join(format!("murmure-posix-{}", uuid::Uuid::new_v4()));
        let error = install_verified_executable(b"not the release".to_vec(), &directory, release)
            .expect_err("a corrupted archive must be refused");
        assert!(error.contains("SHA-256"));
        assert!(
            !directory.join("pocket-tts").exists(),
            "nothing may be published from a corrupted archive"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn an_archive_without_the_executable_is_refused() {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(3);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "release/README.md", &b"abc"[..])
            .expect("the entry can be appended");
        let encoder = builder.into_inner().expect("the archive can be finished");
        let bytes = encoder.finish().expect("the archive can be compressed");

        let error =
            extract_executable(&bytes).expect_err("an archive without the runtime is refused");
        assert!(error.contains("does not hold the executable"));
    }

    #[test]
    fn the_extraction_takes_the_file_name_and_not_the_archive_path() {
        // An entry whose path tries to climb out of the installation
        // directory must still be published under its own name, because the
        // destination is built from a fixed name and never from the entry.
        assert_eq!(
            Path::new("../../evil/pocket-tts")
                .file_name()
                .and_then(|value| value.to_str()),
            Some("pocket-tts")
        );
    }
}
