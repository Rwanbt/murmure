use crate::audio::chunking::ChunkPipeline;
use crate::audio::recorder::AudioRecorder;
use crate::engine::ParakeetEngine;
use cpal::Device;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct MicInfo {
    pub id: String,
    pub label: String,
}

#[derive(Default)]
pub struct PreviewSnapshot {
    pub queue: Vec<f32>,
    pub generation: u64,
    pub revision: u64,
}

pub struct AudioState {
    pub recorder: Mutex<Option<AudioRecorder>>,
    pub engine: Mutex<Option<ParakeetEngine>>,
    pub current_file_name: Mutex<Option<String>>,
    recording_mode: AtomicU8,
    recording_trigger: AtomicU8,
    /// Cached audio input device to avoid re-enumerating devices on each recording
    pub cached_device: Mutex<Option<Device>>,
    /// Wake word to strip from the end of the transcription (set by validate trigger)
    pub strip_word: Mutex<Option<String>>,
    pub streaming_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub streaming_stop: Arc<AtomicBool>,
    pub preview_snapshot: Arc<Mutex<PreviewSnapshot>>,
    pub chunk_inference_active: Arc<AtomicBool>,
    /// The chunking pipeline of the active session
    pub chunk_pipeline: Mutex<Option<ChunkPipeline>>,
    session_gen: AtomicU64,
    /// Spans the whole session, from the record request to the end of
    /// post-processing. Unlike `recorder`, reading it never blocks on the
    /// microphone init and it stays true while the LLM finalizes.
    session_active: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingMode {
    Standard = 0,
    Llm = 1,
    Command = 2,
}

impl RecordingMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            RecordingMode::Standard => "standard",
            RecordingMode::Llm => "llm",
            RecordingMode::Command => "command",
        }
    }
}

impl From<u8> for RecordingMode {
    fn from(val: u8) -> Self {
        match val {
            0 => RecordingMode::Standard,
            1 => RecordingMode::Llm,
            2 => RecordingMode::Command,
            _ => RecordingMode::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingTrigger {
    Keyboard = 0,
    WakeWord = 1,
}

impl From<u8> for RecordingTrigger {
    fn from(val: u8) -> Self {
        match val {
            1 => RecordingTrigger::WakeWord,
            _ => RecordingTrigger::Keyboard,
        }
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            recorder: Mutex::new(None),
            engine: Mutex::new(None),
            current_file_name: Mutex::new(None),
            recording_mode: AtomicU8::new(RecordingMode::Standard as u8),
            recording_trigger: AtomicU8::new(RecordingTrigger::Keyboard as u8),
            cached_device: Mutex::new(None),
            strip_word: Mutex::new(None),
            streaming_handle: Mutex::new(None),
            streaming_stop: Arc::new(AtomicBool::new(false)),
            preview_snapshot: Arc::new(Mutex::new(PreviewSnapshot::default())),
            chunk_inference_active: Arc::new(AtomicBool::new(false)),
            chunk_pipeline: Mutex::new(None),
            session_gen: AtomicU64::new(0),
            session_active: AtomicBool::new(false),
        }
    }

    pub fn begin_session(&self) -> u64 {
        self.session_gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Claim the session for a new recording.
    ///
    /// The claim is atomic, so two rapid triggers cannot both start. The
    /// generation is only advanced by the caller that wins the claim: a
    /// rejected trigger must not invalidate the running session, and it
    /// must not replace the chunk pipeline, because that would discard the
    /// audio already captured and leave the user with an empty result.
    pub fn claim_session(&self) -> Result<u64, SessionBusy> {
        self.session_active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| SessionBusy)?;
        Ok(self.begin_session())
    }

    pub fn invalidate_session(&self) {
        self.session_gen.fetch_add(1, Ordering::SeqCst);
    }

    pub fn current_session(&self) -> u64 {
        self.session_gen.load(Ordering::SeqCst)
    }

    pub fn set_session_active(&self, active: bool) {
        self.session_active.store(active, Ordering::SeqCst);
    }

    pub fn is_session_active(&self) -> bool {
        self.session_active.load(Ordering::SeqCst)
    }

    pub fn set_recording_mode(&self, mode: RecordingMode) {
        self.recording_mode.store(mode as u8, Ordering::SeqCst);
    }

    pub fn get_recording_mode(&self) -> RecordingMode {
        self.recording_mode.load(Ordering::SeqCst).into()
    }

    pub fn set_recording_trigger(&self, trigger: RecordingTrigger) {
        self.recording_trigger
            .store(trigger as u8, Ordering::SeqCst);
    }

    pub fn get_recording_trigger(&self) -> RecordingTrigger {
        self.recording_trigger.load(Ordering::SeqCst).into()
    }

    /// Sets the cached audio device
    pub fn set_cached_device(&self, device: Option<Device>) {
        *self.cached_device.lock() = device;
    }

    /// Gets a clone of the cached audio device
    pub fn get_cached_device(&self) -> Option<Device> {
        self.cached_device.lock().clone()
    }
}

pub(super) enum RecorderStartError {
    Busy,
    DirUnavailable,
    InitFailed,
    StartFailed,
    Superseded,
}

/// A recording session is already running, so a new trigger is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionBusy;

impl std::fmt::Display for SessionBusy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a recording session is already running")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_trigger_cannot_claim_a_running_session() {
        let state = AudioState::new();
        let first = state
            .claim_session()
            .expect("the first trigger must win the claim");
        assert!(state.is_session_active());
        assert_eq!(state.claim_session(), Err(SessionBusy));
        // The refused trigger must not invalidate the running session and
        // must not steal the session flag.
        assert_eq!(state.current_session(), first);
        assert!(state.is_session_active());
    }

    #[test]
    fn a_finished_session_can_be_claimed_again() {
        let state = AudioState::new();
        let first = state.claim_session().unwrap();
        state.set_session_active(false);
        let second = state.claim_session().unwrap();
        assert!(second > first);
        assert!(state.is_session_active());
    }

    #[test]
    fn exactly_one_of_two_concurrent_claims_wins() {
        let state = std::sync::Arc::new(AudioState::new());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let state = std::sync::Arc::clone(&state);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    state.claim_session().is_ok()
                })
            })
            .collect();
        let winners = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|won| *won)
            .count();
        assert_eq!(
            winners, 1,
            "only one concurrent trigger may start a recording"
        );
    }

    #[test]
    fn invalidating_a_session_advances_the_generation() {
        let state = AudioState::new();
        let first = state.begin_session();
        state.invalidate_session();
        assert!(state.current_session() > first);
    }
}
