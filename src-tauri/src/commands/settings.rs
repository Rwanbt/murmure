use crate::overlay::overlay::{MAX_RESULT_PANEL_SECS, MIN_RESULT_PANEL_SECS};
use crate::settings::AppSettings;
use serde::Serialize;
use tauri::{command, AppHandle, Emitter, Manager};

#[command]
pub fn get_all_settings(app: AppHandle) -> Result<AppSettings, String> {
    Ok(crate::settings::load_settings(&app))
}

#[command]
pub fn get_current_language(app: AppHandle) -> Result<String, String> {
    let s = crate::settings::load_settings(&app);
    Ok(s.language)
}

#[command]
pub fn set_current_language(app: AppHandle, lang: String) -> Result<(), String> {
    const SUPPORTED_LANGUAGES: &[&str] = &["default", "en", "fr"];

    if !SUPPORTED_LANGUAGES.contains(&lang.as_str()) {
        return Err(format!("Unsupported language code: {}", lang));
    }

    let mut s = crate::settings::load_settings(&app);
    s.language = lang;
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn get_current_mic_id(app: AppHandle) -> Result<Option<String>, String> {
    let s = crate::settings::load_settings(&app);
    Ok(s.mic_id)
}

#[command]
pub fn set_current_mic_id(
    app: AppHandle,
    mic_id: Option<String>,
    mic_label: Option<String>,
) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.mic_id = mic_id.clone();
    s.mic_label = mic_label;
    crate::settings::save_settings(&app, &s)?;
    crate::audio::microphone::update_mic_cache(&app, mic_id);
    Ok(())
}

#[command]
pub fn get_current_mic_label(app: AppHandle) -> Result<Option<String>, String> {
    let s = crate::settings::load_settings(&app);
    Ok(s.mic_label)
}

#[command]
pub fn get_mic_list() -> Result<Vec<crate::audio::types::MicInfo>, String> {
    let mic_list = crate::audio::microphone::get_mic_list();
    Ok(mic_list)
}

#[command]
pub fn set_sound_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.sound_enabled = enabled;
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn set_sound_volume(app: AppHandle, percent: u8) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.sound_volume = percent.clamp(
        crate::audio::sound::MIN_SOUND_VOLUME_PERCENT,
        crate::audio::sound::MAX_SOUND_VOLUME_PERCENT,
    );
    crate::settings::save_settings(&app, &s)
}

/// The Pocket TTS engine accepts a finite catalog of local French models.
fn supported_tts_model(model: &str) -> bool {
    matches!(model, "french" | "french_24l")
}

#[command]
pub fn set_tts_model(app: AppHandle, model: String) -> Result<(), String> {
    if !supported_tts_model(&model) {
        return Err("Unsupported Pocket TTS model".to_string());
    }
    let mut settings = crate::settings::load_settings(&app);
    settings.tts_model = model;
    crate::settings::save_settings(&app, &settings)
}

/// Explicit per-installation consent for model/voice downloads.
/// Importing a .murmure backup never sets this permission.
#[command]
pub fn set_tts_model_download_approved(app: AppHandle, approved: bool) -> Result<(), String> {
    // Serializes this update with all Pocket TTS requests, including global
    // selection shortcuts. Release the lock BEFORE calling Stop, which also
    // invalidates pending UI Automation reads (opposite lock order).
    {
        let _guard = crate::tts::preview::MODEL_APPROVAL_LOCK
            .lock()
            .map_err(|_| "TTS approval state is unavailable".to_string())?;
        let mut settings = crate::settings::load_settings(&app);
        settings.tts_model_download_approved = approved;
        crate::settings::save_settings(&app, &settings)?;
    }
    if !approved {
        // Revocation also terminates an already active generation/playback.
        // A queued request will recheck the persisted approval before launch.
        crate::tts::preview::pocket_tts_stop(app)?;
    }
    Ok(())
}

#[command]
pub fn set_tts_volume(app: AppHandle, percent: u8) -> Result<(), String> {
    let mut settings = crate::settings::load_settings(&app);
    settings.tts_volume = percent.min(100);
    crate::settings::save_settings(&app, &settings)
}

#[cfg(test)]
mod pocket_tts_settings_tests {
    use super::*;

    #[test]
    fn only_supported_local_models_are_allowed() {
        assert!(supported_tts_model("french"));
        assert!(supported_tts_model("french_24l"));
        assert!(!supported_tts_model("https://example.org/weights"));
        assert!(!supported_tts_model("english"));
        assert!(!supported_tts_model(""));
    }
}

#[command]
pub fn set_lower_output_while_recording(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.lower_output_while_recording = enabled;
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn set_output_volume_while_recording(app: AppHandle, percent: u8) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.output_volume_while_recording = percent.min(crate::audio::output_volume::MAX_LOWERED_PERCENT);
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn set_keep_recordings(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.keep_recordings = enabled;
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn set_remove_hesitations(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.remove_hesitations = enabled;
    crate::settings::save_settings(&app, &s)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ResultPanelSettings {
    duration_secs: u64,
}

fn emit_result_panel_duration(app: &AppHandle, duration_secs: u64) {
    if let Some(window) = app.get_webview_window("recording_overlay") {
        let payload = ResultPanelSettings { duration_secs };
        let _ = window.emit("result-panel-settings-changed", &payload);
    }
}

#[command]
pub fn set_result_panel_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.result_panel_mode = crate::settings::normalize_result_panel_mode(&mode).to_string();
    crate::settings::save_settings(&app, &s)
}

#[command]
pub fn set_result_panel_duration(app: AppHandle, secs: u64) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.result_panel_duration_secs = secs.clamp(MIN_RESULT_PANEL_SECS, MAX_RESULT_PANEL_SECS);
    let res = crate::settings::save_settings(&app, &s);
    emit_result_panel_duration(&app, s.result_panel_duration_secs);
    res
}

#[command]
pub fn get_recordings_dir(app: AppHandle) -> Result<String, String> {
    crate::audio::helpers::ensure_recordings_dir(&app)
        .map(|dir| dir.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[command]
pub fn set_log_level(app: AppHandle, level: String) -> Result<(), String> {
    let valid_levels = ["off", "error", "warn", "info", "debug", "trace"];
    if !valid_levels.contains(&level.to_lowercase().as_str()) {
        return Err(format!("Invalid log level: {}", level));
    }

    let mut s = crate::settings::load_settings(&app);
    s.log_level = level.clone();
    crate::settings::save_settings(&app, &s)?;

    if let Ok(level_filter) = std::str::FromStr::from_str(&level) {
        log::set_max_level(level_filter);
    }

    Ok(())
}

#[command]
pub fn set_show_in_dock(app: AppHandle, show: bool) -> Result<(), String> {
    let mut s = crate::settings::load_settings(&app);
    s.show_in_dock = show;
    crate::settings::save_settings(&app, &s)
}
