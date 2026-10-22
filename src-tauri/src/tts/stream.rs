//! Playback source fed by the streamed WAV of the resident Pocket TTS server.
//!
//! The server sends a plain PCM WAV whose header arrives first. The header is
//! parsed here, then a producer thread converts the 16-bit samples into the
//! `f32` blocks Rodio consumes, so the first sound is heard while the rest of
//! the sentence is still being generated.

use std::collections::VecDeque;
use std::io::Read;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// How many decoded blocks the producer may keep ahead of playback. At
/// 24 kHz a block holds about 170 ms, so the queue holds a few seconds of
/// audio and absorbs a generation that is momentarily slower than playback.
const QUEUE_BLOCKS: usize = 32;
const BLOCK_BYTES: usize = 8_192;
/// Audio buffered before the first sample is handed to the mixer. Without
/// this lead a generation slower than real time turns into audible gaps.
const LEAD_MILLIS: u64 = 1_500;
/// A short phrase never fills the lead, so the wait is bounded.
const LEAD_TIMEOUT: Duration = Duration::from_millis(2_000);

/// The only layout the pinned runtime produces. Anything else is refused
/// instead of being played as noise.
const PCM_FORMAT: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;

/// Counters that explain a stutter without ever touching the audio content:
/// how many samples were handed over, and how many of them were silence
/// emitted while the producer had nothing ready.
#[derive(Default)]
pub(super) struct StreamStats {
    samples: AtomicU64,
    starved: AtomicU64,
    finished: AtomicBool,
}

impl StreamStats {
    pub(super) fn samples(&self) -> u64 {
        self.samples.load(Ordering::Relaxed)
    }

    pub(super) fn starved(&self) -> u64 {
        self.starved.load(Ordering::Relaxed)
    }

    /// Whether the producer reached the end of the body. A server that is
    /// still producing has work to abandon; one that finished has nothing
    /// left to stop.
    pub(super) fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

pub(super) struct PcmStream {
    receiver: Receiver<Vec<f32>>,
    prefetched: VecDeque<Vec<f32>>,
    pending: std::vec::IntoIter<f32>,
    channels: NonZeroU16,
    sample_rate: NonZeroU32,
    failure: Arc<AtomicBool>,
    stats: Arc<StreamStats>,
}

impl PcmStream {
    /// Parse the WAV header of `response` and start converting the body.
    pub(super) fn open(
        mut response: Box<dyn Read + Send>,
        limit: Duration,
    ) -> Result<Self, String> {
        let (channels, sample_rate) = read_wave_header(&mut response)?;
        let (sender, receiver) = mpsc::sync_channel::<Vec<f32>>(QUEUE_BLOCKS);
        let failure = Arc::new(AtomicBool::new(false));
        let producer_failure = Arc::clone(&failure);
        let stats = Arc::new(StreamStats::default());
        let producer_stats = Arc::clone(&stats);
        thread::spawn(move || {
            produce(
                response,
                sender,
                producer_failure,
                producer_stats,
                limit,
                sample_rate.get(),
                channels.get(),
            );
        });
        Ok(Self {
            receiver,
            prefetched: VecDeque::new(),
            pending: Vec::new().into_iter(),
            channels,
            sample_rate,
            failure,
            stats,
        })
    }

    /// Handle on the same flag, kept by the player so a truncated generation
    /// is still reported after the playback loop returns.
    pub(super) fn failure_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.failure)
    }

    /// Counters shared with the playback loop, read once the audio ended.
    pub(super) fn stats(&self) -> Arc<StreamStats> {
        Arc::clone(&self.stats)
    }

    /// Read the whole audio before playback.
    ///
    /// Used when the machine cannot generate faster than real time: waiting
    /// once before the first sound is better than audible gaps throughout,
    /// and the model is already loaded so the wait is only the generation.
    pub(super) fn collect(mut self) -> (Vec<f32>, bool) {
        let mut samples = Vec::new();
        loop {
            if let Some(sample) = self.pending.next() {
                samples.push(sample);
                continue;
            }
            if let Some(block) = self.prefetched.pop_front() {
                self.pending = block.into_iter();
                continue;
            }
            match self.receiver.recv() {
                Ok(block) => self.pending = block.into_iter(),
                Err(_) => break,
            }
        }
        let failed = self.failure.load(Ordering::Acquire);
        (samples, failed)
    }

    /// Buffer a lead of audio before the first sample reaches the mixer, so a
    /// generation that is briefly slower than playback cannot open a gap.
    ///
    /// This runs on the caller's thread: the mixer callback must never block,
    /// otherwise the device buffer runs dry and the very first samples click.
    /// The wait stops as soon as the stream ends, which is what a short
    /// phrase does: it arrives entirely and plays without the extra lead.
    pub(super) fn prime(&mut self) {
        let channels = u64::from(self.channels.get().max(1));
        let target = u64::from(self.sample_rate.get()) * LEAD_MILLIS / 1_000;
        let deadline = Instant::now() + LEAD_TIMEOUT;
        let mut queued = 0_u64;
        while queued < target && Instant::now() < deadline {
            match self.receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(block) => {
                    queued += block.len() as u64 / channels;
                    self.prefetched.push_back(block);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

impl Iterator for PcmStream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(sample) = self.pending.next() {
            self.stats.samples.fetch_add(1, Ordering::Relaxed);
            return Some(sample);
        }
        if let Some(block) = self.prefetched.pop_front() {
            self.pending = block.into_iter();
            return self.next();
        }
        match self.receiver.try_recv() {
            Ok(block) => {
                self.pending = block.into_iter();
                self.next()
            }
            // Starving must not block the mixer thread, which would stall
            // every other sound of the application: the gap is played as
            // silence until the next block or the end of the stream, and it
            // is counted so the cause of a stutter can be established.
            Err(TryRecvError::Empty) => {
                self.stats.samples.fetch_add(1, Ordering::Relaxed);
                self.stats.starved.fetch_add(1, Ordering::Relaxed);
                Some(0.0)
            }
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

impl rodio::Source for PcmStream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn produce(
    mut response: Box<dyn Read + Send>,
    sender: SyncSender<Vec<f32>>,
    failure: Arc<AtomicBool>,
    stats: Arc<StreamStats>,
    limit: Duration,
    sample_rate: u32,
    channels: u16,
) {
    // Every exit path, including a cancellation, must publish that the
    // producer is done so the caller knows no server work remains.
    struct Finished(Arc<StreamStats>);
    impl Drop for Finished {
        fn drop(&mut self) {
            self.0.finished.store(true, Ordering::Release);
        }
    }
    let _finished = Finished(stats);

    let started = Instant::now();
    let max_frames = u64::from(sample_rate) * limit.as_secs().max(1);
    let mut block = vec![0_u8; BLOCK_BYTES];
    let mut pending_byte: Option<u8> = None;
    let mut frames = 0_u64;
    loop {
        if started.elapsed() >= limit {
            failure.store(true, Ordering::Release);
            return;
        }
        let read = match response.read(&mut block) {
            // The runtime fills the announced data size with a placeholder
            // while it streams, so the transfer framing, not that size, ends
            // the audio. A cut connection is reported by the framing instead.
            Ok(0) => return,
            Ok(read) => read,
            Err(_) => {
                failure.store(true, Ordering::Release);
                return;
            }
        };
        let mut samples = Vec::with_capacity(read / 2 + 1);
        let mut index = 0;
        if let Some(low) = pending_byte.take() {
            samples.push(pcm_to_f32(low, block[0]));
            index = 1;
        }
        while index + 1 < read {
            samples.push(pcm_to_f32(block[index], block[index + 1]));
            index += 2;
        }
        if index < read {
            pending_byte = Some(block[index]);
        }
        // A frame carries one sample per channel; the bound applies to the
        // produced audio, not to the bytes received.
        frames = frames.saturating_add((samples.len() / usize::from(channels.max(1))) as u64);
        if frames > max_frames {
            failure.store(true, Ordering::Release);
            return;
        }
        if !samples.is_empty() && sender.send(samples).is_err() {
            return;
        }
    }
}

fn pcm_to_f32(low: u8, high: u8) -> f32 {
    f32::from(i16::from_le_bytes([low, high])) / 32_768.0
}

fn read_wave_header(reader: &mut dyn Read) -> Result<(NonZeroU16, NonZeroU32), String> {
    let mut header = [0_u8; 12];
    reader
        .read_exact(&mut header)
        .map_err(|_| "Pocket TTS produced no WAV".to_string())?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("Pocket TTS produced a file that is not a WAV".to_string());
    }

    let mut channels = None;
    let mut sample_rate = None;
    loop {
        let mut chunk = [0_u8; 8];
        reader
            .read_exact(&mut chunk)
            .map_err(|_| "Pocket TTS produced a truncated WAV".to_string())?;
        let id = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        match &id {
            b"fmt " => {
                if size < 16 {
                    return Err("Pocket TTS produced an invalid WAV header".to_string());
                }
                let mut format = [0_u8; 16];
                reader
                    .read_exact(&mut format)
                    .map_err(|_| "Pocket TTS produced a truncated WAV".to_string())?;
                let audio_format = u16::from_le_bytes([format[0], format[1]]);
                let channel_count = u16::from_le_bytes([format[2], format[3]]);
                let rate = u32::from_le_bytes([format[4], format[5], format[6], format[7]]);
                let bits = u16::from_le_bytes([format[14], format[15]]);
                if audio_format != PCM_FORMAT || bits != BITS_PER_SAMPLE {
                    return Err("Pocket TTS produced a WAV Murmure cannot stream".to_string());
                }
                channels = NonZeroU16::new(channel_count);
                sample_rate = NonZeroU32::new(rate);
                skip(reader, u64::from(size) - 16)?;
            }
            b"data" => {
                let channels = channels
                    .ok_or_else(|| "Pocket TTS produced a WAV without a format".to_string())?;
                let sample_rate = sample_rate
                    .ok_or_else(|| "Pocket TTS produced a WAV without a sample rate".to_string())?;
                // The announced size is not trusted: while it streams, the
                // runtime writes a placeholder there, so the transfer framing
                // is what ends the audio.
                return Ok((channels, sample_rate));
            }
            _ => skip(reader, u64::from(size) + u64::from(size % 2))?,
        }
    }
}

fn skip(reader: &mut dyn Read, mut bytes: u64) -> Result<(), String> {
    let mut buffer = [0_u8; 512];
    while bytes > 0 {
        let wanted = usize::try_from(bytes)
            .map(|left| left.min(buffer.len()))
            .unwrap_or(buffer.len());
        let read = reader
            .read(&mut buffer[..wanted])
            .map_err(|_| "Pocket TTS produced a truncated WAV".to_string())?;
        if read == 0 {
            return Err("Pocket TTS produced a truncated WAV".to_string());
        }
        bytes -= read as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;
    use std::io::Cursor;

    fn wave(
        channels: u16,
        rate: u32,
        samples: &[i16],
        extra: Option<(&[u8; 4], &[u8])>,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&16_u32.to_le_bytes());
        body.extend_from_slice(&1_u16.to_le_bytes());
        body.extend_from_slice(&channels.to_le_bytes());
        body.extend_from_slice(&rate.to_le_bytes());
        body.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        body.extend_from_slice(&(channels * 2).to_le_bytes());
        body.extend_from_slice(&16_u16.to_le_bytes());
        if let Some((id, payload)) = extra {
            body.extend_from_slice(id);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                body.push(0);
            }
        }
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(&data);
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&body);
        file
    }

    fn open(file: Vec<u8>) -> Result<PcmStream, String> {
        PcmStream::open(Box::new(Cursor::new(file)), Duration::from_secs(30))
    }

    /// The producer runs on its own thread, so a test waits for the audio it
    /// announces instead of assuming the queue is already filled.
    fn audio(source: &mut PcmStream, expected: usize) -> Vec<f32> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut collected = Vec::new();
        while collected.len() < expected && Instant::now() < deadline {
            match source.next() {
                Some(sample) if sample != 0.0 => collected.push(sample),
                Some(_) => {}
                None => break,
            }
        }
        collected
    }

    #[test]
    fn the_samples_of_a_mono_wave_are_scaled_to_the_rodio_range() {
        let mut source = open(wave(1, 24_000, &[16_384, -16_384, 32_767], None)).unwrap();
        assert_eq!(source.channels().get(), 1);
        assert_eq!(source.sample_rate().get(), 24_000);
        let samples = audio(&mut source, 3);
        assert!((samples[0] - 0.5).abs() < 0.001);
        assert!((samples[1] + 0.5).abs() < 0.001);
        assert!(samples[2] > 0.99);
    }

    #[test]
    fn chunks_before_the_audio_data_are_skipped() {
        let mut source = open(wave(1, 24_000, &[32_767], Some((b"LIST", b"abc")))).unwrap();
        let samples = audio(&mut source, 1);
        assert_eq!(samples.len(), 1);
        assert!(samples[0] > 0.99);
    }

    #[test]
    fn a_wave_without_audio_data_is_refused() {
        // The RIFF header and the format chunk are complete, but the audio
        // chunk never starts: the stream must be refused, not played.
        let mut truncated = wave(1, 24_000, &[1, 2], None);
        truncated.truncate(36);
        assert!(open(truncated).is_err());
    }

    #[test]
    fn a_compressed_wave_is_refused_instead_of_played_as_noise() {
        let mut file = wave(1, 24_000, &[1, 2], None);
        // The format tag sits 20 bytes into the RIFF header.
        file[20] = 3;
        assert!(open(file).is_err());
    }

    /// A connection cut in the middle of the audio must stop the playback and
    /// be reported, never be mistaken for a finished sentence.
    #[test]
    fn a_broken_body_is_reported_as_an_interrupted_generation() {
        struct Broken {
            data: Vec<u8>,
            position: usize,
            budget: usize,
        }

        impl Read for Broken {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.position >= self.budget {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "the stream was cut",
                    ));
                }
                let available = self.budget - self.position;
                let taken = buffer.len().min(available);
                buffer[..taken].copy_from_slice(&self.data[self.position..self.position + taken]);
                self.position += taken;
                Ok(taken)
            }
        }

        let file = wave(1, 24_000, &[20_000; 4_000], None);
        let budget = file.len() - 2_000;
        let mut source = PcmStream::open(
            Box::new(Broken {
                data: file,
                position: 0,
                budget,
            }),
            Duration::from_secs(30),
        )
        .unwrap();
        let collected = audio(&mut source, 10);
        while source.next().is_some() {}
        assert!(!collected.is_empty());
        assert!(source.failure_flag().load(Ordering::Acquire));
    }

    #[test]
    fn an_oversized_generation_is_stopped_by_its_bound() {
        let samples = vec![1_i16; 96_000];
        let mut source = PcmStream::open(
            Box::new(Cursor::new(wave(1, 24_000, &samples, None))),
            Duration::from_millis(0),
        )
        .unwrap();
        while source.next().is_some() {}
        assert!(source.failure_flag().load(Ordering::Acquire));
    }
}

#[cfg(all(test, target_os = "windows"))]
mod real_runtime_tests {
    use super::*;
    use rodio::Source;
    use std::path::PathBuf;

    /// Prove the resident server path end to end against the installed
    /// runtime: the model is loaded once, the phrase is streamed, and the
    /// samples reach Rodio without a temporary WAV on disk.
    ///
    /// Runs only on a machine where the sidecar and the model of the
    /// selected language are installed, so it stays `ignored` in CI:
    /// `cargo test --lib -- --ignored the_resident_server_streams`.
    #[test]
    #[ignore = "requires the installed Pocket TTS runtime and the French model"]
    fn the_resident_server_streams_a_real_french_wave() {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .expect("LOCALAPPDATA is available on Windows");
        let root = local.join("com.al1x-ai.murmure").join("pocket-tts");
        let binary = root.join("bin").join("pocket-tts.exe");
        let cache = root.join("models");
        assert!(
            binary.is_file(),
            "the runtime is not installed at {}",
            binary.display()
        );
        assert!(
            cache.is_dir(),
            "the model cache is missing at {}",
            cache.display()
        );

        let port =
            super::super::serve::ensure_running(&binary, "french", "estelle", &cache).unwrap();
        let body =
            super::super::serve::request_speech(port, "Bonjour, ceci est un essai rÃ©el.").unwrap();
        let mut source = PcmStream::open(body, Duration::from_secs(120)).unwrap();
        assert_eq!(source.sample_rate().get(), 24_000);
        assert_eq!(source.channels().get(), 1);

        let deadline = Instant::now() + Duration::from_secs(120);
        // Starvation is played as silence, so the audio is measured on the
        // samples that actually carry a signal.
        let mut audible = 0_usize;
        let mut peak = 0_f32;
        while Instant::now() < deadline {
            match source.next() {
                Some(sample) => {
                    if sample != 0.0 {
                        audible += 1;
                    }
                    peak = peak.max(sample.abs());
                }
                None => break,
            }
        }
        super::super::serve::shutdown();
        assert!(!source.failure_flag().load(Ordering::Acquire));
        assert!(
            peak > 0.01,
            "the streamed French WAV carried no signal, peak {peak}"
        );
        assert!(
            audible > 24_000,
            "the streamed French WAV held only {audible} audible samples"
        );
        assert!(
            audible < 24_000 * 30,
            "the streamed French WAV held {audible} audible samples"
        );
    }
}
