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

/// Describe the local state of every supported model.
///
/// `in_use` lists the model currently being synthesized, if any.
pub fn model_statuses(cache: &Path, in_use: Option<&str>) -> Vec<PocketTtsModelStatus> {
    FRENCH_MODELS
        .iter()
        .map(|model| {
            let located = locate_model(cache, model);
            let present = located.is_some();
            let (revision, local_bytes) = match located {
                Some((weights, revision)) => {
                    let bytes = directory_size(&weights);
                    (revision, (bytes > 0).then_some(bytes))
                }
                None => (None, None),
            };
            let state = if !present {
                ModelState::NotInstalled
            } else if in_use == Some(model.id) {
                ModelState::InUse
            } else {
                ModelState::Ready
            };
            PocketTtsModelStatus {
                id: model.id,
                repository: model.repository,
                directory: model.directory,
                license: model.license,
                state,
                present,
                offline_ready: present,
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
}
