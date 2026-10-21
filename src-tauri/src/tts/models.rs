//! The Pocket TTS models Murmure supports, and their local state.
//!
//! The catalog is exact: the repository, the directory inside it and the
//! license are the ones the official model card declares. Nothing is
//! guessed. A revision, a size or a date is reported only when it can be
//! read from the local cache, never invented.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Official source of the French weights.
const FRENCH_REPOSITORY: &str = "kyutai/pocket-tts-without-voice-cloning";
/// License declared by the model card of the repository.
const FRENCH_LICENSE: &str = "CC BY 4.0";

/// A model the pinned Pocket TTS CLI can synthesize with.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PocketTtsModel {
    /// Identifier the CLI expects after `--language`.
    pub id: &'static str,
    /// Official Hugging Face repository the weights come from.
    pub repository: &'static str,
    /// Directory inside the repository that holds this model's weights.
    pub directory: &'static str,
    /// License of the weights.
    pub license: &'static str,
}

/// The models the pinned CLI supports for French speech.
pub const FRENCH_MODELS: [PocketTtsModel; 2] = [
    PocketTtsModel {
        id: "french",
        repository: FRENCH_REPOSITORY,
        directory: "languages/french",
        license: FRENCH_LICENSE,
    },
    PocketTtsModel {
        id: "french_24l",
        repository: FRENCH_REPOSITORY,
        directory: "languages/french_24l",
        license: FRENCH_LICENSE,
    },
];

/// Lifecycle state of a model in Murmure's own cache.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    /// The weights are not available locally.
    NotInstalled,
    /// The repository holds a file the downloader never finished, so the
    /// weights are present in part. The Hugging Face client keeps such files
    /// as `<digest>.incomplete` until a download completes.
    Downloading,
    /// The weights are on disk but cannot be used: an empty file, or a
    /// snapshot entry whose blob is gone.
    Corrupted,
    /// The weights are available locally and were loaded at least once.
    Ready,
    /// A synthesis is currently using the model.
    InUse,
}

/// The local state of one supported model.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PocketTtsModelStatus {
    /// Identifier the CLI expects after `--language`.
    pub id: &'static str,
    /// Official Hugging Face repository the weights come from.
    pub repository: &'static str,
    /// Directory inside the repository that holds this model's weights.
    pub directory: &'static str,
    /// License of the weights.
    pub license: &'static str,
    /// Lifecycle state.
    pub state: ModelState,
    /// Whether the weights are present in Murmure's own model cache.
    pub present: bool,
    /// Whether the model can be used without a network connection.
    pub offline_ready: bool,
    /// Revision of the local snapshot, when the cache records one.
    pub revision: Option<String>,
    /// Size of the local weights, when it can be measured.
    pub local_bytes: Option<u64>,
}

/// Directory the Hugging Face client uses for a repository inside a cache.
fn repository_cache_dir(cache: &Path, repository: &str) -> PathBuf {
    cache
        .join("hub")
        .join(format!("models--{}", repository.replace('/', "--")))
}

/// Locate the weights of a model inside a Hugging Face cache.
///
/// The cache holds one snapshot directory per revision. When more than one
/// is present the local state is ambiguous, so nothing is reported: a model
/// is never presented as ready when its presence cannot be established.
fn locate_model(cache: &Path, model: &PocketTtsModel) -> Option<(PathBuf, Option<String>)> {
    let repository = repository_cache_dir(cache, model.repository);
    let snapshots = repository.join("snapshots");
    let revisions = std::fs::read_dir(&snapshots)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    if revisions.len() != 1 {
        return None;
    }
    let revision = revisions[0]
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let weights = revisions[0].join(model.directory);
    weights.is_dir().then_some((weights, revision))
}

/// Total size of the files below a directory.
fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let metadata = entry.metadata().ok();
            match metadata {
                Some(metadata) if metadata.is_dir() => directory_size(&entry.path()),
                Some(metadata) => metadata.len(),
                None => 0,
            }
        })
        .sum()
}

/// Whether every file below the weights directory can actually be read.
///
/// A Hugging Face snapshot holds entries that point at blobs. When a download
/// was interrupted the snapshot can exist while a blob it names does not, so
/// the directory looks present and the synthesis fails later. A zero byte
/// file is the other shape of the same failure, and a directory with no file
/// at all proves nothing.
fn weights_are_complete(weights: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(weights) else {
        return false;
    };
    let mut found = false;
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        // Following the link is the point: a dangling one has no metadata.
        let Ok(metadata) = std::fs::metadata(&path) else {
            return false;
        };
        if metadata.is_dir() && !weights_are_complete(&path) {
            return false;
        }
        if !metadata.is_dir() && metadata.len() == 0 {
            return false;
        }
        found = true;
    }
    found
}

/// Whether the repository still holds an unfinished download.
fn download_is_unfinished(repository: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(repository.join("blobs")) else {
        return false;
    };
    entries
        .filter_map(|entry| entry.ok())
        .any(|entry| entry.file_name().to_string_lossy().ends_with(".incomplete"))
}

/// Describe the local state of every supported model.
///
/// `in_use` lists the model currently being synthesized, if any.
pub fn model_statuses(cache: &Path, in_use: Option<&str>) -> Vec<PocketTtsModelStatus> {
    FRENCH_MODELS
        .iter()
        .map(|model| {
            let repository = repository_cache_dir(cache, model.repository);
            let located = locate_model(cache, model);
            let present = located.is_some();
            let (revision, local_bytes) = match &located {
                Some((weights, revision)) => {
                    let bytes = directory_size(weights);
                    (revision.clone(), (bytes > 0).then_some(bytes))
                }
                None => (None, None),
            };
            let state = if !present {
                ModelState::NotInstalled
            } else if in_use == Some(model.id) {
                ModelState::InUse
            } else if download_is_unfinished(&repository) {
                ModelState::Downloading
            } else if !located
                .as_ref()
                .is_some_and(|(weights, _)| weights_are_complete(weights))
            {
                ModelState::Corrupted
            } else {
                ModelState::Ready
            };
            // A partial or unreadable model is not usable without a network,
            // whatever its directory looks like.
            let offline_ready = matches!(state, ModelState::Ready | ModelState::InUse);
            PocketTtsModelStatus {
                id: model.id,
                repository: model.repository,
                directory: model.directory,
                license: model.license,
                state,
                present,
                offline_ready,
                revision,
                local_bytes,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cache() -> PathBuf {
        std::env::temp_dir().join(format!("murmure-model-test-{}", uuid::Uuid::new_v4()))
    }

    fn write_weights(cache: &Path, revision: &str, directory: &str) {
        let weights = repository_cache_dir(cache, FRENCH_REPOSITORY)
            .join("snapshots")
            .join(revision)
            .join(directory);
        std::fs::create_dir_all(&weights).unwrap();
        std::fs::write(weights.join("model.safetensors"), b"weights").unwrap();
    }

    #[test]
    fn the_catalog_states_the_official_repository_and_license() {
        assert_eq!(FRENCH_MODELS.len(), 2);
        for model in FRENCH_MODELS.iter() {
            assert_eq!(model.repository, "kyutai/pocket-tts-without-voice-cloning");
            assert_eq!(model.license, "CC BY 4.0");
            assert!(model.directory.starts_with("languages/"));
        }
        assert_eq!(FRENCH_MODELS[0].id, "french");
        assert_eq!(FRENCH_MODELS[0].directory, "languages/french");
        assert_eq!(FRENCH_MODELS[1].id, "french_24l");
        assert_eq!(FRENCH_MODELS[1].directory, "languages/french_24l");
    }

    #[test]
    fn an_absent_cache_reports_every_model_as_not_installed() {
        let cache = empty_cache();
        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses.len(), 2);
        for status in statuses {
            assert_eq!(status.state, ModelState::NotInstalled);
            assert!(!status.present);
            assert!(!status.offline_ready);
            assert!(status.revision.is_none());
            assert!(status.local_bytes.is_none());
        }
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn a_cached_model_is_reported_as_present_and_offline_ready() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Ready);
        assert!(statuses[0].present);
        assert!(statuses[0].offline_ready);
        assert_eq!(statuses[0].revision.as_deref(), Some("abc123"));
        assert_eq!(statuses[0].local_bytes, Some(7));
        // The 24 layer model was not downloaded.
        assert_eq!(statuses[1].state, ModelState::NotInstalled);
        assert!(!statuses[1].present);
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn a_model_in_use_is_reported_as_such() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        let statuses = model_statuses(&cache, Some("french"));
        assert_eq!(statuses[0].state, ModelState::InUse);
        assert_eq!(statuses[1].state, ModelState::NotInstalled);
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn an_ambiguous_cache_is_never_reported_as_ready() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        write_weights(&cache, "def456", "languages/french");
        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::NotInstalled);
        assert!(!statuses[0].present);
        assert!(statuses[0].revision.is_none());
        std::fs::remove_dir_all(cache).ok();
    }

    /// A zero byte weight file is a download that stopped in the middle. The
    /// directory exists and holds the expected name, so a check based on
    /// presence alone would present it as ready and the synthesis would fail
    /// in front of the user.
    #[test]
    fn an_empty_weight_file_is_never_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, FRENCH_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();
        std::fs::write(weights.join("model.safetensors"), b"").unwrap();

        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Corrupted);
        assert!(statuses[0].present);
        assert!(!statuses[0].offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// An empty weights directory proves nothing: it is what an interrupted
    /// extraction leaves behind.
    #[test]
    fn an_empty_weights_directory_is_never_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, FRENCH_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();

        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Corrupted);
        assert!(!statuses[0].offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// Weights organised in subdirectories are complete as soon as their
    /// leaves hold data. A check that only counted the files at the top would
    /// call such a layout empty and refuse a perfectly usable model.
    #[test]
    fn weights_in_a_subdirectory_are_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, FRENCH_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(weights.join("codec")).unwrap();
        std::fs::write(weights.join("codec/model.bin"), b"weights").unwrap();

        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Ready);
        assert!(statuses[0].offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// The Hugging Face client names an unfinished blob `<digest>.incomplete`.
    /// While one is present the model is still downloading, even if a snapshot
    /// directory was already created.
    #[test]
    fn an_unfinished_blob_is_reported_as_downloading() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        let blobs = repository_cache_dir(&cache, FRENCH_REPOSITORY).join("blobs");
        std::fs::create_dir_all(&blobs).unwrap();
        std::fs::write(blobs.join("deadbeef.incomplete"), b"partial").unwrap();

        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Downloading);
        assert!(!statuses[0].offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// A snapshot entry whose blob disappeared is unusable, and following the
    /// link is the only way to notice.
    #[cfg(unix)]
    #[test]
    fn a_dangling_snapshot_link_is_never_reported_as_ready() {
        use std::os::unix::fs::symlink;

        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, FRENCH_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();
        symlink(
            cache.join("blobs/absent"),
            weights.join("model.safetensors"),
        )
        .unwrap();

        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses[0].state, ModelState::Corrupted);
        assert!(!statuses[0].offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }
}
