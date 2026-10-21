//! Explicit reading of the focused Windows UI Automation text selection.
//! No keyboard injection, clipboard access or application focus changes.
//! Untrusted providers may refuse TextPattern; fail closed without fallback.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
};

const MAX_SELECTED_CHARS: usize = 1_000;
static READ_ACTIVE: AtomicBool = AtomicBool::new(false);
// Serialize the last eligibility check with Stop: a slow UIA provider must
// never enqueue newly synthesized speech after an explicit cancellation.
static SELECTION_GENERATION: Mutex<u64> = Mutex::new(0);

pub fn invalidate_pending_selection() {
    let mut epoch = SELECTION_GENERATION.lock();
    *epoch = epoch.wrapping_add(1);
}

fn dispatch_if_current(
    generation: &Mutex<u64>,
    captured: u64,
    submit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let current = generation.lock();
    if *current != captured {
        return Err("Selection reading was cancelled".to_string());
    }
    // Keep this mutex held until the TTS command enters the queue.
    // The Stop command acquires it to invalidate all pending captures.
    submit()
}

struct ReadGuard;
impl Drop for ReadGuard {
    fn drop(&mut self) {
        READ_ACTIVE.store(false, Ordering::Release);
    }
}

struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        // Each capture runs on its own initialized thread.
        unsafe { CoUninitialize() };
    }
}

/// Run on a dedicated thread: UI Automation COM interfaces are not Send.
fn capture_selection() -> Result<String, String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|_| "Windows accessibility initialization failed")?;
        let _com = ComGuard;
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .map_err(|_| "Windows accessibility service is unavailable")?;
        let focused = automation
            .GetFocusedElement()
            .map_err(|_| "Unable to inspect the focused control")?;

        // Do not read Murmure's own editable text when the intended target has
        // lost focus. Password and offscreen controls are always excluded.
        if focused
            .CurrentProcessId()
            .map_err(|_| "Unable to verify selection origin")?
            == std::process::id() as i32
        {
            return Err("Focus is inside Murmure, not a readable external selection".into());
        }
        if focused
            .CurrentIsPassword()
            .map_err(|_| "Unable to check protected content")?
            .as_bool()
            || focused
                .CurrentIsOffscreen()
                .map_err(|_| "Unable to check visible content")?
                .as_bool()
        {
            return Err("Protected or offscreen text cannot be read".into());
        }

        let pattern: IUIAutomationTextPattern = focused
            .GetCurrentPatternAs(UIA_TextPatternId)
            .map_err(|_| "The focused application does not expose a text selection")?;
        let ranges = pattern
            .GetSelection()
            .map_err(|_| "Cannot access selected text")?;
        let len = ranges.Length().map_err(|_| "Cannot enumerate selection")?;
        if len != 1 {
            return Err("Select one contiguous passage to read aloud".into());
        }

        // Fetch one extra char to detect truncation before reading/speaking.
        let text = ranges
            .GetElement(0)
            .and_then(|range| range.GetText((MAX_SELECTED_CHARS + 1) as i32))
            .map_err(|_| "Unable to read selected text")?
            .to_string();
        // A focus change during the COM call invalidates the user's target.
        // Never read a stale selection after the foreground app changes.
        if !focused
            .CurrentHasKeyboardFocus()
            .map_err(|_| "Unable to confirm selection focus")?
            .as_bool()
        {
            return Err("Selection focus changed while reading".into());
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("The selected text is empty".into());
        }
        if trimmed.chars().count() > MAX_SELECTED_CHARS {
            return Err("The selected text exceeds the 1000-character limit".into());
        }
        Ok(trimmed.to_string())
    }
}

pub fn speak_selection(app: &AppHandle) {
    // Establish the capture generation and active flag atomically with
    // respect to Stop, so a Stop cannot interleave between those operations.
    let captured = {
        let generation = SELECTION_GENERATION.lock();
        if READ_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        *generation
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let _guard = ReadGuard;
        if let Err(detail) = capture_selection().and_then(|text| {
            dispatch_if_current(&SELECTION_GENERATION, captured, || {
                crate::tts::preview::pocket_tts_speak(app.clone(), text)
            })
        }) {
            // Emit diagnostics only, never text content or application names.
            let _ = app.emit("pocket-tts-selection-error", detail);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_limit_cannot_exceed_tts_transport_limit() {
        assert!(MAX_SELECTED_CHARS <= 1_000);
    }

    #[test]
    fn cancelled_pending_selection_never_dispatches_to_tts() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        let generation = Mutex::new(3_u64);
        let dispatched = AtomicUsize::new(0);
        // A stop raced while UIA was resolving the selected text.
        *generation.lock() = 4;
        let result = dispatch_if_current(&generation, 3, || {
            dispatched.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(dispatched.load(AtomicOrdering::SeqCst), 0);
        assert!(dispatch_if_current(&generation, 4, || {
            dispatched.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        })
        .is_ok());
        assert_eq!(dispatched.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn capture_is_not_silently_reentrant() {
        READ_ACTIVE.store(false, Ordering::Release);
        assert!(READ_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok());
        assert!(READ_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
        drop(ReadGuard);
        assert!(!READ_ACTIVE.load(Ordering::Acquire));
    }
}
