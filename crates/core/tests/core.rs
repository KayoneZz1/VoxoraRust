use voxora_core::features::{self, FeatureAvailability, FeatureId};
use voxora_core::paging;
use voxora_core::models::PlaybackBehavior;
use voxora_core::settings::{AppSettings, SettingsService, VoiceChangerPreset, VoiceChangerSettings};
use voxora_core::LibraryService;

#[test]
fn settings_defaults_match_csharp() {
    let s = AppSettings::default();
    assert_eq!(s.theme, "dark");
    assert_eq!(s.language, "tr");
    let hotkey = s.radial_menu_hotkey.expect("F1 default");
    assert_eq!(hotkey.virtual_key, 0x70);
    assert_eq!(hotkey.modifiers, 0);
}

#[test]
fn settings_sanitize_clamps_values() {
    let mut s = AppSettings::default();
    s.mic_volume = 5.0;
    s.duck_factor = -1.0;
    s.max_cache_mb = 99999;
    s.engine_latency_ms = 1;
    s.theme = "neon".into();
    s.language = "de".into();
    s.voice_changer.pitch_semitones = 99;
    s.sanitize();
    assert_eq!(s.mic_volume, 1.5);
    assert_eq!(s.duck_factor, 0.0);
    assert_eq!(s.max_cache_mb, 2048);
    assert_eq!(s.engine_latency_ms, 20);
    assert_eq!(s.theme, "dark");
    assert_eq!(s.language, "tr");
    assert_eq!(s.voice_changer.pitch_semitones, 12);
}

#[test]
fn missing_radial_hotkey_gets_f1() {
    let dir = std::env::temp_dir().join(format!("voxora-core-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings-missing.json");
    std::fs::write(&path, r#"{"theme":"light","language":"tr"}"#).unwrap();
    let svc = SettingsService::new(Some(path));
    assert_eq!(svc.settings.radial_menu_hotkey.unwrap().virtual_key, 0x70);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explicit_null_radial_hotkey_stays_cleared() {
    let dir = std::env::temp_dir().join(format!("voxora-core-null-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings-null.json");
    std::fs::write(&path, r#"{"theme":"dark","radialMenuHotkey":null}"#).unwrap();
    let svc = SettingsService::new(Some(path));
    assert!(svc.settings.radial_menu_hotkey.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn radial_paging_splits_and_wraps() {
    let items: Vec<i32> = (0..18).collect();
    assert_eq!(paging::page_count(18), 3);
    assert_eq!(paging::page_items(&items, 0), (0..8).collect::<Vec<_>>());
    assert_eq!(paging::page_items(&items, 1), (8..16).collect::<Vec<_>>());
    assert_eq!(paging::page_items(&items, 2), vec![16, 17]);
    assert_eq!(paging::next_page(0, -1, 18), 2);
    assert_eq!(paging::next_page(2, 1, 18), 0);
    assert_eq!(paging::page_count(8), 1);
    assert_eq!(paging::page_items::<i32>(&[], 0), Vec::<i32>::new());
}

#[test]
fn features_defaults_and_overrides() {
    let mut s = AppSettings::default();
    assert!(features::is_enabled(&s, FeatureId::RadialMenu));
    assert!(features::is_enabled(&s, FeatureId::VoiceChanger));
    assert!(!features::is_enabled(&s, FeatureId::AiSoundAssistant));
    features::set_enabled(&mut s, FeatureId::RadialMenu, false);
    assert!(!features::is_enabled(&s, FeatureId::RadialMenu));
    assert_eq!(
        features::definition(FeatureId::LiveScenes).availability,
        FeatureAvailability::ComingSoon
    );
}

#[test]
fn library_round_trip_and_filter() {
    let dir = std::env::temp_dir().join(format!("voxora-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.json");
    let mut lib = LibraryService::new(Some(path.clone()));
    let cat = lib.add_category("Test");
    let file = dir.join("ornek.wav");
    std::fs::write(&file, b"RIFF").unwrap();
    let item = lib
        .add_sound(&file, Some(cat.id.clone()), false, Some("Örnek Ses".into()))
        .unwrap();
    lib.save();

    let reloaded = LibraryService::new(Some(path));
    assert_eq!(reloaded.sounds().len(), 1);
    assert_eq!(reloaded.sounds()[0].name, "Örnek Ses");
    assert_eq!(reloaded.filtered(None, false, "örnek").len(), 1);
    assert_eq!(reloaded.filtered(Some(&cat.id), false, "").len(), 1);
    assert_eq!(reloaded.filtered(None, true, "").len(), 0);
    assert_eq!(reloaded.sounds()[0].id, item.id);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn removing_sound_preserves_file_and_other_entries_after_reload() {
    let dir = std::env::temp_dir().join(format!("voxora-remove-{}", voxora_core::models::uuid_like()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.json");
    let file = dir.join("shared.wav");
    std::fs::write(&file, b"RIFF").unwrap();
    let mut lib = LibraryService::new(Some(path.clone()));
    let first = lib.add_sound(&file, None, false, Some("First".into())).unwrap();
    let second = lib.add_sound(&file, None, false, Some("Second".into())).unwrap();
    lib.remove_sound(&first.id);
    let reloaded = LibraryService::new(Some(path));
    assert_eq!(reloaded.sounds().len(), 1);
    assert_eq!(reloaded.sounds()[0].id, second.id);
    assert_eq!(std::fs::read(file).unwrap(), b"RIFF");
}

#[test]
fn voice_presets_map_values() {
    assert_eq!(
        VoiceChangerSettings::for_preset(VoiceChangerPreset::Chipmunk).pitch_semitones,
        12
    );
    assert_eq!(
        VoiceChangerSettings::for_preset(VoiceChangerPreset::Deep).pitch_semitones,
        -6
    );
    assert!(VoiceChangerSettings::for_preset(VoiceChangerPreset::Robot).robot_mix > 0.0);
    assert!(VoiceChangerSettings::for_preset(VoiceChangerPreset::Echo).echo_mix > 0.0);
    assert!(VoiceChangerSettings::for_preset(VoiceChangerPreset::Telephone).telephone);
}

#[test]
fn reads_csharp_library_json() {
    let dir = std::env::temp_dir().join(format!("voxora-cs-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.json");
    std::fs::write(
        &path,
        r#"{
  "version": 1,
  "categories": [{ "Id": "c1", "Name": "Oyun", "Color": null, "Icon": "\ud83c\udfae", "Order": 1 }],
  "sounds": [{
    "Id": "s1", "Name": "Kazanma", "FilePath": "C:/x/win.wav", "CategoryId": "c1",
    "Color": null, "Icon": null, "Hotkey": null, "Volume": 0.75, "IsMuted": false,
    "Behavior": 2, "HoldToPlay": true, "Favorite": true, "Order": 2,
    "AddedUtc": "2026-11-01T10:00:00Z", "LastPlayedUtc": null, "DurationSeconds": 1.5
  }],
  "nextOrder": 5
}"#,
    )
    .unwrap();
    let lib = LibraryService::new(Some(path.clone()));
    assert_eq!(lib.sounds().len(), 1);
    assert_eq!(lib.sounds()[0].behavior, PlaybackBehavior::Toggle);
    assert!(lib.sounds()[0].hold_to_play);
    assert!(lib.sounds()[0].favorite);
    assert_eq!(lib.sounds()[0].volume, 0.75);
    assert_eq!(lib.categories().len(), 1);
    assert!(!dir.join("library.json.corrupt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reads_csharp_settings_camel_case() {
    let dir = std::env::temp_dir().join(format!("voxora-cs-set-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{
  "theme": "light", "language": "en", "onboardingCompleted": true,
  "voiceChanger": { "enabled": true, "preset": "Robot", "pitchSemitones": -4,
                    "robotMix": 0.5, "echoMix": 0.2, "echoDelayMs": 200, "telephone": false },
  "radialMenuHotkey": { "Modifiers": 2, "VirtualKey": 113 },
  "micVolume": 0.8, "fxVolume": 0.9, "duckFactor": 0.4, "maxCacheMb": 512, "engineLatencyMs": 60
}"#,
    )
    .unwrap();
    let svc = SettingsService::new(Some(path.clone()));
    assert_eq!(svc.settings.theme, "light");
    assert_eq!(svc.settings.language, "en");
    assert!(svc.settings.onboarding_completed);
    assert!(svc.settings.voice_changer.enabled);
    assert_eq!(svc.settings.voice_changer.pitch_semitones, -4);
    assert_eq!(svc.settings.voice_changer.robot_mix, 0.5);
    let hk = svc.settings.radial_menu_hotkey.unwrap();
    assert_eq!(hk.virtual_key, 0x71);
    assert_eq!(hk.modifiers, 2);
    assert_eq!(svc.settings.mic_volume, 0.8);
    assert!(!dir.join("settings.json.corrupt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
