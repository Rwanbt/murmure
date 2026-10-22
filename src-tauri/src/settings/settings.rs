use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

use super::helpers::{migrate_legacy_paste_method, normalize_result_panel_mode};
use super::types::{
    default_tts_voice, is_supported_tts_model, is_supported_tts_voice, AppSettings,
    DEFAULT_TTS_MODEL,
};

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(format!("create_dir_all failed: {}", e));
    }
    Ok(dir.join("settings.json"))
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let path = match settings_path(app) {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };

    match fs::read_to_string(&path) {
        Ok(content) => {
            let mut settings = parse_settings(&content);
            settings.result_panel_mode =
                normalize_result_panel_mode(&settings.result_panel_mode).to_string();
            settings
        }
        Err(_) => {
            let defaults = AppSettings::default();
            let _ = save_settings(app, &defaults);
            defaults
        }
    }
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

pub fn remove_dictionary_from_settings(
    app: &AppHandle,
    mut settings: AppSettings,
) -> Result<AppSettings, String> {
    settings.dictionary = Vec::new();
    save_settings(app, &settings)?;
    Ok(settings)
}

fn parse_settings(content: &str) -> AppSettings {
    let mut value = match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => value,
        Err(_) => return AppSettings::default(),
    };
    migrate_legacy_paste_method(&mut value);
    // A file written before the voices existed carries no voice at all.
    let has_voice = value.get("tts_voice").and_then(|v| v.as_str()).is_some();
    let mut settings: AppSettings = serde_json::from_value(value).unwrap_or_default();
    // A settings file written by an older build may name a model that is no
    // longer in the catalog. Keep the rest of the preferences and fall back to
    // the light CPU model instead of failing every synthesis.
    if !is_supported_tts_model(&settings.tts_model) {
        settings.tts_model = DEFAULT_TTS_MODEL.to_string();
    }
    // The voice follows the language: a file without a voice, or with a voice
    // the catalog does not hold, gets the default voice of its language.
    if !has_voice || !is_supported_tts_voice(&settings.tts_voice) {
        settings.tts_voice = default_tts_voice(&settings.tts_model).to_string();
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::PasteMethod;

    #[test]
    fn legacy_none_paste_method_disables_auto_insert_and_keeps_other_settings() {
        for legacy in ["none", "None"] {
            let content = format!(
                r#"{{"paste_method":"{}","copy_to_clipboard":true,"api_port":4900}}"#,
                legacy
            );

            let settings = parse_settings(&content);

            assert_eq!(settings.paste_method, PasteMethod::CtrlV);
            assert!(!settings.auto_insert);
            assert!(settings.copy_to_clipboard);
            assert_eq!(settings.api_port, 4900);
        }
    }

    #[test]
    fn legacy_settings_default_to_local_french_tts() {
        let settings = parse_settings(r#"{"language":"fr","sound_volume":50}"#);
        assert_eq!(settings.tts_model, "french");
        assert_eq!(settings.tts_voice, "estelle");
        assert_eq!(settings.tts_volume, 80);
    }

    #[test]
    fn unsupported_tts_model_is_migrated_to_the_cpu_model() {
        let settings = parse_settings(r#"{"tts_model":"french_24l","tts_volume":35}"#);
        assert_eq!(settings.tts_model, DEFAULT_TTS_MODEL);
        assert_eq!(settings.tts_volume, 35);
    }

    #[test]
    fn every_catalogued_language_survives_serialization() {
        for language in crate::settings::SUPPORTED_TTS_MODELS {
            let mut settings = AppSettings::default();
            settings.tts_model = language.to_string();
            settings.tts_voice = crate::settings::default_tts_voice(language).to_string();
            let json = serde_json::to_string(&settings).unwrap();
            let restored = parse_settings(&json);
            assert_eq!(restored.tts_model, language);
            assert_eq!(
                restored.tts_voice,
                crate::settings::default_tts_voice(language)
            );
        }
    }

    /// A file written before the voices existed must not keep the voice of
    /// another language: each language speaks with its own default.
    #[test]
    fn a_legacy_file_without_a_voice_gets_the_voice_of_its_language() {
        let settings = parse_settings(r#"{"tts_model":"english"}"#);
        assert_eq!(settings.tts_model, "english");
        assert_eq!(settings.tts_voice, "alba");

        let settings = parse_settings(r#"{"tts_model":"dutch"}"#);
        assert_eq!(settings.tts_voice, "alba");
    }

    #[test]
    fn an_unknown_voice_is_replaced_by_the_default_of_the_language() {
        let settings = parse_settings(r#"{"tts_model":"german","tts_voice":"not-a-voice"}"#);
        assert_eq!(settings.tts_model, "german");
        assert_eq!(settings.tts_voice, "juergen");
    }

    #[test]
    fn tts_model_and_volume_survive_serialization() {
        let settings = AppSettings {
            tts_model: "french".to_string(),
            tts_voice: "lola".to_string(),
            tts_volume: 35,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored = parse_settings(&json);
        assert_eq!(restored.tts_model, "french");
        assert_eq!(restored.tts_voice, "lola");
        assert_eq!(restored.tts_volume, 35);
    }

    #[test]
    fn legacy_settings_preserve_explicit_shortcuts_and_fill_missing_defaults() {
        let settings = parse_settings(r#"{"record_shortcut":"ctrl+space"}"#);
        assert_eq!(settings.tts_stop_shortcut, "ctrl+alt+q");
        assert_eq!(settings.record_shortcut, "ctrl+space");
    }

    #[test]
    fn legacy_configuration_restores_and_can_disable_pause_shortcut() {
        let old = parse_settings(r#"{"tts_stop_shortcut":"ctrl+alt+k"}"#);
        assert_eq!(old.tts_pause_shortcut, "ctrl+alt+p");
        let mut custom = old;
        custom.tts_pause_shortcut = "ctrl+shift+p".to_string();
        let persisted = parse_settings(&serde_json::to_string(&custom).unwrap());
        assert_eq!(persisted.tts_pause_shortcut, "ctrl+shift+p");
        custom.tts_pause_shortcut.clear();
        assert!(parse_settings(&serde_json::to_string(&custom).unwrap())
            .tts_pause_shortcut
            .is_empty());
    }

    #[test]
    fn tts_stop_hotkey_survives_settings_round_trip_and_can_be_disabled() {
        let mut settings = AppSettings::default();
        settings.tts_stop_shortcut = "ctrl+shift+q".to_string();
        let json = serde_json::to_string(&settings).unwrap();
        let restored = parse_settings(&json);
        assert_eq!(restored.tts_stop_shortcut, "ctrl+shift+q");

        settings.tts_stop_shortcut.clear();
        let disabled = parse_settings(&serde_json::to_string(&settings).unwrap());
        assert!(disabled.tts_stop_shortcut.is_empty());
    }

    #[test]
    fn legacy_settings_never_implicitly_authorize_tts_model_downloads() {
        let settings = parse_settings(r#"{"language":"fr","sound_volume":50}"#);
        assert!(!settings.tts_model_download_approved);
    }

    #[test]
    fn local_tts_download_approval_can_be_revoked_and_persisted() {
        let mut settings = AppSettings::default();
        settings.tts_model_download_approved = true;
        let serialized = serde_json::to_string(&settings).unwrap();
        assert!(parse_settings(&serialized).tts_model_download_approved);
        settings.tts_model_download_approved = false;
        let revoked = serde_json::to_string(&settings).unwrap();
        assert!(!parse_settings(&revoked).tts_model_download_approved);
    }

    #[test]
    fn missing_auto_insert_defaults_to_true() {
        let settings = parse_settings(r#"{"paste_method":"direct"}"#);

        assert_eq!(settings.paste_method, PasteMethod::Direct);
        assert!(settings.auto_insert);
    }
}
