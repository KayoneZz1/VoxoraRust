use crate::models::{HotkeyGesture, PlaybackBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceChangerPreset {
    #[default]
    Natural,
    Chipmunk,
    Deep,
    Robot,
    Echo,
    Telephone,
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VoiceChangerSettings {
    pub enabled: bool,
    pub preset: VoiceChangerPreset,
    pub pitch_semitones: i32,
    pub robot_mix: f32,
    pub echo_mix: f32,
    pub echo_delay_ms: i32,
    pub telephone: bool,
}

impl Default for VoiceChangerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: VoiceChangerPreset::Natural,
            pitch_semitones: 0,
            robot_mix: 0.0,
            echo_mix: 0.0,
            echo_delay_ms: 160,
            telephone: false,
        }
    }
}

impl VoiceChangerSettings {
    pub fn normalize(&mut self) {
        self.pitch_semitones = self.pitch_semitones.clamp(-12, 12);
        self.robot_mix = self.robot_mix.clamp(0.0, 1.0);
        self.echo_mix = self.echo_mix.clamp(0.0, 1.0);
        self.echo_delay_ms = self.echo_delay_ms.clamp(50, 500);
    }

    pub fn for_preset(preset: VoiceChangerPreset) -> Self {
        let mut s = Self {
            enabled: true,
            preset,
            ..Self::default()
        };
        match preset {
            VoiceChangerPreset::Chipmunk => s.pitch_semitones = 12,
            VoiceChangerPreset::Deep => s.pitch_semitones = -6,
            VoiceChangerPreset::Robot => s.robot_mix = 0.75,
            VoiceChangerPreset::Echo => {
                s.echo_mix = 0.35;
                s.echo_delay_ms = 180;
            }
            VoiceChangerPreset::Telephone => s.telephone = true,
            _ => {}
        }
        s
    }

    pub fn clamp_copy(&self) -> Self {
        let mut s = self.clone();
        s.normalize();
        s
    }
}

fn default_radial() -> Option<HotkeyGesture> {
    Some(HotkeyGesture::new(0x70, 0))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub remember_last_category: bool,
    pub last_category_id: Option<String>,
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub auto_start: bool,
    pub copy_on_import: bool,
    pub default_behavior: i32,
    pub suppress_hotkeys: bool,
    pub features: HashMap<String, bool>,
    pub voice_changer: VoiceChangerSettings,
    pub check_for_updates: bool,
    pub last_update_check_utc: Option<String>,
    pub output_device_id: Option<String>,
    pub mic_device_id: Option<String>,
    pub monitor_device_id: Option<String>,
    pub mic_enabled: bool,
    pub mic_volume: f32,
    pub fx_volume: f32,
    pub monitor_enabled: bool,
    pub duck_mic_while_playing: bool,
    pub duck_factor: f32,
    pub stop_all_hotkey: Option<HotkeyGesture>,
    #[serde(default = "default_radial")]
    pub radial_menu_hotkey: Option<HotkeyGesture>,
    pub max_cache_mb: i32,
    pub engine_latency_ms: i32,
    pub onboarding_completed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            language: "tr".into(),
            remember_last_category: true,
            last_category_id: None,
            close_to_tray: true,
            start_minimized: false,
            auto_start: false,
            copy_on_import: true,
            default_behavior: PlaybackBehavior::Restart as i32,
            suppress_hotkeys: true,
            features: HashMap::new(),
            voice_changer: VoiceChangerSettings::default(),
            check_for_updates: true,
            last_update_check_utc: None,
            output_device_id: None,
            mic_device_id: None,
            monitor_device_id: None,
            mic_enabled: true,
            mic_volume: 1.0,
            fx_volume: 1.0,
            monitor_enabled: true,
            duck_mic_while_playing: false,
            duck_factor: 0.25,
            stop_all_hotkey: None,
            radial_menu_hotkey: default_radial(),
            max_cache_mb: 256,
            engine_latency_ms: 50,
            onboarding_completed: false,
        }
    }
}

impl AppSettings {
    pub fn sanitize(&mut self) {
        self.mic_volume = self.mic_volume.clamp(0.0, 1.5);
        self.fx_volume = self.fx_volume.clamp(0.0, 1.5);
        self.duck_factor = self.duck_factor.clamp(0.0, 1.0);
        self.max_cache_mb = self.max_cache_mb.clamp(32, 2048);
        self.engine_latency_ms = self.engine_latency_ms.clamp(20, 200);
        if self.theme != "light" && self.theme != "dark" {
            self.theme = "dark".into();
        }
        self.language = if self.language.eq_ignore_ascii_case("en") {
            "en".into()
        } else {
            "tr".into()
        };
        self.voice_changer.normalize();
    }
}

pub fn base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("VOXORA_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    appdata.join("Voxora")
}

pub fn settings_path() -> PathBuf {
    base_dir().join("settings.json")
}

pub fn library_path() -> PathBuf {
    base_dir().join("library.json")
}

pub fn sounds_dir() -> PathBuf {
    base_dir().join("Sounds")
}

pub struct SettingsService {
    pub settings: AppSettings,
    path: PathBuf,
}

impl SettingsService {
    pub fn new(path: Option<PathBuf>) -> Self {
        let path = path.unwrap_or_else(settings_path);
        let mut settings = load_json::<AppSettings>(&path).unwrap_or_default();
        settings.sanitize();
        Self { settings, path }
    }

    pub fn save(&self) {
        let _ = save_json(&self.path, &self.settings);
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let data = std::fs::read(path).ok()?;
    if data.len() > 32 * 1024 * 1024 {
        return None;
    }
    match serde_json::from_slice::<T>(&data) {
        Ok(v) => Some(v),
        Err(_) => {
            let mut corrupt = path.as_os_str().to_owned();
            corrupt.push(".corrupt");
            let _ = std::fs::copy(path, PathBuf::from(corrupt));
            None
        }
    }
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", crate::models::uuid_like()));
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(&tmp, bytes)?;
    replace_file(&tmp, path)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
    let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(source, target)
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn replaces_existing_json_without_removing_old_file_first() {
        let dir = std::env::temp_dir().join(format!("voxora-save-{}", crate::models::uuid_like()));
        let path = dir.join("settings.json");
        save_json(&path, &vec![1, 2]).unwrap();
        save_json(&path, &vec![3, 4]).unwrap();
        assert_eq!(load_json::<Vec<i32>>(&path), Some(vec![3, 4]));
    }
}
