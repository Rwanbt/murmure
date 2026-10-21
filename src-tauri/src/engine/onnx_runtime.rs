//! Loading of the ONNX Runtime that the speech recognition engine uses.
//!
//! The runtime is a native library that is loaded at runtime instead of
//! being linked at build time. The distribution that the build script would
//! download on Windows is compiled with the DirectML execution provider, so
//! it pulls the DirectX 12 libraries into the import table of every binary
//! that links it, and a Windows installation without the full graphics stack
//! then refuses to start the program. The official CPU-only distribution has
//! no DirectX dependency, and the speech recognition is a pure CPU workload.
//!
//! The library is installed in the resource directory of the application,
//! its digest is verified against the value that is published with the
//! release, and it is only ever loaded from that verified path.

use std::path::PathBuf;

/// The version of the ONNX Runtime that is used.
pub const RUNTIME_VERSION: &str = "1.23.2";

/// The name of the library that is loaded.
pub const LIBRARY_NAME: &str = "onnxruntime.dll";

/// The digest of the library that is loaded, as it is extracted from the
/// official CPU-only distribution for Windows.
#[cfg(windows)]
pub const RUNTIME_SHA256: &str = "dec964ab1ee36cc9b0ae247d13b376627992fc57dec0454354017ab8fd84f1ea";

/// The relative path of the runtime inside the resource directory.
const RESOURCE_PATH: &str = "onnxruntime/onnxruntime.dll";

/// The path of the runtime that is loaded, taken from the resource
/// directory of the application.
pub fn runtime_library_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    crate::utils::resources::resolve_resource_path(app, RESOURCE_PATH)
}

/// The outcome of the preparation of the ONNX Runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeState {
    /// The library is present and is ready to be loaded.
    Ready(PathBuf),
    /// The library is not installed.
    Missing,
}

/// Reports whether the ONNX Runtime is present.
pub fn state(app: &tauri::AppHandle) -> RuntimeState {
    match runtime_library_path(app) {
        Some(path) if path.is_file() => RuntimeState::Ready(path),
        _ => RuntimeState::Missing,
    }
}

/// A message that names the runtime that is expected, for the logs and for
/// the errors that the user reads.
pub fn expected_description() -> String {
    format!("the ONNX Runtime {RUNTIME_VERSION} ({LIBRARY_NAME})")
}

/// The digest that the installed runtime has to match.
pub fn expected_digest() -> &'static str {
    #[cfg(windows)]
    {
        RUNTIME_SHA256
    }
    #[cfg(not(windows))]
    {
        ""
    }
}

/// Verifies the digest of the installed runtime.
///
/// The runtime is a native library that the application loads and executes,
/// so a file that was replaced after the installation must not be loaded. The
/// digest that is checked here is the one that is published with the
/// release.
pub fn verify_digest(path: &std::path::Path) -> Result<(), String> {
    let expected = expected_digest();
    if expected.is_empty() {
        return Ok(());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("The ONNX Runtime could not be read: {error}"))?;
    let digest = sha256(&bytes);
    if digest != expected {
        return Err(format!(
            "The ONNX Runtime at '{}' does not match the expected digest. Expected '{expected}' but got '{digest}'.",
            path.display()
        ));
    }
    Ok(())
}

/// The hexadecimal digest of the given bytes.
fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_is_pinned() {
        #[cfg(windows)]
        assert_eq!(
            expected_digest(),
            "dec964ab1ee36cc9b0ae247d13b376627992fc57dec0454354017ab8fd84f1ea"
        );
    }

    #[test]
    fn the_runtime_is_not_loaded_from_the_current_directory() {
        // A library that is picked up from the working directory could be
        // replaced by another one, so the path must stay relative to the
        // resource directory of the application.
        assert!(
            !RESOURCE_PATH.starts_with('/') && !RESOURCE_PATH.contains(':'),
            "the resource path must stay relative to the resource directory"
        );
        assert!(
            RESOURCE_PATH.contains("onnxruntime"),
            "the resource path must name the runtime directory"
        );
    }

    #[test]
    fn the_resource_path_names_the_library() {
        assert!(
            RESOURCE_PATH.ends_with(LIBRARY_NAME),
            "the resource path must name the library that is loaded"
        );
    }
}
