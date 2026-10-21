//! Remembers which window had the keyboard when a recording started.
//!
//! The transcription is inserted with a `Ctrl+V` sent to whatever has the
//! focus at that moment. Recording shows an overlay and hides it again, and
//! the focus does not necessarily come back to where it was: a browser gives
//! it back to its window chrome, whose focused control is the address bar.
//! The dictated text then replaced the URL instead of landing in the field
//! that was being edited.
//!
//! The window that had the focus before the overlay appeared is therefore
//! remembered, and given back just before the paste.

/// The window that had the focus when the recording started, or zero.
#[cfg(target_os = "windows")]
static TARGET: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Remember the window that currently has the focus.
///
/// Called when a recording starts, before anything of the application is
/// shown. A window that belongs to Murmure is not remembered, because the
/// text is never meant to go back into the application itself.
pub fn remember_foreground() {
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::Ordering;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };

        let handle = unsafe { GetForegroundWindow() };
        if handle.is_null() {
            return;
        }
        let mut process = 0_u32;
        unsafe { GetWindowThreadProcessId(handle, &mut process) };
        if process == std::process::id() {
            return;
        }
        TARGET.store(handle as isize, Ordering::Release);
    }
}

/// Give the focus back to the remembered window.
///
/// Returns whether a window was restored, so the caller can wait a moment
/// before sending the paste key. The stored window is consumed either way:
/// a stale target must not be reused for the next recording.
pub fn restore_foreground() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::Ordering;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
        };

        let raw = TARGET.swap(0, Ordering::AcqRel);
        if raw == 0 {
            return false;
        }
        // Only undo a focus change that this application caused. If the
        // foreground is already a foreign window, the user moved there
        // themselves while the transcription was being prepared, and pulling
        // the focus back would send the text into the window they left.
        let current = unsafe { GetForegroundWindow() };
        if !current.is_null() {
            let mut process = 0_u32;
            unsafe { GetWindowThreadProcessId(current, &mut process) };
            if process != 0 && process != std::process::id() {
                return false;
            }
        }
        let handle = raw as *mut core::ffi::c_void;
        unsafe {
            if IsWindow(handle) == 0 {
                return false;
            }
            SetForegroundWindow(handle) != 0
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
