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

**The official Kyutai release publishes a verified Pocket TTS runtime for
five targets, not only for Windows.** Release `v4.0.0` carries binaries
whose digests are published in its `SHA256SUMS`:

| Target | Pinned archive SHA-256 |
| --- | --- |
| `x86_64-pc-windows-msvc` | `b3b2faa7…` |
| `x86_64-unknown-linux-gnu` | `1336a77e60ce11650a5a3dc354770155a345bfa797f7a05d10bc5b1bcfe0d7e3` |
| `aarch64-unknown-linux-gnu` | `644a9478d7cbd7d432f5a4898007aa7d178a897a65aef7ebc64557e0b1a1739a` |
| `x86_64-apple-darwin` | `3a908c524d1ca976eb5f699c523fbdd47a5fa029ccc3bd9f6197b8f8d91f01c1` |
| `aarch64-apple-darwin` | `e209feca849d53df18eba89f5bd4c253c1be62a6c54794db5654635c95e02990` |

A previous revision of this report claimed that no official Linux or macOS
build existed and that functional parity was therefore impossible. That
claim was wrong: it was inferred from the single archive that this fork had
pinned, not from the release itself. The Windows runtime is the only one
that is wired, verified and exercised today; the other four are published
and pinned above but **not yet qualified**. Until they are, the Rust side
still refuses to run TTS on those platforms rather than accepting an
unverified binary (`src-tauri/src/tts/preview.rs`, `binary_path`), so
Linux and macOS TTS parity stays **not claimed** — for lack of
qualification, not for lack of an official build.

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
| Dependency audit | **PROVEN** (both trees) | Rust: `cargo audit` checks the RustSec database against the exact lock file, the reachable advisories are fixed and the eight that need a major upgrade of a parent are listed with their reason. JavaScript: `scripts/audit-js-dependencies.py` queries the open `osv.dev` database with the 1029 packages the lock file installs. It found five advisories, three of them closed by updating `vite` to 7.3.7; the two that remain are accepted with a written justification and fail the job if any other appears. |

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
| First recording | **PARTIAL** | the capture device and the transcription are proven on real hardware (§6.1.1); the shortcut-driven flow that starts and stops a recording through the interface is not |
| Repeated recording | **PARTIAL** | `a_finished_session_can_be_claimed_again` proves a finished session can be claimed again; no real recording was repeated |
| Manual stop | **PARTIAL** | the superseded branch releases the claim, and the claim has unit tests; no real two-second stop was performed |
| Short recording | **PARTIAL** | `audio::chunking` carries 19 tests over the windowing; no real short recording was made |
| Long recording within the limits | **PARTIAL** | the same 19 chunking tests; no real long recording was made |
| Silence | **PROVEN at the engine level** | `real_silence_is_not_transcribed` returns an empty text for a real five second capture of this machine's microphone (peak 2.2 % of full scale) and for the deterministic fixture. The application adds the voice activity gate on top, covered by 10 unit tests |
| No audio input | **NOT TESTED** | the `InitFailed` branch of `audio.rs` calls `notify_recording_error`, and `audio.rs` carries no test. It has never been exercised on a machine without an input device |
| Microphone unavailable | **NOT TESTED** | same branch, same reason |
| Permission refused | **MANUAL** | needs the operating system permission prompt |
| Permission revoked | **MANUAL** | same |
| Device change | **MANUAL** | needs two real devices and a change while the application runs |
| System interruption | **PARTIAL** | the session claim survives a competing trigger; a full sleep and wake needs a real device |
| French transcription | **PROVEN** | job `119192646552` and a local run print `"bonjour ceci est un essai de synthese vocale locale"` for a real recording. The assertion itself requires alphabetic output, not specific words |
| Offline transcription | **NOT TESTED** | the model and the tokenizer are local and the code path makes no network call, but no run was made with the network disabled |
| Model absent | **NOT TESTED** | the path exists; no test asserts what it reports |
| Model corrupted | **NOT TESTED** | the ONNX session would fail to load; no test exercises it |
| Restart after a failure | **PARTIAL** | the claim is released on the error paths; no restart was performed after a real failure |
| Several rapid triggers | **PROVEN** | `a_second_trigger_cannot_claim_a_running_session` and `exactly_one_of_two_concurrent_claims_wins` |
| Concurrent shortcuts | **PROVEN** | the same two tests: exactly one of two concurrent claims wins |

---

## 3. Progress journal

Status values: `TODO`, `IN_PROGRESS`, `BLOCKED`, `VALIDATING`, `DONE`.

| # | Workstream | Priority | Status | Branch | PR | SHA | Tests executed | Result | Remaining | Risk | Next action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Continuous real-audio CI gate | P0 | DONE | `test/pocket-tts-offline-cache-windows` | #31 | `e26761c` | CI on PR #31 | real French WAV 19 s, cached WAV 25 s, qualification gate pass | historical runs of PRs #33–#39 still show the old skip | GitHub does not re-run CI when only a base branch moves | Merge bottom-up; the gate then covers every later pull request |
| 2 | Model cache isolation and credential hygiene | P0 | DONE | `fix/pocket-tts-script-powershell-contract` | #40 | `e924b2e` | 11 deterministic checks | green in CI | — | — | — |
| 3 | Shared structural WAV validator | P0 | DONE | `test/pocket-tts-wav-validator-contract` | #41 | `4506c0c` | 11 deterministic checks | green in CI | — | — | — |
| 4 | Packaged Windows application | P0 | IN_PROGRESS | `fix/dependency-advisories` | #54 | `adbef67` | the CI job, then a full install cycle on a real desktop | Job `119192646518`: every step green. Locally: NSIS bundle 590 MB, silent install exit 0 in 61 s (16 files, 666.4 MB), registry entry `murmure 2.0.3`, launch with window `Murmure`, 39 threads, 50 MB, no orphan, silent uninstall exit 0 removing the directory and the key, user settings preserved (§6.0) | the interactive journey (steps 2 to 10 and 12 to 13) needs a human at the desktop | — | run §6 |
| 5 | rustfmt compliance for the T0 gate | P1 | DONE | `chore/rustfmt-pocket-tts-chain` | #43 | `e7eaa0b` | `cargo fmt --check` | 0 differences | — | — | — |
| 6 | Cache never deletes the runtime | P0 | DONE | `fix/tts-model-cache-preserves-runtime` | #44 | `4553cf8` | 6 deterministic checks | green in CI | — | — | — |
| 7 | Generation supervision failure modes | P0 | DONE | `test/pocket-tts-generation-supervision` | #45 | `ee3abab` | 5 deterministic checks | green in CI | — | — | — |
| 8 | ONNX Runtime download reliability | P1 | DONE | `ci/harden-onnx-runtime-download` | #46 | `9a5ba3c` | the installer, run by three jobs | `install-onnx-runtime.ps1` verifies the archive digest (`0b38df9a…`) and the library digest the engine checks (`dec964ab…`) before anything is used; a mismatch aborts. Exercised by the round-trip, Rodio and packaged jobs in runs `39591303205` and `39593352219` | - | - | - |
| 9 | Model lifecycle states | P0 | DONE | `fix/dependency-advisories` | #54 | `e913b2b` | 4 deterministic tests plus the 410-test suite | `an_unfinished_blob_is_reported_as_downloading`, `an_empty_weight_file_is_never_reported_as_ready`, `an_empty_weights_directory_is_never_reported_as_ready`, `weights_in_a_subdirectory_are_reported_as_ready`; the dangling-link test is `#[cfg(unix)]` and runs on the Linux job | The state followed the presence of the directory, so a partial download was shown as ready and offline usable. It now reads the `<digest>.incomplete` blob and the readability of every weight | the five states the mission lists as `verifying`, `failed` and `removing` have no signal today: the weights are fetched by the official CLI, and nothing observes that moment | watch the CLI output if those states are required |
| 10 | Global pause/resume shortcut | P0 | DONE | `feat/tts-pause-resume-hotkey-port` | inherited by the chain | `3ebd585` | `registry.rs` tests plus the 406-test suite | `registers_pause_tts_without_colliding_with_stt_or_stop`, `registers_default_tts_stop_shortcut_without_affecting_stt`, `registers_selection_read_shortcut_without_mutating_stt`; the global hotkey is registered in `registry.rs` and dispatched in `shortcuts.rs` | The journal said TODO; the shortcut was already registered, wired to `TogglePauseTts` and covered by tests. The interface reads it from `SHORTCUT_CONFIGS.ttsPause` | pressing the real hotkey during playback needs a desktop | the manual row in 6.1 |
| 11 | STT qualification | P0 | DONE | `fix/dependency-advisories` | #54 | `5f5b474` | the whole unit suite plus a real French recording through the pinned model | job `119192646552` at `5f5b474`: `1 passed; 0 failed; 404 filtered out`, transcription `"bonjour ceci est un essai de synthese vocale locale"`, engine load 2.27 s, 0.83 s for 3.64 s of audio (real time factor 0.228) | Two stacked defects held the claim: the test was never registered (`running 0 tests` in run `39587708682`), then the job had never installed the ONNX Runtime the engine loads (`The ONNX Runtime is not installed` in run `39589886791`). Both fixed in `0264519` and `5f5b474` | the device and permission scenarios stay manual | the manual microphone and device scenarios (6.1) |
| 12 | WCAG 2.2 AA audit | P1 | IN_PROGRESS | `fix/dependency-advisories` | #54 | `e913b2b` | contrast measured on the tokens, static audit of 133 screens | `check-contrast.py`: 9 pairs above their thresholds; `audit-accessibility.py`: 133 screens, 0 finding | screen-reader reading needs a manual pass | the announcement order cannot be measured statically | run §6.2 |
| 13 | Dependency audit | P1 | DONE | `fix/dependency-advisories` | #54 | `b02b79e` | RustSec and `osv.dev` | `Dependency audit` and `JavaScript dependency audit` green at `5f5b474` and `9ce4800`; the eight Rust and two JavaScript advisories are accepted with a written reason in the job, and any unlisted advisory fails the build | the accepted advisories need a major upgrade of a parent dependency | tracked and listed in the audit gate | revisit when the parents move |
| 14 | Qualification report | P1 | DONE | `docs/murmure-voice-qualification` | #47 | `f5cdbc8` | — | this document | — | — | — |
| 15 | Rapid triggers keep the recording | P0 | DONE | `fix/stt-second-trigger-keeps-recording` | #51 | `d6c5548` | 4 unit tests | `a_second_trigger_cannot_claim_a_running_session`, `a_finished_session_can_be_claimed_again`, `exactly_one_of_two_concurrent_claims_wins`, `invalidating_a_session_advances_the_generation`, all green in the 410-test suite | pressing the real shortcut four times needs a desktop | §6.1 row 14 | - |
| 16 | French Pocket TTS errors | P1 | DONE | `fix/tts-localized-errors` | #53 | `c2013c8` | the type check, the lint and the locale file | `npx tsc --noEmit` and `eslint --max-warnings 0` both exit 0 locally; `fr.json` holds 749 keys, including the two added for the new model states | - | - | - |
| 17 | Real French WAV through the production Rodio decoder | P0 | DONE | `feat/tts-pause-resume-hotkey-port` | #50 | `ddaa83e` | the real generated recording, decoded by Rodio | job `Pocket TTS real French WAV decoded by Rodio` green at `5f5b474` and `9ce4800` | - | the runner has no audio device, so the transport is checked on an in-memory queue | - |
| 18 | The engine does not depend on the graphics stack | P0 | DONE | `feat/tts-pause-resume-hotkey-port` | #50 | `c99f3e4` | the whole unit suite, 400 passed | `cargo test --lib` | the packaged application has to ship the runtime | - | validate the packaged installer |
| 19 | No binary depends on the common controls task dialog | P0 | DONE | `feat/tts-pause-resume-hotkey-port` | #50 | `cdb5930` | the whole unit suite, 400 passed and 0 failed | `cargo test --lib` | - | - | - |
| 20 | Pocket TTS on Linux and macOS | P2 | DONE (x86_64 Linux, aarch64 macOS) | `fix/dependency-advisories` | #54 | `3ec8e7d` | 11 validator checks, 4 archive digests and 4 executable digests pinned, 2 real French syntheses | job `119154672419` (Linux): `24000 Hz, 1 channel(s), 16 bits, 3.48 s, peak 0.405`; job `119154672275` (macOS): same recording, executable `6a7a6cb0…` | aarch64 Linux and x86_64 macOS are pinned but no runner exercises them | - | add those two runners or record them as unqualified |

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
| The real French WAV proof required 120 silent samples inside a 20 ms window, which Rodio's mixer blocks can exceed | Medium | the proof failed on a loaded runner although the transport behaved correctly | PR #50, the proof now inspects the decoded samples directly and the transport stays proven on the in-memory queue |
| The interface offered the runtime installation only when the browser user agent contained `Windows`, so Linux and macOS users could never install a runtime that the official release does publish | High | the install button was gated on `navigator.userAgent.includes('Windows')`, while the backend verifies a pinned digest for four POSIX targets | PR #54: the backend now reports `platformSupported` from the installer itself, the button follows that capability, and a platform without a pinned build says so instead of showing nothing |
| The installation download reported its progress only as a sentence, so assistive technology received no value | Medium | the progress region was a status paragraph with a percentage in its text | PR #54: a real `progress` element carries `max` and `value` and is labelled |
| The interface told every user that the runtime was a Windows one | Medium | the missing-runtime message and the install button named Windows | PR #54: both name the official runtime, and the French catalogue carries the new strings |
| A test that proved the speech recognition round trip was not a test at all, so its job stayed green while running zero cases | **Critical** | the function carried `#[ignore]` without `#[test]`; `cargo test` reported `running 0 tests; 404 filtered out` and exited successfully (run `39587708682`) | PR #54 `0264519`: the attribute is restored, the job fails unless the output contains `1 passed`, and `scripts/check-ignored-tests.py` refuses any ignored function that is not registered, in `Check Guidelines` |
| The test binaries refused to start, so no unit test could run on Windows | Critical | `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) reproduced locally. The window menu library of the framework imports `comctl32.dll!TaskDialogIndirect`, which only the side-by-side version 6 of that library exports; a test binary is produced by the test harness of the compiler and carries no manifest, so the loader resolved the version 5 and refused the process before the first test | PR #50 `cdb5930`. Two attempts were made: embedding the manifest in every linked target fixed the tests but broke the release link with `CVT1100: duplicate resource type MANIFEST`, because the framework already embeds its own manifest through `resource.lib`. The dependency is instead removed at its source by leaving out the framework's optional `common-controls-v6` feature, so the library falls back to the message box of the user interface library. No binary depends on the side-by-side assembly any more. Proof: `cargo test --lib` reports 400 passed and 0 failed |
| The speech recognition engine pulled the DirectX 12 libraries into the import table of every binary, so a Windows installation without the full graphics stack refused to start the program | High | the ONNX Runtime distribution that the build script downloads on Windows is compiled with the DirectML execution provider, so it declares `/DEFAULTLIB:d3d12.lib` and `/DEFAULTLIB:dxgi.lib` | PR #50 `c99f3e4`, the official CPU-only ONNX Runtime is loaded at runtime from a verified path and the DirectX dependency is gone |
| The Tauri build script refuses to run without the SmartMic resources, so the Rust crate never compiled in the new jobs | Medium | `glob pattern resources/smartmic/* path not found` | PR #52, the resources are built first |
| The frontend dependency tree was never audited, so a vulnerable transitive package could have shipped unnoticed | High | `scripts/audit-js-dependencies.py` against `osv.dev`: five advisories affected the locked versions | PR #54: `vite` 7.3.5 -> 7.3.7 closes GHSA-587h-7wmj-h5pm, GHSA-698v-4r3x-3vcv and GHSA-r825-w8mm-5pxw. GHSA-vfj7-8cjw-p6xm (`braces`, no patched release, needs an attacker-controlled glob pattern that only the build configuration provides) and GHSA-238p-pmpm-9mq7 (`katex`, pinned by the component library at `^0.16`, needs TeX rendered with `trust`, which the application never does) are accepted with their reason and fail the job if any other advisory appears |
| The dependency audit found fourteen vulnerabilities on the lock file | High | `cargo audit` against `src-tauri/Cargo.lock` | PR #54, the reachable ones are fixed and the rest are listed with their reason |
| The dependency audit could not publish its result | Low | `Resource not accessible by integration` | PR #54, the job now grants the `checks` permission |
| The round-trip job never installed the ONNX Runtime that the engine loads, so the test could not have passed even once it was registered | **Critical** | the first honest run failed with `The ONNX Runtime is not installed, the speech recognition is unavailable` (run `39589886791`); the missing step was invisible while the test ran zero cases | PR #54 `5f5b474`: the job installs the official CPU-only distribution and exports `ORT_DYLIB_PATH`, like the two other jobs that load the engine |
| A model whose download was interrupted was presented as "Ready, usable offline", and the first synthesis then failed in front of the user | High | the state followed the presence of the directory only; an unfinished Hugging Face download leaves the directory and a `<digest>.incomplete` blob | PR #54 `e913b2b`: three real signals are read instead of one guess (`<digest>.incomplete`, a zero byte weight, a snapshot entry whose blob is gone), the states `downloading` and `corrupted` are reported, `offline_ready` requires `ready` or `in_use`, and the interface and the French catalogue name the new states |
| A compiled Python bytecode file was committed, and the root build directory was not ignored although local builds write 5.7 GB there | Low | `git log -- scripts/__pycache__/verify-pocket-tts-wave.cpython-313.pyc` names `59ea171`; `git status` listed `target/` as untracked | PR #54 `1dc497a`: the file leaves the index and the two patterns are ignored |
| The diagnostic script that runs on a failed test launched the test binary with no argument, so the whole suite ran without a filter and reported an unrelated failure | Medium | the round-trip log of run `39589886791` contains `running 405 tests` and a failure in `commands::murmure_file` | PR #54 `5f5b474`: it now passes `--list`, which makes the harness print its test names without running them; the loader has already done its work at that point, which is the only thing the script observes |

---

## 4. Known gaps and risks

| Gap | Impact | Why it is still open |
| --- | --- | --- |
| The graphical user journey of the packaged application is not automated | The Definition of Done asks for a demonstrated full journey | Driving the Tauri interface needs a visible desktop session; the CI runner is headless. A reproducible protocol is in §6. |
| The global pause/resume shortcut is missing from the head chain | MV-009 is a P0 issue | The implementation exists in `feat/tts-pause-resume-hotkey` (PR #36), which is **not** an ancestor of the current chain head. |
| The Rodio real-WAV decode test is outside the chain | Lower: the real-audio CI job covers the actual WAV | `test/pocket-tts-real-wav-rodio` (PR #35) is not an ancestor of the chain head. |
| The model state machine is not exposed | The UI cannot show whether a model is ready, corrupted or usable offline | Partly closed by PR #48: the catalog, the local state and the offline availability are exposed. The states that need weight integrity or download tracking remain open. |
| Nothing prevented a screen from shipping a mouse-only control, an unnamed field or a reordered focus sequence | Accessibility depended on whoever reviewed the screen last | `scripts/audit-accessibility.py` audits the 133 screens of `src/features` for 2.1.1 (a clickable non-button needs a role and a keyboard handler), 4.1.2 (every field needs a name, explicit, implicit or by `aria-labelledby`), 1.1.1 (an image needs `alt`) and 2.4.3 (a positive `tabIndex` is refused). It found one real defect: the WAV file picker kept a hidden but focusable and unnamed file input in the tab order, now removed from both. It runs in `Check Guidelines`, so the next screen with one of those patterns fails the build. |
| Contrast and screen-reader behaviour were asserted rather than measured | A palette change could drop the interface below AA without anybody noticing | `scripts/check-contrast.py` now converts the `oklch()` tokens of the dark theme to sRGB and checks them against WCAG 2.2: body text 16.11:1, secondary text 6.74:1 (5.66:1 on a card), error text 6.13:1, warning text 8.26:1, focus indicator 3.67:1, all above their thresholds. The component boundary measures 1.71:1 and is reported as advisory, because every control carries a text label and Understanding 1.4.11 does not require a boundary in that case. The check runs in `Check Guidelines`, so a regression fails the build. Screen-reader reading still needs a manual pass. |
| Two of the four POSIX targets are pinned but never exercised | Anyone on `aarch64` Linux or `x86_64` macOS relies on a digest that no run has reproduced | The official release publishes `aarch64-unknown-linux-gnu` and `x86_64-apple-darwin`, and both are pinned with their archive and executable digests. Continuous integration exercises `x86_64-unknown-linux-gnu` (job `119154672419`) and `aarch64-apple-darwin` (job `119154672275`), which produced a real French WAV of 3.48 s at 24 kHz. The other two targets are refused at run time rather than trusted, and are listed as unqualified. |
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

### 6.0 Executed on a real Windows desktop

Steps 1, 11 and the whole install cycle were executed on the project owner's
machine, so they are evidence rather than protocol.

- Build: `pnpm build` then `pnpm tauri build` on Windows with the pinned Rust
  toolchain. The release profile finished in 19 min 34 s and produced
  `target/release/murmure.exe`, 27.7 MB. The NSIS bundle
  `murmure_2.0.3_x64-setup.exe` is 590 MB, because the speech model travels
  inside it.
- Clean install: the target directory, the registry key and the installed
  program were absent before the run. `murmure_2.0.3_x64-setup.exe /S`
  exited 0 after 61 s and installed 16 files, 666.4 MB, in
  `%LOCALAPPDATA%\murmure`, with `murmure.exe` and `uninstall.exe` present.
  The uninstall entry registered `murmure`, version 2.0.3.
- Launch of the installed application: the executable opened a window titled
  `Murmure`, 39 threads, 50 MB of working set, and it was still alive after
  12 s. It was then stopped, and `Get-Process murmure` returned zero entries.
- Uninstall: `uninstall.exe /S` exited 0; the install directory and the
  uninstall registry key were both gone afterwards. No orphan process
  remained.
- Persisted data: a first start creates `%APPDATA%\com.al1x-ai.murmure` with
  `settings.json` (59 keys: shortcuts, modes, text to speech preferences),
  `dictionary.json`, `formatting_rules.json`, `llm_connect.json` and
  `stats_aggregated.json` (five aggregate counters: words per minute sums,
  words this month, last reset, total audio bytes). No recording, no
  transcript and no audio file is written there, and the uninstaller leaves
  the directory in place, so a reinstall keeps the user's settings. An
  earlier claim in this document said that a first start wrote nothing: it
  was wrong, because the paths checked were `com.murmure.app` and `murmure`
  instead of the identifier the application actually uses.
- Not executed here: steps 2 to 10 and 12 to 13, which need a human to click
  through the interface.

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

### 6.1 Microphone and speech recognition

The continuous integration never touches a microphone, so the interface
scenarios below still need a human at a desktop with an input device. One
part, however, has been executed for real and is recorded in §6.1.1: the
capture path of this machine and the transcription of audio that came
through the air rather than from a reference file.

Record the outcome of each one.

| # | Scenario | What to do | What must happen |
| --- | --- | --- | --- |
| 1 | First recording | Press the recording shortcut, say a French sentence, press it again | A transcription appears, the audio that was captured matches what was said, and no recording starts before the shortcut |
| 2 | Repeated recording | Do the first scenario three times in a row | Every attempt produces its own transcription, and no recording stays open between them |
| 3 | Manual stop | Start a recording and stop it by hand after two seconds | The recording stops immediately and the short audio is transcribed rather than discarded |
| 4 | Long recording | Record for the longest duration the interface allows | The recording ends at the limit without losing the beginning, and the transcription arrives |
| 5 | Silence | Record five seconds of silence | The result is empty or a clear message, never invented text |
| 6 | Absent input | Disable the microphone in the system settings, then record | A message names the missing device; the application does not hang and does not crash |
| 7 | Permission refused | Remove the microphone permission of the application, then record | A message states that permission is required, with no silent failure |
| 8 | Permission revoked | Start with permission granted, revoke it while the application runs, then record | The revocation is noticed on the next attempt, and the application recovers once permission returns |
| 9 | Device change | Plug or unplug a microphone, or switch the default device, then record | The next recording uses the new device without restarting the application |
| 10 | System interruption | Put the machine to sleep during a recording and wake it | The session ends cleanly, the microphone is released, and the tray shows an idle state |
| 11 | Absent model | Move the speech model directory away, then record | A message names the missing model; no partial transcript is produced |
| 12 | Corrupted model | Truncate one model file, then record | The failure is reported as such, and the application stays usable after the file is restored |
| 13 | Restart after a failure | After any failure above, restart the application and record again | The first recording after the restart succeeds |
| 14 | Rapid triggers | Press the recording shortcut four times quickly | Exactly one recording runs; the running audio is never discarded |
| 15 | Concurrent shortcuts | Press the recording shortcut and a text-to-speech shortcut together | The two do not interleave: either one is refused with a message, or they run one after the other |

#### 6.1.1 Executed: real capture device and real transcription

This part does not need the interface and was executed, so it is evidence
rather than protocol.

- Machine: Windows desktop, Realtek High Definition Audio. Two capture
  devices are exposed, and the Windows multimedia registry reports
  `Microphone Array` and `Headset` as active (`DeviceState = 1`).
- Capture: `ffmpeg -f dshow` opened
  `Microphone Array (Intel® Smart Sound Technology for Digital Microphones)`
  and recorded 9 s at 16 kHz mono. The measured file holds 143 792 frames,
  8.99 s, peak 8 008 (24.4 % of full scale), RMS 628. A device that never
  opened would have produced no file; a muted path would have produced a
  peak near zero.
- Signal: while the capture ran, the speaker played the French sample that
  Pocket TTS generated on this same machine (`french-smoke.wav`, 3.64 s,
  24 000 Hz, peak 10 268) twice. The audio therefore travelled through the
  air, not through a cable or a shared buffer.
- Transcription by the pinned engine, run from the local test binary:
  `engine load: 1.90 s, transcription: 1.11 s for 8.99 s of audio, real time
  factor 0.123`, output `"en moche chris est dans lessai de chante du local
  local"`. The words are recognisable and corrupted in the way a far-field
  laptop microphone corrupts a speaker; the point is that real captured
  audio reached the real model and produced real words.
- The same binary, given the clean generated sample, returned
  `"bonjour ceci est un essai de synthese vocale locale"` after loading in
  2.08 s and transcribing 3.64 s in 0.76 s (real time factor 0.208), which
  reproduces the result the job `119192646552` reported.

What this proves: the capture device opens, delivers samples, and its audio
is transcribed by the pinned model on this hardware. What it does not
prove: every row above, because those rows drive the interface and change
the device or the permission while the application runs.

#### 6.1.2 Executed: silence produces no words

The opposite failure is worse than a missing transcription: a model that
turns room noise into a sentence would write text nobody said.

- Five seconds were captured from the same device with no sound played.
  The file holds 79 776 frames, peak 736 (2.2 % of full scale), RMS 38,
  which is room hiss rather than speech.
- The engine returned an empty string for it, and the guard in the test
  refuses a fixture that carries speech (`peak < 0.05`), so the test cannot
  silently assert the opposite of its claim.
- The counter-test is now permanent: `real_silence_is_not_transcribed` runs
  in the round-trip job against a fixture generated with a fixed seed
  (`scripts/make-silence-wav.py`), because a runner has no microphone. The
  local run above is the hardware evidence; the job is the regression gate.


### 6.2 Screen reader

The contrast and the structure are measured by the pipeline, but the
announcement order is not. Run this with Narrator on Windows, or VoiceOver on
macOS, and record what is announced.

| # | Scenario | What must happen |
| --- | --- | --- |
| 1 | Reach the speech screen with the keyboard only | Every control is reachable in reading order, and the focus indicator is always visible |
| 2 | Focus the consent checkbox | Its purpose is announced in French, and its state is announced when it changes |
| 3 | Start a runtime installation | The progress is announced as a value that changes, not only as a moving sentence |
| 4 | Trigger an error | The message is announced without moving the focus away from where the user is |
| 5 | Focus the disabled speak button | The reason it is disabled is available to the screen reader, not only visually |
| 6 | Play, pause, resume, stop | Each state change is announced once, and the announcements do not overlap |
| 7 | Leave the screen with the keyboard | The focus never becomes trapped and never disappears |

### 6.3 Recording the results

Each row above is recorded with its date, the version under test, and its
outcome. A row that was not run stays untested: it must not be marked as
passed by omission. Once every row has an outcome, the verdict in §7 can move
to **CONDITIONAL GO** for the rows that passed, and the rows that failed
become defect reports.

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
rows of §3 are closed with evidence. The Linux and macOS TTS gap is not a
platform limit: the official release publishes those binaries, and the
digests are pinned above. Closing it needs a POSIX verification script, a
POSIX installer and a real-audio CI job per platform — work that is
identified and tracked, not impossible.
