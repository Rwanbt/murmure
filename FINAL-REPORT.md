# Murmure Voice — final qualification report

This report closes the qualification of the `Rwanbt/murmure` fork. Every
statement below is tied to a commit, a command, a job, or a file in the
repository. Anything that was not demonstrated is written as not
demonstrated, and the verdict accounts for it.

The detailed per-scenario evidence lives in [`QUALIFICATION.md`](QUALIFICATION.md);
this report is the summary and the verdict.

## 1. What was actually done

| Workstream | Result | Evidence |
| --- | --- | --- |
| Real French audio from the official runtime | **Demonstrated on Windows, Linux x86_64 and macOS arm64** | Windows jobs; Linux job `119154672419`: `24000 Hz, 1 channel(s), 16 bits, 3.48 s, peak 0.405`; macOS job `119154672275`: same recording |
| Deterministic negative tests for the audio contract | **11 checks**, generated fixtures, no network | `scripts/test-pocket-tts-wave-posix.py`, `scripts/test-pocket-tts-wave-validator.ps1` |
| Offline scenario | **Demonstrated** | "Pocket TTS cached French WAV without network" job |
| Verified runtime installation | **Demonstrated**, digest checked before and after extraction, atomic publication | `src-tauri/src/tts/install_windows.rs`, `install_posix.rs` |
| Packaged Windows application | **Built, installed, launched, uninstalled on a real desktop**, no orphan process, no temporary file, no leftover shortcut, user settings preserved | "Packaged Windows application" job `119192646518`, run `39591303205`; the same cycle repeated by hand on the project owner's machine (§6.0 of `QUALIFICATION.md`) |
| Safe cache removal | **Demonstrated**, junction/reparse refused, runtime never deleted | `src-tauri/src/tts/preview.rs`, 6 tests |
| Rust dependency audit | **Enforced**, 8 advisories accepted with a written reason | "Dependency audit" job |
| JavaScript dependency audit | **Enforced**, 5 advisories found, 3 closed, 2 accepted with a written reason | `scripts/audit-js-dependencies.py`, job `39584806493` |
| Contrast measured | **9 pairs**, all above their WCAG 2.2 threshold | `scripts/check-contrast.py` |
| Screen structure audit | **133 screens**, 0 finding | `scripts/audit-accessibility.py` |
| Rust unit tests | **410 registered, 405 passed, 5 ignored** — and each ignored test is executed by a named job, listed in `QUALIFICATION.md` §3.2 | `cargo test --lib` |
| TypeScript and lint | **Clean** | `tsc --noEmit` exit 0, `eslint --max-warnings 0` exit 0 |
| STT round trip with the real model | **PROVEN**, after two defects were removed. It now reports `1 passed; 0 failed; 404 filtered out`, transcribes `"bonjour ceci est un essai de synthese vocale locale"`, loads the engine in 2.27 s and transcribes 3.64 s of audio in 0.83 s (real time factor 0.228) | job "Pocket TTS to Parakeet French round trip" `119192646552`, run `39591303205` at `5f5b474`. Before that: `running 0 tests; 404 filtered out` in run `39587708682`, then `The ONNX Runtime is not installed` in run `39589886791` |
| Fourth French recording, generated on the project owner's machine | **PROVEN**: 3.64 s, 24 000 Hz, peak 10 268, from the digest-checked official runtime | `scripts/verify-pocket-tts-windows.ps1 -GenerateFrench -KeepGeneratedWav` |
| The recorder meets a real microphone | **PROVEN** for the capture path and the engine: 8.99 s captured, peak 24.4 % of full scale, transcribed as `"en moche chris est dans lessai de chante du local local"` after travelling through the air | `QUALIFICATION.md` §6.1.1 |
| Silence produces no words | **PROVEN**: a five second capture of room noise (peak 2.2 % of full scale) returns an empty transcription, in CI and locally | `real_silence_is_not_transcribed`, job `119199531784` |

## 2. Defects found and fixed

Every one of these was a real behaviour defect, not a style issue.

| Defect | Severity | How it was found |
| --- | --- | --- |
| Test binaries refused to start on Windows (`STATUS_ENTRYPOINT_NOT_FOUND`), so no unit test could run | **Critical** | Reproduced locally; the window library imports `comctl32!TaskDialogIndirect`, which only the side-by-side v6 exports, and a test binary carries no manifest |
| The speech engine pulled the DirectX 12 libraries into every binary, so a Windows install without the graphics stack could not start | **High** | `onnxruntime.lib` declares `/DEFAULTLIB:d3d12.lib` |
| The interface offered the runtime installation only when the user agent contained `Windows`, so Linux and macOS users could never install it | **High** | Static review of the screen |
| The frontend dependency tree had never been audited | **High** | New `osv.dev` audit found 5 advisories |
| "Clear the model cache" deleted the installed runtime | **Critical** | Tauri's cache and local-data roots are the same directory on Windows |
| A partially downloaded or unreadable model was presented as "Ready, usable offline" | **High** | The state followed the presence of the directory; an interrupted Hugging Face download leaves a directory with an `<digest>.incomplete` blob, and the first synthesis then failed in front of the user |
| The dictated words were written to the log | **High** | Ten `debug!`, `info!` and `trace!` statements printed the transcription or the recognized phrase. A log file outlives the audio it came from and travels into bug reports | `bb1d067`, `0c0da20` and `eb47928`, and `scripts/check-private-logs.py` now refuses the pattern in `Check Guidelines` |
| Two tests were ignored with a reason and had no runner at all | **High** | Neither installer test was ever handed the archive it needs, so both had been skipped silently since they were written |
| A rapid second trigger discarded the running recording | **High** | The wake word path lacked the guard the shortcuts have |
| A hidden but focusable file input trapped keyboard focus | **Medium** | New screen audit |
| The install progress was a sentence, not a value | **Medium** | Static review |

Four claims made earlier in this effort were **wrong and were corrected**:
1. "No official Pocket TTS build exists for Linux or macOS." It does: release
   `v4.0.0` publishes five targets with their digests in `SHA256SUMS`. The
   claim had been inferred from the single archive the fork pinned.
2. "The real French WAV proof needs the Rodio player to show the transport."
   The player drains its mixer in blocks, so that proof depended on the
   machine; the decode proof now inspects the samples and the transport is
   proven deterministically on an in-memory queue.
3. "The speech recognition round trip is proven by a green job." It was not:
   the test was never registered, so the job ran zero cases and exited
   successfully. A green job that runs nothing is worse than a red one,
   because it removes the reason to look. The attribute is restored, the job
   now refuses a run that does not report `1 passed`, and a mechanical check
   refuses any ignored function that is not registered. The first honest run
   then failed for a second reason: the job had never installed the ONNX
   Runtime the engine loads, which nobody could notice while the test was not
   running. With both fixed, the job passes on a real transcription
   (`5f5b474`, job `119192646552`).
4. "A first start of the application writes nothing." It does: the first
   launch creates `%APPDATA%\com.al1x-ai.murmure` with the settings, the
   dictionary, the formatting rules, the LLM connection file and five
   aggregate counters. The claim came from checking `com.murmure.app` and
   `murmure`, which are not the identifier this application uses, and
   concluding from the absence of a directory that nothing was written. What
   matters for privacy still holds and is now verified: no recording, no
   transcript and no audio file is stored there.

## 3. Pull requests

All are **Draft**, **open**, **not merged**. `main` is untouched at `ffef307`.

| PR | Branch | Head | Scope |
| --- | --- | --- | --- |
| [#50](https://github.com/Rwanbt/murmure/pull/50) | `feat/tts-pause-resume-hotkey-port` | `cdb5930` | pause/resume/hotkey port, deterministic audio proofs, engine without the graphics stack, no task-dialog dependency |
| [#51](https://github.com/Rwanbt/murmure/pull/51) | `fix/stt-second-trigger-keeps-recording` | `daca246` | the second trigger keeps the running recording |
| [#52](https://github.com/Rwanbt/murmure/pull/52) | `test/stt-real-french-roundtrip` | `d958663` | real French recording transcribed by the pinned model |
| [#53](https://github.com/Rwanbt/murmure/pull/53) | `fix/tts-localized-errors` | `684c640` | French error messages |
| [#54](https://github.com/Rwanbt/murmure/pull/54) | `fix/dependency-advisories` | `ef841b8` | dependency advisories, real STT round trip and its silence counter-test, the two installer tests that had no runner, model states that refuse a partial download, the privacy gate on log statements, CI robustness, and the qualification documents. Its body states that it exceeds the 400-line target and why |

## 4. Qualification matrix

Legend: **PROVEN** means a command or a job produced the result and the
evidence is named. **PARTIAL** means some part is demonstrated and another is
not. **BLOCKED** means the environment cannot produce the evidence.

| Area | Windows | Linux x86_64 | macOS arm64 | Linux arm64 / macOS x86_64 |
| --- | --- | --- | --- | --- |
| Compilation | **PROVEN** (`compile-windows`) | **PROVEN** (`compile-linux`) | **PROVEN** (`compile-macos`) | not exercised |
| Unit tests | **PROVEN** (405 passed of 410, 5 ignored with a named runner) | **PROVEN** | **PROVEN** | not exercised |
| TTS: real French audio from the official runtime | **PROVEN** on the runner and on a real desktop | **PROVEN** (job `119154672419`) | **PROVEN** (job `119154672275`) | pinned but **not exercised** |
| Offline audio | **PROVEN** | not exercised | not exercised | no |
| Installer: verified runtime | **PROVEN**, archive and executable digests checked before use, atomic publication, cancellation, overwrite refused (two tests that until `caf5b75` had no runner) | **PROVEN** for install/verify | **PROVEN** for install/verify | pinned, not exercised |
| Installer: packaged application | **PROVEN** on a real desktop: NSIS 590 MB, silent install 61 s, launch, silent uninstall, settings preserved, no leftover shortcut | no packaging | no packaging | no |
| Graphical user journey | **BLOCKED** — headless runner | **BLOCKED** | **BLOCKED** | **BLOCKED** |
| Audio device, permission refused or revoked, device change | **PARTIAL** — a real capture and a real silence are transcribed; the interface scenarios are **BLOCKED** | **BLOCKED** | **BLOCKED** | **BLOCKED** |
| STT transcription | **PROVEN** (real engine on a real reference recording, and on a real capture from this machine's microphone) | compilation only | compilation only | no |
| Security: child processes | **PROVEN** — the CLI receives the text on stdin rather than in its arguments, no inherited Hugging Face credential, timeouts on generation | shared | shared | shared |
| Security: file system | **PROVEN** — junction and reparse point refused, atomic publication, size limit, path validated before removal | shared | shared | shared |
| Security: Tauri surface | **PROVEN** for the client facing surface: the speech input is bounded to 1 000 characters, the selection to 1 000, the archives to 16 MB on Windows and 64 MB elsewhere, and the cache path is resolved by the backend rather than accepted from the interface | shared | shared | shared |
| Security: privacy | **PROVEN** after `bb1d067` and `0c0da20`: no recording, no transcript and no spoken text is logged, the settings directory holds none of them (§6.0), and a gate refuses the return of the pattern | shared | shared | shared |
| Dependency audit | **PROVEN** (Rust + JS) | shared | shared | shared |
| Contrast | **PROVEN** (measured) | shared | shared | shared |
| Screen structure | **PROVEN** (133 screens) | shared | shared | shared |
| Screen-reader reading | **BLOCKED** — needs a real AT | **BLOCKED** | **BLOCKED** | **BLOCKED** |
| Model lifecycle | **PROVEN** for every state the cache can justify: `not_installed`, `downloading` (an unfinished `<digest>.incomplete` blob), `corrupted` (an empty or unreadable weight, a dangling snapshot link), `ready`, `in_use`. A partial model is never reported as offline ready | **PROVEN** for install/verify | **PROVEN** for install/verify | pinned, not exercised |
| Performance | **MEASURED** — the round trip prints the load time, the transcription time and the real time factor (commit `577a0dd`) | same, measured on the Windows runner only | not measured | not measured |

## 5. Gates

| Gate | State |
| --- | --- |
| **T0** static | Rust format and clippy, TypeScript check, frontend lint, workflow validation, RustSec audit, OSV audit, contrast measurement, screen audit, ignored-test registration, and the refusal of any log statement that carries what the user said (`scripts/check-private-logs.py`). **All enforced in CI.** |
| **T1** unit | 410 Rust tests registered, 405 passed and 5 ignored with a stated reason, 11 audio-contract checks, TypeScript compile. **Green.** |
| **T2** real integration | Official runtime, real French model, real WAV on three platforms, a fourth generated locally, offline run, installation, cancellation, cache, real STT on a reference recording, real STT on a real capture, and a silent capture that produces no words. **Green, with the two POSIX targets of the matrix not exercised.** |
| **T3** packaged end-to-end | Built, installed, launched, stopped, uninstalled on a real desktop, no orphan process, no temporary file, user settings preserved. **Green for the automated part; the graphical journey is `BLOCKED`.** |

No critical test is skipped. The tests that do not run are not skipped — they
are `BLOCKED`, with the reason and a manual protocol recorded in
`QUALIFICATION.md` §6.

## 6. Risks and limitations

1. **The graphical user journey was never executed.** Pause, resume, stop,
   volume, selection reading, restart and cache reuse through the interface
   are unproven. The automated part covers the build, the install, the launch,
   the stop, the uninstall and the absence of leftovers, both in CI and by
   hand on a real desktop.
2. **The microphone was exercised, but not through the interface.** A real
   capture from this machine's array was transcribed by the pinned model, and
   a real silent capture produced no words. What remains unproven is
   everything the interface drives: permission refused, permission revoked,
   absent input, device change, system interruption and the shortcuts.
3. **No screen reader was ever used.** Contrast and structure are measured; the
   actual announcement order is not.
4. **Two POSIX targets are pinned but never run.** `aarch64-unknown-linux-gnu`
   and `x86_64-apple-darwin` are refused at run time rather than trusted.
5. **Three lifecycle states have no signal.** `verifying`, `failed` and
   `removing` cannot be reported, because the weights are fetched by the
   official CLI, which the application does not instrument. Reporting them
   would mean inventing them.
6. **Two JavaScript advisories are accepted**, each with a written reason and a
   condition that would invalidate it.
7. **Eight RustSec advisories are accepted**, each with a written reason.
8. **Performance is measured once, not budgeted.** The round trip prints the load time and the real time factor of the pinned model on the Windows runner, with a loose bound that only catches a pathological regression. There is no baseline across versions and no budget for the interface.

## 7. Verdict

**NO-GO.**

The reasons are concrete, not cautious:

- A packaged application whose main interface path was never driven is not a
  proven product, whatever the pipeline says. The installer now installs,
  launches, stops and uninstalls on a real desktop, and the window opens, but
  nobody has clicked through consent, download, synthesis, pause, volume and
  the settings that follow.
- The recorder has met a microphone, and that part is proven: 8.99 seconds of
  real capture, peak 24.4 % of full scale, transcribed by the pinned model,
  and a silent capture that produced no words. What remains unproven is the
  interface around it: a refused permission, a revoked permission, a device
  change mid-run, a sleep and wake, and the shortcuts.
- An accessibility claim that never met a screen reader is not proven.

The work that this environment could prove is proven, and it is deliberately
not rounded up: three real French recordings from three platforms with their
digests checked first, a fourth generated on the project owner's machine, a
real microphone capture transcribed on that same machine, 410 unit tests,
measured contrast, 133 audited screens, two audited dependency trees, and a
packaged installer that installs, starts, stops, uninstalls and leaves the
user's settings alone.

The three blocked items now have a complete, recordable protocol in
`QUALIFICATION.md`: §6 covers the packaged journey (13 steps, of which 1 and
11 are now recorded as executed in §6.0), **§6.1** covers the microphone and
speech recognition (15 scenarios, with §6.1.1 and §6.1.2 recording what was
executed outside the interface), and **§6.2** covers the screen reader
(7 scenarios). §6.3 states the rule that makes the result usable: a row that
was not run stays untested and is never counted as passed.

The verdict becomes **CONDITIONAL GO** once those rows are executed and
recorded, and **GO** once the two unexercised POSIX targets, the three
lifecycle states that have no signal, and a performance baseline are covered.
None of those require new engineering for the first two: they require a
desktop, a microphone, a screen reader, two runners, and a stopwatch.
