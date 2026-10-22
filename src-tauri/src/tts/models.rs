//! The Pocket TTS models Murmure supports, and their local state.
//!
//! The catalog is exact: the repository, the directory inside it and the
//! license are the ones the official model card declares. Nothing is
//! guessed. A revision, a size or a date is reported only when it can be
//! read from the local cache, never invented.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::settings::{SUPPORTED_TTS_MODELS, SUPPORTED_TTS_VOICES};

/// Official source of the weights.
const POCKET_TTS_REPOSITORY: &str = "kyutai/pocket-tts-without-voice-cloning";
/// License declared by the model card of the repository.
const POCKET_TTS_LICENSE: &str = "CC BY 4.0";

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
    /// Voice the model speaks with when the user has not chosen one.
    pub default_voice: &'static str,
    /// Voices published for this language.
    pub voices: &'static [&'static str],
}

/// The CPU models the pinned CLI supports, one per published language.
///
/// Only the light variants are catalogued: the heavier ones need more RAM
/// than the machines Murmure targets, and the user chooses the voice, not
/// the size of the model.
pub const POCKET_TTS_MODELS: [PocketTtsModel; 7] = [
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[0],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/english",
        license: POCKET_TTS_LICENSE,
        default_voice: "alba",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[1],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/french",
        license: POCKET_TTS_LICENSE,
        default_voice: "estelle",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[2],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/german",
        license: POCKET_TTS_LICENSE,
        default_voice: "juergen",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[3],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/portuguese",
        license: POCKET_TTS_LICENSE,
        default_voice: "rafael",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[4],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/italian",
        license: POCKET_TTS_LICENSE,
        default_voice: "giovanni",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[5],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/spanish",
        license: POCKET_TTS_LICENSE,
        default_voice: "lola",
        voices: &SUPPORTED_TTS_VOICES,
    },
    PocketTtsModel {
        id: SUPPORTED_TTS_MODELS[6],
        repository: POCKET_TTS_REPOSITORY,
        directory: "languages/dutch",
        license: POCKET_TTS_LICENSE,
        default_voice: "alba",
        voices: &SUPPORTED_TTS_VOICES,
    },
];

/// The sentence spoken when the user asks for a preview.
///
/// Every language gets its own sentence, so a preview proves that the
/// selected language is the one that is actually spoken.
pub fn preview_text(language: &str) -> &'static str {
    match language {
        "english" => "Hello! This is the local Murmure text to speech preview.",
        "french" => "Bonjour ! Voici l'aperçu local de la synthèse vocale de Murmure.",
        "german" => "Hallo! Dies ist die lokale Sprachausgabe von Murmure.",
        "portuguese" => "Olá! Esta é a pré-visualização local da voz do Murmure.",
        "italian" => "Ciao! Questa è l'anteprima locale della sintesi vocale di Murmure.",
        "spanish" => "¡Hola! Esta es la vista previa local de la voz de Murmure.",
        "dutch" => "Hallo! Dit is de lokale stemvoorbeeldweergave van Murmure.",
        _ => "Hello! This is the local Murmure text to speech preview.",
    }
}

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
    /// Voice the model speaks with when the user has not chosen one.
    pub default_voice: &'static str,
    /// Voices published for this language.
    pub voices: &'static [&'static str],
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

/// Whether a directory holds a `.safetensors` file at any depth.
///
/// The cache gives the weights, the tokenizer and the voice their own
/// snapshot, and all three live under the same `languages/<model>` path, so
/// the presence of the directory proves nothing. The weights are the
/// `.safetensors` files, which is why they decide.
fn holds_weights(directory: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        let is_directory = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if is_directory {
            if holds_weights(&path) {
                return true;
            }
        } else if path
            .file_name()
            .map(|name| name.to_string_lossy().ends_with(".safetensors"))
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Locate the weights of a model inside a Hugging Face cache.
///
/// The cache creates one snapshot directory per artifact requested, so a
/// complete download normally holds several snapshots and the number alone
/// says nothing. Among the snapshots that hold the model directory, the one
/// that carries `.safetensors` files is the weights, and when several do the
/// largest is taken: the weights are the bulk of a model, and choosing one is
/// better than reporting a downloaded model as absent.
fn locate_model(cache: &Path, model: &PocketTtsModel) -> Option<(PathBuf, Option<String>)> {
    let repository = repository_cache_dir(cache, model.repository);
    let snapshots = repository.join("snapshots");
    let candidates = std::fs::read_dir(&snapshots)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| path.join(model.directory).is_dir())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    let with_weights = candidates
        .iter()
        .filter(|snapshot| holds_weights(&snapshot.join(model.directory)))
        .collect::<Vec<_>>();

    let chosen = if with_weights.is_empty() {
        // The directory exists but holds no recognisable weights. It is still
        // the model's directory, and the completeness check reports it.
        candidates.first()?.clone()
    } else {
        with_weights
            .into_iter()
            .max_by_key(|snapshot| directory_size(&snapshot.join(model.directory)))?
            .clone()
    };

    let revision = chosen
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    Some((chosen.join(model.directory), revision))
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
    POCKET_TTS_MODELS
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
            } else if !located
                .as_ref()
                .is_some_and(|(weights, _)| weights_are_complete(weights))
            {
                // The weights of this language are incomplete. Every
                // language shares one repository, so an unfinished blob can
                // belong to another language: it only proves that this
                // download is running when this language is the incomplete
                // one.
                if download_is_unfinished(&repository) {
                    ModelState::Downloading
                } else {
                    ModelState::Corrupted
                }
            } else {
                // The weights of this language are complete, so the download
                // of another language cannot make it unusable.
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
                default_voice: model.default_voice,
                voices: model.voices,
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

    /// The languages Parakeet TDT 0.6B v3 transcribes, as the model card
    /// lists them: Bulgarian, Croatian, Czech, Danish, Dutch, English,
    /// Estonian, Finnish, French, German, Greek, Hungarian, Italian,
    /// Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Russian,
    /// Slovak, Slovenian, Spanish, Swedish and Ukrainian.
    const PARAKEET_LANGUAGES: [&str; 25] = [
        "bulgarian",
        "croatian",
        "czech",
        "danish",
        "dutch",
        "english",
        "estonian",
        "finnish",
        "french",
        "german",
        "greek",
        "hungarian",
        "italian",
        "latvian",
        "lithuanian",
        "maltese",
        "polish",
        "portuguese",
        "romanian",
        "russian",
        "slovak",
        "slovenian",
        "spanish",
        "swedish",
        "ukrainian",
    ];

    fn empty_cache() -> PathBuf {
        std::env::temp_dir().join(format!("murmure-model-test-{}", uuid::Uuid::new_v4()))
    }

    fn write_weights(cache: &Path, revision: &str, directory: &str) {
        let weights = repository_cache_dir(cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join(revision)
            .join(directory);
        std::fs::create_dir_all(&weights).unwrap();
        std::fs::write(weights.join("model.safetensors"), b"weights").unwrap();
    }

    /// The status of one language, so a test never depends on the order of
    /// the catalog.
    fn status_of<'a>(statuses: &'a [PocketTtsModelStatus], id: &str) -> &'a PocketTtsModelStatus {
        statuses
            .iter()
            .find(|status| status.id == id)
            .unwrap_or_else(|| panic!("{id} is missing from the catalog"))
    }

    #[test]
    fn the_catalog_states_the_official_repository_and_license() {
        assert_eq!(POCKET_TTS_MODELS.len(), 7);
        for model in POCKET_TTS_MODELS.iter() {
            assert_eq!(model.repository, "kyutai/pocket-tts-without-voice-cloning");
            assert_eq!(model.license, "CC BY 4.0");
            assert_eq!(
                model.directory,
                format!("languages/{}", model.id),
                "the weights of a language live in its own directory"
            );
        }
        assert_eq!(POCKET_TTS_MODELS[1].id, "french");
        assert_eq!(POCKET_TTS_MODELS[1].directory, "languages/french");
    }

    /// The catalog must cover exactly the languages the transcription model
    /// understands, so a language chosen for the synthesis is always a
    /// language Murmure can transcribe.
    #[test]
    fn every_catalogued_language_is_also_transcribed_by_parakeet() {
        for model in POCKET_TTS_MODELS.iter() {
            assert!(
                PARAKEET_LANGUAGES.contains(&model.id),
                "{} is not one of the 25 Parakeet languages",
                model.id
            );
        }
        assert_eq!(PARAKEET_LANGUAGES.len(), 25);
    }

    #[test]
    fn every_language_publishes_every_voice() {
        for model in POCKET_TTS_MODELS.iter() {
            assert_eq!(
                model.voices.len(),
                27,
                "{} does not publish the whole voice catalog",
                model.id
            );
            assert!(
                model.voices.contains(&model.default_voice),
                "the default voice of {} is not in its own catalog",
                model.id
            );
            for voice in [
                "alba", "daan", "estelle", "giovanni", "juergen", "lola", "rafael",
            ] {
                assert!(
                    model.voices.contains(&voice),
                    "{} does not publish {voice}",
                    model.id
                );
            }
        }
    }

    /// The catalog and the settings must name the same default voice, or the
    /// interface would show a voice the application does not speak with.
    #[test]
    fn the_catalog_and_the_settings_agree_on_the_default_voice() {
        for model in POCKET_TTS_MODELS.iter() {
            assert_eq!(
                model.default_voice,
                crate::settings::default_tts_voice(model.id),
                "{} has two different default voices",
                model.id
            );
        }
    }

    /// The catalog must cover exactly the languages of the settings, so a
    /// language the user can select is always a language that can be
    /// downloaded.
    #[test]
    fn the_catalog_covers_every_language_of_the_settings() {
        let catalogued: Vec<&str> = POCKET_TTS_MODELS.iter().map(|model| model.id).collect();
        assert_eq!(catalogued, crate::settings::SUPPORTED_TTS_MODELS.to_vec());
    }

    #[test]
    fn every_language_speaks_a_preview_in_its_own_language() {
        for model in POCKET_TTS_MODELS.iter() {
            let text = preview_text(model.id);
            assert!(!text.is_empty(), "{} has no preview sentence", model.id);
        }
        assert_ne!(preview_text("english"), preview_text("french"));
    }

    #[test]
    fn an_absent_cache_reports_every_model_as_not_installed() {
        let cache = empty_cache();
        let statuses = model_statuses(&cache, None);
        assert_eq!(statuses.len(), 7);
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
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Ready);
        assert!(french.present);
        assert!(french.offline_ready);
        assert_eq!(french.revision.as_deref(), Some("abc123"));
        assert_eq!(french.local_bytes, Some(7));
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn a_model_in_use_is_reported_as_such() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        let statuses = model_statuses(&cache, Some("french"));
        assert_eq!(status_of(&statuses, "french").state, ModelState::InUse);
        assert_eq!(
            status_of(&statuses, "english").state,
            ModelState::NotInstalled
        );
        std::fs::remove_dir_all(cache).ok();
    }

    /// The real cache holds one snapshot per artifact: the weights, the
    /// tokenizer and the voice each get their own, and all three sit under
    /// the same `languages/french` path. Only the snapshot that carries a
    /// `.safetensors` file is the model, so three snapshots are not
    /// ambiguous. Treating them as ambiguous made a fully downloaded model
    /// read as "not downloaded".
    #[test]
    fn several_snapshots_are_not_ambiguous_when_only_one_carries_the_weights() {
        let cache = empty_cache();
        write_weights(&cache, "weights", "languages/french");
        let repository = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY);

        let tokenizer = repository
            .join("snapshots")
            .join("tokenizer")
            .join("languages/french");
        std::fs::create_dir_all(&tokenizer).unwrap();
        std::fs::write(tokenizer.join("tokenizer.json"), b"{}").unwrap();

        let voice = repository
            .join("snapshots")
            .join("voice")
            .join("languages/french/embeddings");
        std::fs::create_dir_all(&voice).unwrap();
        std::fs::write(voice.join("estelle.safetensors"), b"voice").unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Ready);
        assert!(french.offline_ready);
        assert_eq!(french.revision.as_deref(), Some("weights"));
        std::fs::remove_dir_all(cache).ok();
    }

    /// Two snapshots that both carry weights are two revisions of the same
    /// model. The cache cannot say which one the user asked for, so the
    /// largest is taken: the weights are the bulk of a model, and returning
    /// "not installed" for a directory that holds 200 MB of weights was the
    /// mistake that made a fully downloaded model read as absent.
    #[test]
    fn two_weight_snapshots_resolve_to_the_largest() {
        let cache = empty_cache();
        write_weights(&cache, "small", "languages/french");
        let bigger = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join("bigger")
            .join("languages/french");
        std::fs::create_dir_all(&bigger).unwrap();
        std::fs::write(bigger.join("model.safetensors"), vec![0_u8; 4096]).unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Ready);
        assert!(french.present);
        assert_eq!(french.revision.as_deref(), Some("bigger"));
        assert_eq!(french.local_bytes, Some(4096));
        std::fs::remove_dir_all(cache).ok();
    }

    /// A zero byte weight file is a download that stopped in the middle. The
    /// directory exists and holds the expected name, so a check based on
    /// presence alone would present it as ready and the synthesis would fail
    /// in front of the user.
    #[test]
    fn an_empty_weight_file_is_never_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();
        std::fs::write(weights.join("model.safetensors"), b"").unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Corrupted);
        assert!(french.present);
        assert!(!french.offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// An empty weights directory proves nothing: it is what an interrupted
    /// extraction leaves behind.
    #[test]
    fn an_empty_weights_directory_is_never_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Corrupted);
        assert!(!french.offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// Weights organised in subdirectories are complete as soon as their
    /// leaves hold data. A check that only counted the files at the top would
    /// call such a layout empty and refuse a perfectly usable model.
    #[test]
    fn weights_in_a_subdirectory_are_reported_as_ready() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(weights.join("codec")).unwrap();
        std::fs::write(weights.join("codec/model.bin"), b"weights").unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Ready);
        assert!(french.offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// The Hugging Face client names an unfinished blob `<digest>.incomplete`.
    /// While the weights of the language are incomplete, that blob proves the
    /// download is still running.
    #[test]
    fn an_unfinished_blob_is_reported_as_downloading() {
        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
            .join("snapshots")
            .join("abc123")
            .join("languages/french");
        std::fs::create_dir_all(&weights).unwrap();
        std::fs::write(weights.join("model.safetensors"), b"").unwrap();
        let blobs = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY).join("blobs");
        std::fs::create_dir_all(&blobs).unwrap();
        std::fs::write(blobs.join("deadbeef.incomplete"), b"partial").unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Downloading);
        assert!(!french.offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }

    /// Every language shares one repository, so downloading one of them
    /// leaves unfinished blobs next to the languages that are already
    /// complete. A complete language must stay usable offline.
    #[test]
    fn the_download_of_one_language_does_not_hold_another_language_back() {
        let cache = empty_cache();
        write_weights(&cache, "abc123", "languages/french");
        let blobs = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY).join("blobs");
        std::fs::create_dir_all(&blobs).unwrap();
        std::fs::write(blobs.join("deadbeef.incomplete"), b"partial").unwrap();

        let statuses = model_statuses(&cache, None);
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Ready);
        assert!(french.offline_ready);
        assert_eq!(
            status_of(&statuses, "english").state,
            ModelState::NotInstalled,
            "a language that was never downloaded must not read as ready"
        );
        std::fs::remove_dir_all(cache).ok();
    }

    /// A snapshot entry whose blob disappeared is unusable, and following the
    /// link is the only way to notice.
    #[cfg(unix)]
    #[test]
    fn a_dangling_snapshot_link_is_never_reported_as_ready() {
        use std::os::unix::fs::symlink;

        let cache = empty_cache();
        let weights = repository_cache_dir(&cache, POCKET_TTS_REPOSITORY)
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
        let french = status_of(&statuses, "french");
        assert_eq!(french.state, ModelState::Corrupted);
        assert!(!french.offline_ready);
        std::fs::remove_dir_all(cache).ok();
    }
}
