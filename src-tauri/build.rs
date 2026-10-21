//! Build script of the application.
//!
//! The application is built for Windows, where the window library of the
//! framework imports the task dialog of the common controls version 6. That
//! library lives in a side-by-side assembly, so the program has to declare
//! the dependency in its manifest. Without that declaration, the loader
//! resolves the version 5 of the library, which does not export the task
//! dialog, and the process refuses to start with STATUS_ENTRYPOINT_NOT_FOUND.
//!
//! The framework embeds that manifest in the application, through the
//! `WindowsAttributes` of the build helper, but the test binaries are
//! produced by the test harness of the compiler and carry no manifest at
//! all. The manifest is therefore embedded here for every linked target, so
//! a test binary starts exactly like the application does.

fn main() {
    tauri_build::build();

    #[cfg(target_os = "windows")]
    embed_common_controls_manifest();
}

/// Writes the manifest that declares the common controls version 6 and
/// embeds it in the linked binaries.
#[cfg(target_os = "windows")]
fn embed_common_controls_manifest() {
    use std::{env, fs, path::PathBuf};

    // The same declaration as the one the framework embeds in the
    // application, so both binaries ask the loader for the same library.
    const MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#;

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("the output directory is always set"));
    let manifest_path = out_dir.join("murmure-common-controls.manifest");
    fs::write(&manifest_path, MANIFEST).expect("the manifest can be written");

    // The manifest is merged with the one the linker generates and embedded
    // in the binary, so the loader reads it from the binary itself.
    println!(
        "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rerun-if-changed=build.rs");
}
