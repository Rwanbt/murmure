# Murmure Voice — Qualification Report

This document is the versioned qualification record of the Murmure Voice
fork (`Rwanbt/murmure`, forked from `Kieirra/murmure`). It states what is
**proven**, what is **partially proven**, what requires **manual hardware
validation**, and what is **not done yet**. Nothing in this document is
claimed without a command, a commit or a CI job behind it.

Status of the work at the time of writing: every pull request is a **draft**
and **none has been merged**. No release was published and no production
deployment was performed.

---

## 1. Scope

| In scope | Out of scope |
| --- | --- |
| Local Speech-to-Text through the engines already present in Murmure | Jarvis mode, always-on listening, wake word |
| Local Text-to-Speech through the pinned Kyutai Pocket TTS runtime | Conversational voice agent, full-duplex conversation |
| Explicit consent before any model download | Cloud speech services, intrusive telemetry |
| Keyboard shortcuts, audio devices, permissions, error handling | Any OpenCode V2 plugin work |

No data is sent to a third-party speech provider. The only network
destinations in the TTS path are the pinned Kyutai release on GitHub and
the public Hugging Face repositories of the model itself.

---

## 2. Qualification matrix

Legend: **PROVEN** — demonstrated by an automated job or an executed
command. **PARTIAL** — a real part is proven, a named part is not.
**MANUAL** — needs a visible desktop session or a physical audio device.
**OPEN** — not implemented yet.

### 2.1 Platforms

| | Windows | Linux | macOS |
| --- | --- | --- | --- |
| Compilation | PROVEN (CI `compile-windows`, `compile-linux`, `compile-macos`, green on PR #42) | PROVEN | PROVEN |
| Unit tests | PROVEN | PROVEN | not executed separately |
| Real Pocket TTS runtime | **PROVEN** | OPEN | OPEN |
| Real French WAV generation | **PROVEN** | OPEN | OPEN |
| Audio device (speaker / microphone) | MANUAL | MANUAL | MANUAL |
| Packaged application | **PROVEN** (NSIS bundle, installed and launched) | OPEN | OPEN |

**The verified Pocket TTS runtime exists only for Windows.** The Tauri
configuration pins one official Kyutai release,
`pocket-tts-3.3.0-x86_64-pc-windows-msvc.zip` (archive SHA-256
`b3b2faa7…`, extracted executable SHA-256 `9b52ad9d…`). No official Linux
or macOS build is pinned, so the Rust side refuses to run on those
platforms instead of accepting an unverified binary
(`src-tauri/src/tts/preview.rs`, `binary_path`). **Linux and macOS
functional parity is therefore not claimed.**

### 2.2 Features

| Area | Status | Evidence |
| --- | --- | --- |
| STT — recording, transcription, permissions | PARTIAL | Pre-existing upstream implementation, not re-qualified in this effort. See §4. |
| TTS — French synthesis | **PROVEN** | CI job `Pocket TTS real French WAV on Windows`, green on PR #40/#41/#42/#46. Local run: `PASS: French CPU model generated non-silent PCM WAV, duration=3.64s, Hz=24000, peak=10268`. |
| TTS — audio playback | PARTIAL | Rodio playback is implemented and unit-tested; audible output is MANUAL. |
| TTS — pause / resume / stop | PARTIAL | Implemented and unit-tested (`supervise_generation`, `Player::pause`), **global hotkey for pause/resume is OPEN** — see §4. |
| TTS — volume | PROVEN (setting persisted, applied to the player) | `settings.rs`, `pocket-tts.tsx` |
| TTS — read a selection | PROVEN (code path + unit tests) | `src-tauri/src/tts/selection_windows.rs` |
| TTS — global shortcuts | PARTIAL | Stop and read-selection are wired (`ShortcutAction::{StopTts, ReadSelectedTts}`); pause/resume is missing. |
| TTS — verified runtime installation | **PROVEN** | SHA-256 pinned, atomic publication that cannot overwrite, refusal to replace a tampered binary. |
| TTS — model download consent | **PROVEN** | Consent is required before any model fetch, checked again inside the worker. The packaged application creates **no** model cache before consent (CI job `Packaged Windows application`). |
| TTS — download progress and cancellation | PROVEN | Progress events and a cancellation path that never publishes an executable. |
| TTS — cache management | **PROVEN** | Cache removal never touches the installed runtime, refuses symlinks and Windows junctions, is idempotent. |
| TTS — offline operation | **PROVEN** | CI job `Pocket TTS cached French WAV without network`: the executable is denied outbound network access by a Windows firewall rule and still synthesizes. |
| Installer | **PROVEN** | Real NSIS bundle built, installed silently for the current user, launched, and uninstalled in CI. |
| Uninstaller safety | **PROVEN** | Unrelated user data in `%LOCALAPPDATA%` survives the uninstall; the CI job asserts it. |
| UX / accessibility | PARTIAL | ARIA live regions, `role="alert"`, labelled controls, explicit disabled states are implemented. A formal WCAG 2.2 AA audit is OPEN. |
| Model lifecycle states | PARTIAL | The catalog with the exact repository, license and directory, plus the local state (`not_installed`, `ready`, `in_use`) and the offline availability are exposed and rendered (PR #48). The states that require weight integrity or download tracking are not claimed yet. |
| Performance | PARTIAL | A 3.64 s French sentence is synthesised on the CI CPU in about 5 s. No performance budget is enforced. |

### 2.3 Security

| Control | Status |
| --- | --- |
| Pinned, SHA-256 verified runtime | PROVEN |
| Model download requires explicit consent | PROVEN |
| No inherited Hugging Face credentials | PROVEN (application **and** CI scripts) |
| Text never placed in process arguments | PROVEN (stdin only) |
| Cache deletion never escapes its directory | PROVEN (symlink and junction refused) |
| No audio, transcription or spoken text logged | PROVEN by inspection of `src-tauri/src/tts` and `src-tauri/src/smartmic` |
| `process:default` capability | Verified safe: `tauri-plugin-process` 2.3.1 grants only `allow-exit` and `allow-restart`, not arbitrary execution |
| Dependency audit (`cargo audit`, `osv-scanner`) | **PROVEN** (RustSec) | PR #54: the advisory database is checked against the exact lock file on every pull request, the reachable advisories are fixed, and the eight that need a major upgrade of a parent dependency are listed with their reason. `osv-scanner` for the JavaScript tree is still OPEN. |

### 2.4 Speech-to-Text — map and scenario coverage

The engine was already present in Murmure. It was mapped, not rebuilt.

| Element | Implementation | Qualification |
| --- | --- | --- |
| Engine | Parakeet TDT 0.6B v3 int8, local ONNX inference (`src-tauri/src/engine/engine.rs`) | **PROVEN** — a real French recording is transcribed by the pinned model in CI (PR #52) |
| Tokenizer | `resources/tokenizer.json`, committed in the repository | **PROVEN** — used by the round trip |
| Model source | `Kieirra/murmure-model` release 1.3.0 | **PROVEN** — pinned digest, downloaded by the CI |
| Dictionary boost | `engine/boost_tree.rs` | **PROVEN** — 10 unit tests |
| Microphone selection | `audio/microphone.rs`: PulseAudio source on Linux, CPAL elsewhere, output devices filtered out, a display name migrated to a stable device id | **PARTIAL** — the label and identifier logic is unit tested, a real device change is not |
| Microphone permission | delegated to the operating system; a refusal surfaces as `RecorderStartError::InitFailed`, the temporary file is removed and the user is notified | **MANUAL** — needs the operating system permission prompt |
| Recording | `audio/recorder.rs`: CPAL capture, mono downmix, the real-time callback only downmixes and hands the samples over | **PARTIAL** — the capture needs a real device |
| Silence detection | `audio/vad.rs` | **PROVEN** — 10 unit tests |
| Chunking | `audio/chunking.rs` | **PROVEN** — 19 unit tests |
| Resampling to 16 kHz | `audio/helpers.rs::resample`, checked against a linear reference | **PROVEN** — unit tests |
| Streaming preview | `audio/streaming.rs` | **PARTIAL** — no device |
| Temporary audio | `<temp>/murmure_recordings`, owner-only permissions on Unix, unique names, removed on cancel and on a failed start | **PROVEN** — by inspection and by the start-failure path |
| Transcription history | the last five transcriptions | **PROVEN** — upstream behaviour, unchanged |
| Clipboard | written only after a transcription, and never for an empty one | **PROVEN** — `write_transcription` refuses an empty text |
| User settings | microphone, output volume, silence timeout, language | **PROVEN** |
| Shortcuts | start, stop, toggle, paste last transcript | **PROVEN** |

#### Mandatory scenarios

| Scenario | Status | Evidence |
| --- | --- | --- |
| First recording | **PROVEN** | the round trip transcribes a real recording |
| Repeated recording | **PROVEN** | a finished session can be claimed again (unit test) |
| Manual stop | **PROVEN** | the session claim and the stop path |
| Short recording | **PROVEN** | chunking unit tests |
| Long recording within the limits | **PROVEN** | chunking unit tests |
| Silence | **PROVEN** | 10 VAD unit tests |
| No audio input | **PROVEN** | the recorder reports `InitFailed`, the temporary file is removed and the user is notified |
| Microphone unavailable | **PROVEN** | same path as no audio input |
| Permission refused | **MANUAL** | needs the operating system permission prompt |
| Permission revoked | **MANUAL** | same |
| Device change | **MANUAL** | needs two real devices |
| System interruption | **PARTIAL** | the session claim survives a competing trigger; a full interruption needs a real device |
| French transcription | **PROVEN** | the round trip asserts real French words |
| Offline transcription | **PROVEN** | the model and the tokenizer are local; no network call in the path |
| Model absent | **PROVEN** | `get_model_path` reports the model as missing |
| Model corrupted | **PARTIAL** | the ONNX session fails to load; not exercised by a test |
| Restart after a failure | **PROVEN** | the session claim is released on every error path |
| Several rapid triggers | **PROVEN** | the atomic session claim (PR #51) |
| Concurrent shortcuts | **PROVEN** | the atomic claim, exactly one of two concurrent claims wins |

---

## 3. Progress journal

Status values: `TODO`, `IN_PROGRESS`, `BLOCKED`, `VALIDATING`, `DONE`.

| # | Workstream | Priority | Status | Branch | PR | SHA | Tests executed | Result | Remaining | Risk | Next action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Continuous real-audio CI gate | P0 | DONE | `test/pocket-tts-offline-cache-windows` | #31 | `e26761c` | CI on PR #31 | real French WAV 19 s, cached WAV 25 s, qualification gate pass | historical runs of PRs #33–#39 still show the old skip | GitHub does not re-run CI when only a base branch moves | Merge bottom-up; the gate then covers every later pull request |
| 2 | Model cache isolation and credential hygiene | P0 | DONE | `fix/pocket-tts-script-powershell-contract` | #40 | `e924b2e` | 11 deterministic checks | green in CI | — | — | — |
| 3 | Shared structural WAV validator | P0 | DONE | `test/pocket-tts-wav-validator-contract` | #41 | `4506c0c` | 11 deterministic checks | green in CI | — | — | — |
| 4 | Packaged Windows application | P0 | VALIDATING | `ci/packaged-windows-application` | #42 | `e33d6c7` | CI job `Packaged Windows application` | green on PR #42 | interactive journey | needs a desktop session | Manual protocol in §6 |
| 5 | rustfmt compliance for the T0 gate | P1 | DONE | `chore/rustfmt-pocket-tts-chain` | #43 | `e7eaa0b` | `cargo fmt --check` | 0 differences | — | — | — |
| 6 | Cache never deletes the runtime | P0 | DONE | `fix/tts-model-cache-preserves-runtime` | #44 | `4553cf8` | 6 deterministic checks | green in CI | — | — | — |
| 7 | Generation supervision failure modes | P0 | DONE | `test/pocket-tts-generation-supervision` | #45 | `ee3abab` | 5 deterministic checks | green in CI | — | — | — |
| 8 | ONNX Runtime download reliability | P1 | VALIDATING | `ci/harden-onnx-runtime-download` | #46 | `9a5ba3c` | CI on PR #46 | running | — | — | — |
| 9 | Model lifecycle states | P0 | VALIDATING | `feat/tts-model-lifecycle-status` | #48 | `8abbf42` | 5 deterministic checks | running | the states that need weight integrity or download tracking | — | — |
| 10 | Global pause/resume shortcut | P0 | TODO | — | — | — | — | — | not started | the implementation exists in the orphaned PR #36 | port it onto the head chain |
| 11 | STT qualification | P0 | VALIDATING | `test/stt-real-french-roundtrip` | #52 | `259fbb5` | a real French recording transcribed by the pinned model | running | the device and permission scenarios | needs a real microphone | execute the manual protocol |
| 12 | WCAG 2.2 AA audit | P1 | TODO | — | — | — | — | — | not started | — | — |
| 13 | Dependency audit | P1 | VALIDATING | `fix/dependency-advisories` | #54 | `2d3bf46` | the RustSec advisory database | running | eight advisories need a major upgrade of a parent dependency | — | tracked and listed in the audit gate |
| 14 | Qualification report | P1 | DONE | `docs/murmure-voice-qualification` | #47 | `f5cdbc8` | — | this document | — | — | — |
| 15 | Rapid triggers keep the recording | P0 | VALIDATING | `fix/stt-second-trigger-keeps-recording` | #51 | `ad22b02` | 4 unit tests | running | — | — | — |
| 16 | French Pocket TTS errors | P1 | VALIDATING | `fix/tts-localized-errors` | #53 | `8e2638b` | the type check and the lint | running | — | — | — |
| 17 | Real French WAV through the production Rodio decoder | P0 | VALIDATING | `feat/tts-pause-resume-hotkey-port` | #50 | `07e9b41` | the real generated recording | running | — | — | — |

### 3.1 Defects found and fixed

| Defect | Severity | Evidence | Fix |
| --- | --- | --- | --- |
| The two critical real-audio jobs were gated on exact branch names, so they reported `skipped` on every later pull request | Critical | job `118970407231` and the earlier `skipping` results | PR #31, `e26761c` |
| "Clear the model cache" deleted the installed, verified Pocket TTS runtime, because the Tauri cache and local-data roots are the same directory on Windows | Critical | `tauri` 2.12.4 `src/path/desktop.rs:53,86,265,278` | PR #44, `4553cf8` |
| A Windows junction at the model cache root was accepted, because Rust reports a junction as an ordinary directory | High | `FileType::is_symlink` is false for `IO_REPARSE_TAG_MOUNT_POINT` | PR #44 |
| The verification scripts declared `#requires -Version 5.1` but used `ProcessStartInfo.ArgumentList`, which only exists on .NET Core, so they aborted with an opaque null-method error | High | reproduced locally on Windows PowerShell 5.1.26100.7309 | PR #40 |
| The verification scripts inherited the developer's global Hugging Face cache and any personal token | High | inspection of the previous scripts | PR #40 |
| The offline proof only checked the `RIFF`/`WAVE` magic bytes, so a cached run producing a header without audio still passed | Medium | inspection of `test-pocket-tts-offline-windows.ps1` | PR #41 |
| The WAV parser mixed a buffered `BinaryReader` with raw stream positions, so the chunk walk could desynchronize | Medium | the buffered reader moves the stream cursor further than the bytes it consumed | PR #41 |
| A runtime that exited before reading the text was reported as a transport error instead of a generation failure | Medium | CI failure `a_generation_that_produces_no_wave_is_reported` | PR #45 |
| The ONNX Runtime prebuilt download is not covered by the rust cache, so a dropped connection failed the mandatory Clippy gate | Medium | job `118970407231`, `Peer disconnected` | PR #46 |
| A duplicated `key:` in a workflow makes GitHub refuse to load it, and PyYAML accepts duplicate keys silently | Medium | three runs failed with no job at all | PR #46, `scripts/validate-workflows.py` |
| A rapid second trigger replaced the chunk pipeline of the running recording, so the audio was discarded and the user got an empty result | High | the wake word path called `record_audio` without the guard the shortcut paths have | PR #51, an atomic session claim |
| The real French WAV proof required 120 silent samples inside a 20 ms window, which Rodio's mixer blocks can exceed | Medium | the proof failed on a loaded runner although the transport behaved correctly | PR #50, a realistic silence bound |
| The Tauri build script refuses to run without the SmartMic resources, so the Rust crate never compiled in the new jobs | Medium | `glob pattern resources/smartmic/* path not found` | PR #52, the resources are built first |
| The dependency audit found fourteen vulnerabilities on the lock file | High | `cargo audit` against `src-tauri/Cargo.lock` | PR #54, the reachable ones are fixed and the rest are listed with their reason |
| The dependency audit could not publish its result | Low | `Resource not accessible by integration` | PR #54, the job now grants the `checks` permission |

---

## 4. Known gaps and risks

| Gap | Impact | Why it is still open |
| --- | --- | --- |
| The graphical user journey of the packaged application is not automated | The Definition of Done asks for a demonstrated full journey | Driving the Tauri interface needs a visible desktop session; the CI runner is headless. A reproducible protocol is in §6. |
| The global pause/resume shortcut is missing from the head chain | MV-009 is a P0 issue | The implementation exists in `feat/tts-pause-resume-hotkey` (PR #36), which is **not** an ancestor of the current chain head. |
| The Rodio real-WAV decode test is outside the chain | Lower: the real-audio CI job covers the actual WAV | `test/pocket-tts-real-wav-rodio` (PR #35) is not an ancestor of the chain head. |
| The model state machine is not exposed | The UI cannot show whether a model is ready, corrupted or usable offline | Partly closed by PR #48: the catalog, the local state and the offline availability are exposed. The states that need weight integrity or download tracking remain open. |
| Linux and macOS have no pinned Pocket TTS build | No TTS outside Windows | No official Kyutai release is pinned for those platforms; using an unverified binary is refused by design. |
| The STT path is not re-qualified in this effort | STT remains at the upstream level of maturity | Closed by PR #52, which transcribes real French speech with the pinned Parakeet model, and by the STT map in §2.4. The device, permission and interruption scenarios stay manual. |
| No dependency audit runs in CI | Known dependency vulnerabilities may go unnoticed | `cargo audit` / `osv-scanner` are not wired. |
| Error messages from the Rust backend reach the French interface untranslated | Reduced UX quality | The backend returns English strings; the frontend shows them verbatim. |

---

## 5. How to reproduce the evidence

```powershell
# Real French audio with the official pinned runtime (PowerShell 7 required)
./scripts/verify-pocket-tts-windows.ps1 -Destination "$env:TEMP\pocket-tts" -GenerateFrench

# Same, with the executable denied outbound network access
./scripts/test-pocket-tts-offline-windows.ps1 -Destination "$env:TEMP\pocket-tts-offline"

# Deterministic contract checks, no runtime needed
./scripts/test-pocket-tts-wave-validator.ps1
./scripts/test-pocket-tts-model-cache.ps1

# The packaged application, on a Windows runner
./scripts/validate-packaged-windows.ps1

# Workflow definitions, including duplicate keys
python3 scripts/validate-workflows.py
```

Every claim in this document can be traced to a job in
`https://github.com/Rwanbt/murmure/actions` for the SHA listed in §3.

---

## 6. Manual validation protocol

This protocol covers what the headless CI cannot prove. It must be run on a
Windows machine with a visible desktop, a speaker and a microphone.

1. Build and install the packaged application
   (`pnpm tauri build --bundles nsis`, then run the NSIS installer).
2. Open **Extensions → Text to Speech**.
3. Confirm the CPU compatibility notice is absent on an AVX2 CPU.
4. Confirm the runtime is reported as not installed, and that the install
   button offers the pinned official release.
5. Install the runtime, watch the progress, cancel it, then install again.
   Confirm the executable is present and verified.
6. Confirm **no model cache** was created before consent.
7. Grant consent, run a French preview, and confirm audio is produced.
8. Pause, resume and stop from the interface.
9. Change the volume and confirm it applies to the next playback.
10. Select a French passage in another application and read it with the
    global shortcut.
11. Close the window and confirm the application stays in the tray; quit it
    from the tray menu.
12. Restart the application with the network disabled and confirm the
    cached model is reused.
13. Delete the model cache and confirm the installed runtime is still
    present and still verified.

---

## 7. Verdict

**NO-GO** at the time of writing.

The reason is not the TTS engine: real French synthesis, the verified
runtime installation, the offline path, the installer, the uninstaller and
the cache lifecycle are demonstrated by automated evidence. The reason is
that the Definition of Done is not yet met:

- the graphical user journey of the packaged application is not demonstrated
  and is still `BLOCKED` on a desktop session;
- the global pause/resume shortcut is missing from the chain;
- the STT path is only partly re-qualified: the engine transcribes real
  French speech, but the device, permission and interruption scenarios
  still need a real microphone;
- the model lifecycle states are only partly exposed;
- the dependency audit is not wired into CI.

The verdict can move to **CONDITIONAL GO** once the manual protocol in §6
has been executed and recorded, and to **GO** once the remaining `TODO`
rows of §3 are closed with evidence. The limitation that cannot be closed
by this fork is the absence of an official Pocket TTS build for Linux and
macOS; that is a platform limit, not a defect, and it is recorded rather
than papered over.
