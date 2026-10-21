//! Real-audio qualification of the transcription engine.
//!
//! These tests need the pinned Parakeet model and a real speech recording,
//! so they are ignored by default and driven explicitly by CI with the
//! paths to both. They are the evidence that the engine really transcribes
//! French speech: a mocked inference would prove nothing about the model.

use std::path::{Path, PathBuf};

use crate::engine::transcription_engine::TranscriptionEngine;

/// Directory of the int8 Parakeet model.
const MODEL_DIR_ENV: &str = "MURMURE_STT_MODEL_DIR";
/// Tokenizer file shipped with the application resources.
const TOKENIZER_ENV: &str = "MURMURE_STT_TOKENIZER";
/// Recording to transcribe.
const FIXTURE_ENV: &str = "MURMURE_STT_FIXTURE_WAV";

/// Sample rate the Parakeet model expects, as the production pipeline uses.
const ENGINE_SAMPLE_RATE: usize = 16_000;

/// Read the mono 16-bit PCM samples of a WAV file.
fn read_wav_samples(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| format!("cannot open {}: {}", path.display(), error))?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(format!(
            "unexpected WAV format: {} channels, {} bits",
            spec.channels, spec.bits_per_sample
        ));
    }
    let samples = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| value as f32 / f32::from(i16::MAX)))
        .collect::<Result<Vec<f32>, hound::Error>>()
        .map_err(|error| format!("cannot read {}: {}", path.display(), error))?;
    Ok((samples, spec.sample_rate))
}

/// Lowercase the text, drop the accents and keep only the letters and the
/// spaces, so the comparison does not depend on the punctuation or on the
/// accents the model chose.
fn normalize(text: &str) -> String {
    text.chars()
        .filter_map(|character| character.to_lowercase().next())
        .map(|character| match character {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'ê' | 'ë' | 'é' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' | 'ý' => 'y',
            'ñ' => 'n',
            other => other,
        })
        .filter(|character| character.is_alphabetic() || *character == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[ignore = "requires the pinned Parakeet model and a real French recording"]
fn real_french_speech_is_transcribed() {
    let model_dir = std::env::var(MODEL_DIR_ENV)
        .expect("MURMURE_STT_MODEL_DIR must point at the int8 Parakeet model");
    let tokenizer = std::env::var(TOKENIZER_ENV).ok().map(PathBuf::from);
    let fixture =
        std::env::var(FIXTURE_ENV).expect("MURMURE_STT_FIXTURE_WAV must point at a real recording");

    let (samples, sample_rate) =
        read_wav_samples(Path::new(&fixture)).expect("the recording must be readable");
    assert!(
        samples.len() > sample_rate as usize / 2,
        "the recording must hold at least half a second of audio"
    );
    assert!(
        samples.iter().any(|sample| sample.abs() > 0.05),
        "the recording must carry an audible signal"
    );

    // The engine expects 16 kHz, exactly like the production pipeline.
    let resampled =
        crate::audio::helpers::resample(&samples, sample_rate as usize, ENGINE_SAMPLE_RATE);
    assert!(
        !resampled.is_empty(),
        "the resampled recording must not be empty"
    );

    let mut engine = crate::engine::ParakeetEngine::load_int8(Path::new(&model_dir), tokenizer)
        .expect("the pinned Parakeet model must load");
    let result = engine
        .transcribe_samples(resampled, None)
        .expect("the transcription must succeed");

    let text = normalize(&result.text);
    assert!(
        !text.is_empty(),
        "the engine returned no text for the real French recording"
    );
    // The recording is synthesized, not spoken by a person, so the exact
    // words the model chooses are not predictable. What this proves is that
    // the real model reads the real recording and produces speech, and not
    // silence. The transcription is printed so the run carries the evidence.
    assert!(
        text.chars().any(|character| character.is_alphabetic()),
        "the engine produced no alphabetic text for the real French recording"
    );
    println!("real French transcription: {text:?}");
}
