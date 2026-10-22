use serde::{Deserialize, Serialize};

/// Pocket TTS languages accepted by the backend and the UI.
///
/// These are the seven languages the official Pocket TTS release publishes,
/// and every one of them belongs to the 25-language set of the Parakeet
/// TDT 0.6B v3 model, so the transcription and the speech synthesis cover
/// the same languages. The identifier is the one the pinned CLI expects
/// after `--language`, and it is also the directory that holds the weights.
pub const SUPPORTED_TTS_MODELS: [&str; 7] = [
    "english",
    "french",
    "german",
    "portuguese",
    "italian",
    "spanish",
    "dutch",
];
/// The language used for new settings and for legacy values that are no
/// longer part of the catalog.
pub const DEFAULT_TTS_MODEL: &str = "french";

/// The voices the official Pocket TTS release publishes for every language.
pub const SUPPORTED_TTS_VOICES: [&str; 27] = [
    "alba",
    "anna",
    "azelma",
    "bill_boerst",
    "caro_davy",
    "charles",
    "cosette",
    "daan",
    "eponine",
    "estelle",
    "eve",
    "fantine",
    "george",
    "giovanni",
    "jane",
    "javert",
    "jean",
    "juergen",
    "lola",
    "marius",
    "mary",
    "michael",
    "paul",
    "peter_yearsley",
    "rafael",
    "stuart_bell",
    "vera",
];
/// The voice used when the settings carry no voice.
pub const DEFAULT_TTS_VOICE: &str = "estelle";

pub fn is_supported_tts_model(model: &str) -> bool {
    SUPPORTED_TTS_MODELS.contains(&model)
}

pub fn is_supported_tts_voice(voice: &str) -> bool {
    SUPPORTED_TTS_VOICES.contains(&voice)
}

/// The voice a language speaks with when the user has not chosen one.
///
/// These are the defaults the pinned runtime itself declares: the CLI
/// documents `giovanni` for Italian, `lola` for Spanish, `juergen` for
/// German, `rafael` for Portuguese, `estelle` for French and `alba` for
/// every other language.
pub fn default_tts_voice(language: &str) -> &'static str {
    match language {
        "italian" => "giovanni",
        "spanish" => "lola",
        "german" => "juergen",
        "portuguese" => "rafael",
        "french" => "estelle",
        _ => "alba",
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    #[default]
    #[serde(alias = "CtrlV")]
    CtrlV,
    #[serde(alias = "CtrlShiftV")]
    CtrlShiftV,
    #[serde(alias = "Direct")]
    Direct,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct OnboardingState {
    pub used_home_shortcut: bool,
    pub transcribed_outside_app: bool,
    pub added_dictionary_word: bool,
    pub congrats_dismissed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AppSettings {
    pub record_shortcut: String,
    pub last_transcript_shortcut: String,
    pub command_shortcut: String,
    pub llm_mode_1_shortcut: String,
    pub llm_mode_2_shortcut: String,
    pub llm_mode_3_shortcut: String,
    pub llm_mode_4_shortcut: String,
    pub llm_transform_1_shortcut: String,
    pub llm_transform_2_shortcut: String,
    pub llm_transform_3_shortcut: String,
    pub llm_transform_4_shortcut: String,
    pub voice_mode_toggle_shortcut: String,
    pub tts_stop_shortcut: String,
    pub tts_pause_shortcut: String,
    pub tts_read_selection_shortcut: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dictionary: Vec<String>,
    pub record_mode: String,      // "push_to_talk" | "toggle_to_talk"
    pub overlay_mode: String,     // "hidden" | "recording" | "always"
    pub overlay_position: String, // "top" | "bottom"
    pub api_enabled: bool,
    pub api_port: u16,
    pub copy_to_clipboard: bool, // Keeps transcription in clipboard after recording finishes
    pub paste_method: PasteMethod,
    pub auto_insert: bool,
    pub persist_history: bool, // Persists last 5 transcriptions to disk
    pub language: String,      // UI language code (e.g., "en", "fr")
    pub sound_enabled: bool,
    pub sound_volume: u8,
    pub tts_model: String, // supported language id; normalized on load
    pub tts_voice: String, // supported voice id; normalized on load
    pub tts_volume: u8,    // 0..100; applied to the next playback
    pub tts_model_download_approved: bool, // explicit local approval, never migrated from exports
    pub lower_output_while_recording: bool,
    pub output_volume_while_recording: u8,
    pub onboarding: OnboardingState,
    pub cancel_shortcut: String,
    pub mic_id: Option<String>,
    pub mic_label: Option<String>, // Persisted so the disconnected-mic UI can still show a friendly name
    pub log_level: String,         // "info" | "debug" | "trace" | "warn" | "error"
    // Debug aid: keep the recorded WAV files in the system temp dir after
    // transcription instead of deleting them.
    pub keep_recordings: bool,
    pub wake_word_enabled: bool,
    pub wake_word_record: String,
    pub wake_word_command: String,
    pub wake_word_cancel: String,
    pub wake_word_validate: String,
    pub wake_word_submit: String,
    pub auto_enter_after_wake_word: bool,
    pub silence_timeout_ms: u64,
    pub show_in_dock: bool,
    pub smartmic_enabled: bool,
    pub smartmic_port: u16,
    pub smartmic_relay_enabled: bool, // Relay URL is ignored when false
    pub smartmic_relay_url: Option<String>, // e.g. "https://smartmic.hospital.com"
    pub smartmic_machine_id_enabled: bool, // Gates inclusion of machine_id in the relay URL
    pub smartmic_machine_id: Option<String>,
    pub smartmic_token_ttl_hours: Option<u64>, // None or 0 means infinite
    pub smartmic_bind_address: Option<String>, // None means auto-detect
    pub streaming_preview: bool,
    pub overlay_size: String, // "small" | "medium" | "large"
    pub streaming_text_width: u32,
    pub streaming_font_size: u32,
    pub streaming_max_lines: u32,
    pub result_panel_mode: String, // "off" | "command" | "command_llm" | "all"
    pub result_panel_duration_secs: u64,
    pub remove_hesitations: bool,
    // Linux only. Persists the user's dismissal of `WaylandModeNotice`
    // so the onboarding banner does not reappear on next launch.
    pub wayland_notice_dismissed: bool,
    // Linux only. Persists the user's dismissal of the Wayland clipboard
    // fallback onboarding card (manual Ctrl+V hint when auto-paste fails).
    pub wayland_clipboard_fallback_dismissed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            record_shortcut: "ctrl+alt+x".to_string(),
            last_transcript_shortcut: "ctrl+shift+space".to_string(),
            command_shortcut: "ctrl+shift+x".to_string(),
            llm_mode_1_shortcut: "ctrl+shift+1".to_string(),
            llm_mode_2_shortcut: "ctrl+shift+2".to_string(),
            llm_mode_3_shortcut: "ctrl+shift+3".to_string(),
            llm_mode_4_shortcut: "ctrl+shift+4".to_string(),
            llm_transform_1_shortcut: "ctrl+alt+shift+1".to_string(),
            llm_transform_2_shortcut: "ctrl+alt+shift+2".to_string(),
            llm_transform_3_shortcut: "ctrl+alt+shift+3".to_string(),
            llm_transform_4_shortcut: "ctrl+alt+shift+4".to_string(),
            voice_mode_toggle_shortcut: "ctrl+shift+0".to_string(),
            tts_stop_shortcut: "ctrl+alt+q".to_string(),
            tts_pause_shortcut: "ctrl+alt+p".to_string(),
            tts_read_selection_shortcut: "ctrl+alt+w".to_string(),
            dictionary: Vec::new(),
            // Wayland CLI mode binds shortcuts at the OS level, so Murmure receives no key-release events. Push-to-talk is impossible.
            record_mode: {
                #[cfg(target_os = "linux")]
                {
                    if crate::utils::platform::is_wayland_session() {
                        "toggle_to_talk".to_string()
                    } else {
                        "push_to_talk".to_string()
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    "push_to_talk".to_string()
                }
            },
            // gtk-layer-shell handles focus/positioning only on wlr-layer-shell
            // compositors. Mutter (GNOME) and Muffin (Cinnamon) lack it, so the
            // Tauri fallback misplaces the overlay and steals focus there.
            overlay_mode: {
                #[cfg(target_os = "linux")]
                {
                    if crate::utils::platform::is_wayland_session()
                        && !crate::utils::platform::is_portal_reliable_desktop()
                    {
                        "hidden".to_string()
                    } else {
                        "recording".to_string()
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    "recording".to_string()
                }
            },
            overlay_position: "bottom".to_string(),
            api_enabled: false,
            api_port: 4800,
            // Default to true on Wayland so transcriptions remain accessible
            // via manual Ctrl+V when enigo's key injection cannot reach native
            // Wayland apps. Users remain free to disable it.
            copy_to_clipboard: {
                #[cfg(target_os = "linux")]
                {
                    crate::utils::platform::is_wayland_session()
                }
                #[cfg(not(target_os = "linux"))]
                {
                    false
                }
            },
            paste_method: PasteMethod::default(),
            auto_insert: true,
            persist_history: false,
            language: "default".to_string(),
            sound_enabled: true,
            sound_volume: 50,
            tts_model: DEFAULT_TTS_MODEL.to_string(),
            tts_voice: DEFAULT_TTS_VOICE.to_string(),
            tts_volume: 80,
            tts_model_download_approved: false,
            lower_output_while_recording: false,
            output_volume_while_recording: 50,
            onboarding: OnboardingState::default(),
            cancel_shortcut: "ctrl+alt+s".to_string(),
            mic_id: None,
            mic_label: None,
            log_level: "info".to_string(),
            keep_recordings: false,
            wake_word_enabled: false,
            wake_word_record: "ok alix".to_string(),
            wake_word_command: "alix command".to_string(),
            wake_word_cancel: "alix cancel".to_string(),
            wake_word_validate: "alix validate".to_string(),
            wake_word_submit: "thank you alix".to_string(),
            auto_enter_after_wake_word: false,
            silence_timeout_ms: 1500,
            show_in_dock: true,
            smartmic_enabled: false,
            smartmic_port: 4801,
            smartmic_relay_enabled: false,
            smartmic_relay_url: None,
            smartmic_machine_id_enabled: false,
            smartmic_machine_id: None,
            smartmic_token_ttl_hours: None,
            smartmic_bind_address: None,
            streaming_preview: true,
            overlay_size: "small".to_string(),
            streaming_text_width: 450,
            streaming_font_size: 11,
            streaming_max_lines: 5,
            result_panel_mode: "command".to_string(),
            result_panel_duration_secs: 5,
            remove_hesitations: false,
            wayland_notice_dismissed: false,
            wayland_clipboard_fallback_dismissed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_shortcut_survives_serde_round_trip() {
        let mut settings = AppSettings::default();
        settings.record_shortcut = String::new();

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.record_shortcut, "");
    }

    #[test]
    fn the_tts_catalog_lists_every_language_pocket_tts_publishes() {
        assert_eq!(SUPPORTED_TTS_MODELS.len(), 7);
        for language in [
            "english",
            "french",
            "german",
            "portuguese",
            "italian",
            "spanish",
            "dutch",
        ] {
            assert!(is_supported_tts_model(language), "{language} is missing");
        }
        assert!(!is_supported_tts_model("french_24l"));
        assert!(!is_supported_tts_model("german_24l"));
        assert!(!is_supported_tts_model("https://example.org/weights"));
        assert!(!is_supported_tts_model(""));
    }

    #[test]
    fn the_voice_catalog_lists_every_voice_pocket_tts_publishes() {
        assert_eq!(SUPPORTED_TTS_VOICES.len(), 27);
        for voice in [
            "alba", "daan", "estelle", "giovanni", "juergen", "lola", "rafael",
        ] {
            assert!(is_supported_tts_voice(voice), "{voice} is missing");
        }
        assert!(!is_supported_tts_voice(""));
        assert!(!is_supported_tts_voice(
            "https://example.org/voice.safetensors"
        ));
    }

    #[test]
    fn every_language_has_a_default_voice_from_the_catalog() {
        for language in SUPPORTED_TTS_MODELS {
            let voice = default_tts_voice(language);
            assert!(
                is_supported_tts_voice(voice),
                "{language} has no catalogued default voice"
            );
        }
        assert_eq!(default_tts_voice("english"), "alba");
        assert_eq!(default_tts_voice("french"), "estelle");
        assert_eq!(default_tts_voice("dutch"), "alba");
        assert_eq!(
            default_tts_voice("unknown"),
            "alba",
            "an unknown language falls back to the runtime default"
        );
    }
}
