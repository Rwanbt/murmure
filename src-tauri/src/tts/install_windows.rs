//! User-initiated installation of a pinned Kyutai Pocket TTS Windows sidecar.
//! No arbitrary URLs, shell commands, ZIP paths or unverified binaries execute.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;

const RELEASE_URL: &str = "https://github.com/kyutai-labs/pocket-tts/releases/download/v4.0.0/pocket-tts-3.3.0-x86_64-pc-windows-msvc.zip";
const ARCHIVE_SHA256: &str = "b3b2faa74703c505e22a7f428c984f69577548c85440c4c9eee0d336797f3cef";
const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXE_BYTES: u64 = 64 * 1024 * 1024;
static INSTALL_ACTIVE: AtomicBool = AtomicBool::new(false);
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    received_bytes: usize,
    total_bytes: Option<u64>,
}

pub fn cancel_install() -> bool {
    if INSTALL_ACTIVE.load(Ordering::Acquire) {
        CANCEL_REQUESTED.store(true, Ordering::Release);
        true
    } else {
        false
    }
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
        INSTALL_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A Pocket TTS installation is already running".to_string())?;
        CANCEL_REQUESTED.store(false, Ordering::Release);
        Ok(Self)
    }
}
impl Drop for ActiveInstall {
    fn drop(&mut self) {
        INSTALL_ACTIVE.store(false, Ordering::Release);
        CANCEL_REQUESTED.store(false, Ordering::Release);
    }
}

struct PendingFile(PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn expected_digest(bytes: &[u8]) -> bool {
    format!("{:x}", Sha256::digest(bytes)) == ARCHIVE_SHA256
}

fn install_verified_exe(archive_bytes: Vec<u8>, directory: &Path) -> Result<(), String> {
    if !expected_digest(&archive_bytes) {
        return Err("Pocket TTS archive SHA-256 mismatch".into());
    }
    std::fs::create_dir_all(directory)
        .map_err(|_| "Cannot create the Pocket TTS installation directory".to_string())?;
    if !std::fs::symlink_metadata(directory)
        .map_err(|_| "Cannot inspect the installation directory".to_string())?
        .file_type().is_dir()
    {
        return Err("Pocket TTS installation directory is not a real directory".into());
    }

    let target = directory.join("pocket-tts.exe");
    if std::fs::symlink_metadata(&target).is_ok() {
        // Reject existing files, symlinks and other filesystem objects; never
        // overwrite an executable or a stale/broken symbolic link.
        return super::preview::verify_sidecar(&target, super::preview::WINDOWS_SIDECAR_SHA256);
    }
    let reader = std::io::Cursor::new(archive_bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|_| "Pocket TTS release archive is not a valid ZIP".to_string())?;
    let matching: Vec<usize> = (0..archive.len()).filter(|&i| {
        archive.by_index(i).is_ok_and(|entry| {
            entry.is_file() && entry.name().replace('\\', "/")
                .rsplit('/').next() == Some("pocket-tts.exe")
        })
    }).collect();
    if matching.len() != 1 {
        return Err("Expected exactly one Pocket TTS executable in the release".into());
    }
    let mut entry = archive.by_index(matching[0])
        .map_err(|_| "Unable to open Pocket TTS executable in archive".to_string())?;
    if entry.size() > MAX_EXE_BYTES {
        return Err("Pocket TTS executable exceeds its size limit".into());
    }
    // Fixed destination basename; never use an entry path supplied by the ZIP.
    let temporary = PendingFile(directory.join(format!(
        ".pocket-tts-{}.partial", uuid::Uuid::new_v4()
    )));
    let mut output = std::fs::OpenOptions::new()
        .write(true).create_new(true).open(&temporary.0)
        .map_err(|_| "Unable to stage the Pocket TTS executable".to_string())?;
    let copied = std::io::copy(&mut entry.take(MAX_EXE_BYTES + 1), &mut output)
        .map_err(|_| "Unable to extract Pocket TTS executable".to_string())?;
    if copied > MAX_EXE_BYTES {
        return Err("Extracted Pocket TTS executable exceeds its size limit".into());
    }
    output.flush().map_err(|_| "Unable to flush Pocket TTS binary".to_string())?;
    output.sync_all().map_err(|_| "Unable to save Pocket TTS binary".to_string())?;
    drop(output);
    super::preview::verify_sidecar(
        &temporary.0, super::preview::WINDOWS_SIDECAR_SHA256,
    )?;
    check_cancelled()?;
    // On Windows std::fs::rename may replace an existing destination.
    // An atomic hard link cannot overwrite an existing file or symlink,
    // including one created concurrently after the initial check.
    // Both paths are in the same directory / volume; the Drop guard
    // removes the staging name and leaves the published verified binary.
    std::fs::hard_link(&temporary.0, &target)
        .map_err(|_| "Unable to publish Pocket TTS without replacing existing files".to_string())?;
    Ok(())
}

pub async fn install(app: AppHandle) -> Result<(), String> {
    let _active = ActiveInstall::acquire()?;
    if !super::preview::cpu_compatible() {
        return Err("This Pocket TTS binary requires an AVX2 CPU".into());
    }
    let directory = app.path().app_local_data_dir()
        .map_err(|_| "Unable to locate the application data directory".to_string())?
        .join("pocket-tts").join("bin");
    let target = directory.join("pocket-tts.exe");
    if std::fs::symlink_metadata(&target).is_ok() {
        return super::preview::verify_sidecar(
            &target, super::preview::WINDOWS_SIDECAR_SHA256,
        );
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Unable to initialize the Pocket TTS downloader".to_string())?;
    let fetch = client.get(RELEASE_URL).send();
    tokio::pin!(fetch);
    let response = loop {
        tokio::select! {
            result = &mut fetch => break result
                .map_err(|_| "Unable to download the official Pocket TTS release".to_string())?,
            _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => check_cancelled()?,
        }
    }.error_for_status()
        .map_err(|_| "The official Pocket TTS release is unavailable".to_string())?;
    if response.content_length().is_some_and(|n| n > MAX_ARCHIVE_BYTES as u64) {
        return Err("Pocket TTS archive is larger than permitted".into());
    }
    let total_bytes = response.content_length();
    let mut data = Vec::new();
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
        let chunk = chunk.map_err(|_| "Pocket TTS archive download was interrupted".to_string())?;
        if chunk.len() > MAX_ARCHIVE_BYTES.saturating_sub(data.len()) {
            return Err("Pocket TTS archive exceeded its download limit".into());
        }
        data.extend_from_slice(&chunk);
        let _ = app.emit("pocket-tts-install-progress", InstallProgress {
            received_bytes: data.len(),
            total_bytes,
        });
    }
    check_cancelled()?;
    tokio::task::spawn_blocking(move || install_verified_exe(data, &directory))
        .await.map_err(|_| "Pocket TTS installation worker failed".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_digest_rejects_unknown_downloads() {
        assert!(!expected_digest(b"not a Pocket TTS release"));
    }

    #[test]
    #[ignore = "uses the official pinned Kyutai Windows ZIP downloaded in CI"]
    fn official_archive_extracts_untampered_binary_and_refuses_overwrite() {
        let filename = std::env::var_os("MURMURE_TTS_OFFICIAL_ARCHIVE")
            .expect("MURMURE_TTS_OFFICIAL_ARCHIVE must be the SHA-256 verified archive");
        let archive = std::fs::read(filename).expect("cannot read official Pocket TTS archive");
        let directory = std::env::temp_dir()
            .join(format!("murmure-official-sidecar-{}", uuid::Uuid::new_v4()));
        install_verified_exe(archive.clone(), &directory)
            .expect("official Pocket TTS archive must install");
        let binary = directory.join("pocket-tts.exe");
        super::super::preview::verify_sidecar(
            &binary, super::super::preview::WINDOWS_SIDECAR_SHA256,
        ).expect("installed exe must match the pinned official digest");
        install_verified_exe(archive.clone(), &directory)
            .expect("reinstalling an already verified binary must be idempotent");
        std::fs::write(&binary, b"tampered").expect("tamper fixture");
        assert!(install_verified_exe(archive, &directory).is_err(),
            "installer must never replace an unknown existing binary");
        std::fs::remove_dir_all(&directory).expect("cleanup isolated installer fixture");
    }

    #[test]
    fn rejected_archive_never_creates_a_sidecar_or_staging_files() {
        let directory = std::env::temp_dir()
            .join(format!("murmure-tts-invalid-{}", uuid::Uuid::new_v4()));
        assert!(install_verified_exe(b"not a verified release".to_vec(), &directory).is_err());
        assert!(!directory.exists(), "invalid archive unexpectedly created the installation directory");
    }

    #[test]
    fn publishing_verified_binary_never_overwrites_an_existing_target() {
        let directory = std::env::temp_dir()
            .join(format!("murmure-tts-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let staged = directory.join("staged.partial");
        let target = directory.join("pocket-tts.exe");
        std::fs::write(&staged, b"verified-for-test").unwrap();
        std::fs::write(&target, b"preexisting-file").unwrap();
        assert!(std::fs::hard_link(&staged, &target).is_err(),
            "publishing unexpectedly replaced a preexisting target");
        assert_eq!(std::fs::read(&target).unwrap(), b"preexisting-file");
        std::fs::remove_file(&target).unwrap();
        std::fs::hard_link(&staged, &target).expect("publish into empty destination");
        std::fs::remove_file(&staged).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"verified-for-test");
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn concurrent_installs_are_rejected() {
        assert!(!cancel_install());
        let lock = ActiveInstall::acquire().unwrap();
        assert!(ActiveInstall::acquire().is_err());
        assert!(cancel_install());
        assert!(check_cancelled().is_err());
        drop(lock);
        assert!(!cancel_install());
        assert!(check_cancelled().is_ok());
        assert!(ActiveInstall::acquire().is_ok());
    }
}
