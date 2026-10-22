//! Local TTS components, independent from Murmure's microphone/STT engine.
// The official release publishes a build per desktop target, so the
// installer is not a Windows privilege. Only one of the two is compiled
// for a given target.
#[cfg(not(target_os = "windows"))]
mod install_posix;
#[cfg(target_os = "windows")]
mod install_windows;
pub mod models;
pub mod preview;
#[cfg(target_os = "windows")]
pub mod selection_windows;
mod serve;
mod stream;
pub mod text;

/// Start a sidecar without a console window on Windows.
///
/// Pocket TTS is a console executable. When Murmure spawns it from a GUI
/// process, Windows otherwise allocates a visible console window for the
/// lifetime of the server, and briefly for each one-shot preview.
#[cfg(target_os = "windows")]
pub(super) fn hide_console_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// No console is allocated for child processes on the other platforms.
#[cfg(not(target_os = "windows"))]
pub(super) fn hide_console_window(_command: &mut std::process::Command) {}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    /// The sidecar is a console executable. Without `CREATE_NO_WINDOW`, it
    /// inherits (or allocates) a console that stays visible while the resident
    /// server runs, and flashes for every one-shot preview.
    #[test]
    fn hide_console_window_removes_the_child_console() {
        let mut command = std::process::Command::new("powershell.exe");
        hide_console_window(&mut command);
        let output = command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Add-Type -MemberDefinition '[DllImport(\"kernel32.dll\")] public static extern System.IntPtr GetConsoleWindow();' -Name Native -Namespace Win32; [Win32.Native]::GetConsoleWindow().ToInt64()",
            ])
            .output()
            .expect("PowerShell should run");

        assert!(
            output.status.success(),
            "PowerShell failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }
}
