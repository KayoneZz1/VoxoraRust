use crate::lang::Lang;
use crate::platform::{self, HotkeyAction, HotkeyEvent, Hotkeys, UpdateInfo};
use crate::theme::{self, Palette, ThemeMode};
use crate::widgets;
use eframe::CreationContext;
use egui::{
    Align, Align2, Color32, Frame, Layout, Margin, Rect, RichText, Sense,
    Stroke, StrokeKind, TextEdit, pos2, vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use voxora_audio::{decode_file, AudioEngine, DeviceInfo};
use voxora_core::features::{self, FeatureAvailability, FeatureId};
use voxora_core::models::{HotkeyGesture, PlaybackBehavior, SoundItem};
use voxora_core::settings::{AppSettings, SettingsService, VoiceChangerPreset, VoiceChangerSettings};
use voxora_core::LibraryService;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const I_ADD: char = '\u{E710}';
const I_FOLDER: char = '\u{E8B7}';
const I_EXPORT: char = '\u{E898}';
const I_SETTINGS: char = '\u{E713}';
const I_SEARCH: char = '\u{E721}';
const I_STAR: char = '\u{E735}';
const I_VOLUME: char = '\u{E767}';
const I_MUTE: char = '\u{E74F}';
const I_STOP: char = '\u{E71A}';
const I_INFO: char = '\u{E946}';
const I_MUSIC: char = '\u{E8D6}';
const I_IMPORT: char = '\u{E8B5}';
const I_EXPORT_SOUNDS: char = '\u{E7C3}';
const I_MIC: char = '\u{E720}';
const I_CHECK: char = '\u{E73E}';
const I_REFRESH: char = '\u{E72C}';
const I_CLOSE: char = '\u{E711}';
const I_DELETE: char = '\u{E74D}';

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Settings,
    Onboarding,
    Guide,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Audio,
    Behavior,
    Features,
    Updates,
    About,
}

#[derive(Clone, PartialEq, Eq)]
pub enum CategorySel {
    All,
    Favorites,
    Id(String),
}

#[derive(Clone)]
struct Toast {
    text: String,
    kind: widgets::ToastKind,
    at: Instant,
}

enum Dialog {
    CategoryNew {
        name: String,
    },
    CategoryRename {
        id: String,
        name: String,
    },
    CategoryDelete {
        id: String,
        name: String,
    },
    SoundRemove {
        id: String,
        name: String,
    },
    SoundEdit {
        id: String,
        name: String,
        hotkey: Option<HotkeyGesture>,
        behavior: i32,
        hold: bool,
        favorite: bool,
        category: Option<String>,
        icon: String,
        color: String,
        file_path: String,
        volume: f32,
        muted: bool,
    },
}


enum CardAction {
    ToggleFavorite,
    Remove,
    Mute,
    Play,
    Edit,
    Locate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HotkeyTarget {
    StopAll,
    Radial,
}

pub struct RadialShared {
    pub overlay: crate::radial_overlay::RadialOverlay,
    pub items: std::sync::Mutex<Vec<(String, String)>>,
}

pub static RADIAL_SHARED: std::sync::OnceLock<RadialShared> = std::sync::OnceLock::new();

pub fn radial_shared() -> &'static RadialShared {
    RADIAL_SHARED.get().expect("radial shared")
}

/// F1 / tray: UI karesini beklemeden doğrudan aç/kapat.
pub fn radial_toggle() {
    let s = radial_shared();
    if s.overlay.is_open() {
        s.overlay.hide();
        return;
    }
    let items = s.items.lock().map(|v| v.clone()).unwrap_or_default();
    if items.is_empty() {
        return;
    }
    let (x, y) = platform::cursor_pos();
    s.overlay.show(items, x, y);
}

pub struct VoxoraApp {
    pub settings: SettingsService,
    pub library: LibraryService,
    pub engine: AudioEngine,
    pub lang: Lang,
    pub mode: ThemeMode,
    pub view: View,
    pub tab: SettingsTab,
    pub draft: Option<AppSettings>,
    pub category: CategorySel,
    pub search: String,
    pub filtered: Vec<SoundItem>,
    pub playing: HashSet<String>,
    pub decoded: HashMap<String, Arc<Vec<f32>>>,
    pub levels: (f32, f32),
    pub devices_out: Vec<DeviceInfo>,
    pub devices_in: Vec<DeviceInfo>,
    toasts: Vec<Toast>,
    rename_rx: Option<std::sync::mpsc::Receiver<crate::rename::RenameOutcome>>,
    dialog: Option<Dialog>,
    dialog_capture: bool,
    holding: Vec<(String, HotkeyGesture)>,
    hotkey_conflicts: Vec<String>,
    decoded_bytes: usize,
    decoded_order: std::collections::VecDeque<String>,
    device_rx: Option<std::sync::mpsc::Receiver<(Vec<DeviceInfo>, Vec<DeviceInfo>)>>,
    focus_search: bool,
    tray: crate::tray::Tray,
    allow_close: bool,
    update_rx: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>>,
    update_progress: Option<std::sync::mpsc::Receiver<(u64, u64)>>,
    update_received: u64,
    update_total: u64,
    start_hidden_done: bool,
    engine_ready: bool,
    pub hotkeys: Hotkeys,
    pub update: Option<UpdateInfo>,
    pub status: String,
    pub onboarding_step: usize,
    hotkey_capture: Option<HotkeyTarget>,
    pub dirty_at: Option<Instant>,
    pub last_save: Instant,
    pub show_update_banner: bool,
    first_frame_done: bool,
}

impl VoxoraApp {
    pub fn new(
        cc: &CreationContext,
        data_dir: Option<PathBuf>,
        theme_override: Option<ThemeMode>,
        page: Option<String>,
    ) -> Self {
        if let Some(dir) = data_dir {
            unsafe {
                std::env::set_var("VOXORA_DATA_DIR", dir);
            }
        }
        let settings = SettingsService::new(None);
        let library = LibraryService::new(None);
        let mut mode = if settings.settings.theme == "light" {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        if let Some(override_mode) = theme_override {
            mode = override_mode;
        }
        theme::install_fonts(&cc.egui_ctx);
        platform::set_repaint_context(cc.egui_ctx.clone());
        theme::apply(&cc.egui_ctx, mode);
        let (c1, c2, c3) = platform::titlebar_colors(mode == ThemeMode::Dark);
        platform::apply_titlebar(mode == ThemeMode::Dark, c1, c2, c3);

        let lang = Lang::new(&settings.settings.language);
        let engine = AudioEngine::new();
        let status = "hazır".to_string();
        let voice = settings.settings.voice_changer.clamp_copy();
        let voice = if features::is_enabled(&settings.settings, FeatureId::VoiceChanger) {
            voice
        } else {
            VoiceChangerSettings {
                enabled: false,
                ..voice
            }
        };
        engine.set_voice_settings(&voice);
        engine.configure_mix(
            settings.settings.mic_enabled,
            settings.settings.mic_volume,
            settings.settings.fx_volume,
            settings.settings.duck_mic_while_playing,
            settings.settings.duck_factor,
        );

        let devices_out = voxora_audio::output_devices();
        let devices_in = voxora_audio::input_devices();
        platform::install_wheel_interceptor();
        let hotkeys = Hotkeys::start();
        let mut view = if settings.settings.onboarding_completed {
            View::Main
        } else {
            View::Onboarding
        };
        let start_tab = SettingsTab::General;
        if let Some(page) = page.as_deref() {
            match page {
                "settings" => view = View::Settings,
                "guide" => view = View::Guide,
                "onboarding" => view = View::Onboarding,
                "main" => view = View::Main,
                _ => {}
            }
        }
        let restored_category = {
            if settings.settings.remember_last_category {
                match settings.settings.last_category_id.as_deref() {
                    Some("fav") => CategorySel::Favorites,
                    Some("all") | None => CategorySel::All,
                    Some(id) => CategorySel::Id(id.to_string()),
                }
            } else {
                CategorySel::All
            }
        };
        let initial_draft = if view == View::Settings {
            Some(settings.settings.clone())
        } else {
            None
        };
        if RADIAL_SHARED.get().is_none() {
            let _ = RADIAL_SHARED.set(RadialShared {
                overlay: crate::radial_overlay::RadialOverlay::start(),
                items: std::sync::Mutex::new(Vec::new()),
            });
            platform::set_radial_handler(Box::new(radial_toggle));
        }
        let mut app = Self {
            settings,
            library,
            engine,
            lang,
            mode,
            view,
            tab: start_tab,
            draft: initial_draft,
            category: restored_category,
            search: String::new(),
            filtered: Vec::new(),
            playing: HashSet::new(),
            decoded: HashMap::new(),
            levels: (0.0, 0.0),
            devices_out,
            devices_in,
            toasts: Vec::new(),
            rename_rx: None,
            dialog: None,
            dialog_capture: false,
            holding: Vec::new(),
            hotkey_conflicts: Vec::new(),
            decoded_bytes: 0,
            decoded_order: std::collections::VecDeque::new(),
            device_rx: if std::env::var("VOXORA_NO_WATCH").is_ok() {
                None
            } else {
                Some(spawn_device_watcher())
            },
            focus_search: false,
            tray: crate::tray::Tray::start("Voxora — ses konsolu"),
            allow_close: false,
            update_rx: None,
            update_progress: None,
            update_received: 0,
            update_total: 0,
            start_hidden_done: false,
            engine_ready: false,
            hotkeys,
            update: None,
            status,
            onboarding_step: 0,
            hotkey_capture: None,
            dirty_at: None,
            last_save: Instant::now(),
            show_update_banner: false,
            first_frame_done: false,
        };
        app.refresh_filtered();
        app.rebuild_hotkeys();
        app
    }

    fn palette(&self) -> Palette {
        theme::palette(self.mode)
    }

    fn t(&self, key: &str) -> String {
        self.lang.t(key)
    }

    fn refresh_filtered(&mut self) {
        let (cat, fav) = match &self.category {
            CategorySel::All => (None, false),
            CategorySel::Favorites => (None, true),
            CategorySel::Id(id) => (Some(id.as_str()), false),
        };
        self.filtered = self.library.filtered(cat, fav, &self.search);
        if let Some(s) = RADIAL_SHARED.get() {
            let source: &[SoundItem] = if self.filtered.is_empty() {
                &self.library.data.sounds
            } else {
                &self.filtered
            };
            if let Ok(mut items) = s.items.lock() {
                *items = source
                    .iter()
                    .map(|x| (x.id.clone(), x.name.clone()))
                    .collect();
            }
        }
    }

    fn save_now(&mut self) {
        if self.settings.settings.remember_last_category {
            self.settings.settings.last_category_id = Some(match &self.category {
                CategorySel::All => "all".to_string(),
                CategorySel::Favorites => "fav".to_string(),
                CategorySel::Id(id) => id.clone(),
            });
        }
        self.settings.save();
        self.library.save();
        self.dirty_at = None;
        self.last_save = Instant::now();
    }

    fn mark_dirty(&mut self) {
        self.dirty_at = Some(Instant::now());
    }

    fn toast(&mut self, text: impl Into<String>, kind: widgets::ToastKind) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            at: Instant::now(),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn rebuild_hotkeys(&mut self) {
        let mut map = HashMap::new();
        let mut conflicts: Vec<String> = Vec::new();
        if let Some(stop) = &self.settings.settings.stop_all_hotkey {
            if stop.is_valid() {
                map.insert(stop.clone(), HotkeyAction::StopAll);
            }
        }
        if features::is_enabled(&self.settings.settings, FeatureId::RadialMenu) {
            if let Some(radial) = &self.settings.settings.radial_menu_hotkey {
                if radial.is_valid() {
                    map.insert(radial.clone(), HotkeyAction::Radial);
                }
            }
        }
        for sound in &self.library.data.sounds {
            if let Some(hk) = &sound.hotkey {
                if hk.is_valid() {
                    if map.contains_key(hk) {
                        conflicts.push(sound.name.clone());
                    } else {
                        map.insert(hk.clone(), HotkeyAction::Play(sound.id.clone()));
                    }
                }
            }
        }
        if conflicts != self.hotkey_conflicts {
            if !conflicts.is_empty() {
                let text = self.msg(
                    &format!("Kısayol çakışması: {}. İlk ses öncelikli.", conflicts.join(", ")),
                    &format!("Hotkey conflict: {}. First sound wins.", conflicts.join(", ")),
                );
                self.toast(text, widgets::ToastKind::Warning);
            }
            self.hotkey_conflicts = conflicts;
        }
        self.hotkeys
            .update(map, self.settings.settings.suppress_hotkeys);
    }

    fn find_sound(&self, id: &str) -> Option<SoundItem> {
        self.library.data.sounds.iter().find(|s| s.id == id).cloned()
    }

    fn update_sound<F: FnOnce(&mut SoundItem)>(&mut self, id: &str, f: F) {
        let mut updated: Option<SoundItem> = None;
        if let Some(sound) = self.library.data.sounds.iter_mut().find(|s| s.id == id) {
            f(sound);
            updated = Some(sound.clone());
        }
        if let Some(item) = updated {
            if let Some(sound) = self.filtered.iter_mut().find(|s| s.id == id) {
                *sound = item;
            }
        }
        self.mark_dirty();
    }

    fn cache_insert(&mut self, id: String, samples: Arc<Vec<f32>>) {
        let bytes = samples.len() * std::mem::size_of::<f32>();
        if let Some(old) = self.decoded.remove(&id) {
            self.decoded_bytes = self.decoded_bytes.saturating_sub(old.len() * 4);
        }
        self.decoded_bytes += bytes;
        self.decoded.insert(id.clone(), samples);
        self.decoded_order.retain(|k| k != &id);
        self.decoded_order.push_back(id);
        let limit = (self.settings.settings.max_cache_mb.max(32) as usize) * 1024 * 1024;
        while self.decoded_bytes > limit {
            let Some(oldest) = self.decoded_order.pop_front() else {
                break;
            };
            if let Some(old) = self.decoded.remove(&oldest) {
                self.decoded_bytes = self.decoded_bytes.saturating_sub(old.len() * 4);
            }
        }
    }

    fn touch_cache(&mut self, id: &str) {
        self.decoded_order.retain(|k| k != id);
        self.decoded_order.push_back(id.to_string());
    }

    fn trigger_sound(&mut self, sound: &SoundItem) {
        if !self.engine.is_running() {
            self.restart_engine();
            if !self.engine.is_running() {
                return;
            }
        }

        let is_playing = self.engine.is_playing(&sound.id);
        match sound.behavior {
            PlaybackBehavior::Toggle if is_playing => {
                self.engine.stop_id(&sound.id);
                self.playing.remove(&sound.id);
                return;
            }
            PlaybackBehavior::Restart if is_playing => {
                self.engine.stop_id(&sound.id);
            }
            _ => {}
        }

        let samples = if let Some(cached) = self.decoded.get(&sound.id).cloned() {
            self.touch_cache(&sound.id);
            cached
        } else {
            match decode_file(std::path::Path::new(&sound.file_path)) {
                Ok(decoded) => {
                    let arc = Arc::new(decoded.samples);
                    let duration = decoded.duration_seconds;
                    self.cache_insert(sound.id.clone(), arc.clone());
                    if sound.duration_seconds <= 0.0 {
                        self.update_sound(&sound.id, |s| s.duration_seconds = duration);
                    }
                    arc
                }
                Err(e) => {
                    self.toast(
                        self.lang
                            .tf("Err_DecodeFailed", &[&sound.name, &e.to_string()]),
                        widgets::ToastKind::Error,
                    );
                    return;
                }
            }
        };

        let volume = if sound.is_muted { 0.0 } else { sound.volume };
        self.engine.play(&sound.id, samples, volume);
        self.playing.insert(sound.id.clone());
    }

    fn stop_sound(&mut self, id: &str) {
        self.engine.stop_id(id);
        self.playing.remove(id);
    }

    fn play_test_tone(&mut self) {
        let rate = 48_000.0f32;
        let frames = (rate * 0.8) as usize;
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let env = if i < 2400 {
                i as f32 / 2400.0
            } else if frames - i < 4800 {
                (frames - i) as f32 / 4800.0
            } else {
                1.0
            };
            let v = (2.0 * std::f32::consts::PI * 660.0 * i as f32 / rate).sin() * 0.22 * env;
            samples.push(v);
            samples.push(v);
        }
        self.engine.play("__test_tone__", Arc::new(samples), 1.0);
        self.toast(self.t("Onboarding_TestTonePlayed"), widgets::ToastKind::Info);
    }

    fn restart_engine(&mut self) {
        let voice = self.settings.settings.voice_changer.clamp_copy();
        let voice = if features::is_enabled(&self.settings.settings, FeatureId::VoiceChanger) {
            voice
        } else {
            VoiceChangerSettings {
                enabled: false,
                ..voice
            }
        };
        let mic = if self.settings.settings.mic_enabled {
            self.settings.settings.mic_device_id.as_deref()
        } else {
            None
        };
        self.engine
            .configure_latency(self.settings.settings.engine_latency_ms as u32);
        let monitor = if self.settings.settings.monitor_enabled {
            self.settings.settings.monitor_device_id.as_deref()
        } else {
            None
        };
        match self
            .engine
            .start(self.settings.settings.output_device_id.as_deref(), monitor, mic)
        {
            Ok(()) => {
                self.engine.set_voice_settings(&voice);
                self.engine.configure_mix(
                    self.settings.settings.mic_enabled,
                    self.settings.settings.mic_volume,
                    self.settings.settings.fx_volume,
                    self.settings.settings.duck_mic_while_playing,
                    self.settings.settings.duck_factor,
                );
                self.status = "hazır".into();
            }
            Err(e) => {
                self.status = e.to_string();
                self.toast(e.to_string(), widgets::ToastKind::Error);
            }
        }
    }

    fn apply_engine_settings(&mut self) {
        self.engine.configure_mix(
            self.settings.settings.mic_enabled,
            self.settings.settings.mic_volume,
            self.settings.settings.fx_volume,
            self.settings.settings.duck_mic_while_playing,
            self.settings.settings.duck_factor,
        );
        let voice = self.settings.settings.voice_changer.clamp_copy();
        let voice = if features::is_enabled(&self.settings.settings, FeatureId::VoiceChanger) {
            voice
        } else {
            VoiceChangerSettings {
                enabled: false,
                ..voice
            }
        };
        self.engine.set_voice_settings(&voice);
    }

    fn auto_configure_cable(&mut self) {
        let cable_out = self
            .devices_out
            .iter()
            .find(|d| d.name.to_lowercase().contains("cable input"))
            .map(|d| d.name.clone());
        let cable_in = self
            .devices_in
            .iter()
            .find(|d| d.name.to_lowercase().contains("cable output"));
        match cable_out {
            Some(out) => {
                self.settings.settings.output_device_id = Some(out);
                if let Some(mic) = self
                    .devices_in
                    .iter()
                    .find(|d| !d.name.to_lowercase().contains("cable") && d.is_default)
                    .or_else(|| self.devices_in.iter().find(|d| !d.name.to_lowercase().contains("cable")))
                {
                    self.settings.settings.mic_device_id = Some(mic.name.clone());
                }
                if let Some(mon) = self.devices_out.iter().find(|d| d.is_default && !d.name.to_lowercase().contains("cable")) {
                    self.settings.settings.monitor_device_id = Some(mon.name.clone());
                }
                self.settings.settings.mic_enabled = true;
                self.settings.settings.monitor_enabled = true;
                let _ = cable_in;
                self.mark_dirty();
                self.restart_engine();
                self.toast(self.t("Settings_AutoSetupOk"), widgets::ToastKind::Info);
            }
            None => self.toast(self.t("Settings_AutoSetupMissing"), widgets::ToastKind::Warning),
        }
    }

    fn open_settings(&mut self) {
        self.open_settings_tab(SettingsTab::General);
    }

    fn open_settings_tab(&mut self, tab: SettingsTab) {
        self.devices_out = voxora_audio::output_devices();
        self.devices_in = voxora_audio::input_devices();
        self.draft = Some(self.settings.settings.clone());
        self.tab = tab;
        self.view = View::Settings;
    }

    fn cancel_settings(&mut self) {
        self.draft = None;
        self.mode = if self.settings.settings.theme == "light" {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        self.view = View::Main;
        self.refresh_filtered();
    }

    fn save_settings(&mut self, ctx: &egui::Context) {
        if let Some(draft) = self.draft.take() {
            self.settings.settings = draft;
            self.settings.settings.sanitize();
            self.lang
                .set_language(&self.settings.settings.language);
            self.mode = if self.settings.settings.theme == "light" {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            theme::apply(ctx, self.mode);
            let (c1, c2, c3) = platform::titlebar_colors(self.mode == ThemeMode::Dark);
            platform::apply_titlebar(self.mode == ThemeMode::Dark, c1, c2, c3);
            platform::set_autostart(self.settings.settings.auto_start);
            self.save_now();
            self.refresh_filtered();
            self.rebuild_hotkeys();
            self.restart_engine();
        }
        self.view = View::Main;
    }

    fn finish_onboarding(&mut self) {
        self.settings.settings.onboarding_completed = true;
        self.save_now();
        self.view = View::Main;
        self.restart_engine();
    }

    fn handle_hotkeys(&mut self) {
        let events = self.hotkeys.poll();
        for event in events {
            match event {
                HotkeyEvent::Pressed(HotkeyAction::StopAll) => {
                    self.engine.stop_all();
                    self.playing.clear();
                }
                HotkeyEvent::Pressed(HotkeyAction::Radial) => {
                    if features::is_enabled(&self.settings.settings, FeatureId::RadialMenu) {
                        self.open_radial();
                    }
                }
                HotkeyEvent::Pressed(HotkeyAction::Play(id)) => {
                    if let Some(sound) = self.find_sound(&id) {
                        if sound.hold_to_play {
                            if !self.engine.is_playing(&id) {
                                self.trigger_sound(&sound);
                            }
                            if let Some(hk) = sound.hotkey.clone() {
                                self.holding.push((id.clone(), hk));
                            }
                        } else {
                            self.trigger_sound(&sound);
                        }
                    }
                }
                HotkeyEvent::Released(HotkeyAction::Play(id)) => {
                    if let Some(sound) = self.find_sound(&id) {
                        if sound.hold_to_play {
                            self.engine.stop_id(&id);
                            self.playing.remove(&id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn open_radial(&mut self) {
        radial_toggle();
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let mut commands = Vec::new();
        while let Ok(command) = self.tray.rx.try_recv() {
            commands.push(command);
        }
        for command in commands {
            match command {
                crate::tray::TrayCommand::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::tray::TrayCommand::StopAll => {
                    self.engine.stop_all();
                    self.playing.clear();
                }
                crate::tray::TrayCommand::Radial => {
                    if features::is_enabled(&self.settings.settings, FeatureId::RadialMenu) {
                        self.open_radial();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    }
                }
                crate::tray::TrayCommand::ToggleMic => {
                    self.settings.settings.mic_enabled = !self.settings.settings.mic_enabled;
                    self.apply_engine_settings();
                    self.restart_engine();
                    self.settings.save();
                }
                crate::tray::TrayCommand::Exit => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let mut added = 0usize;
        for file in dropped {
            let path = file.path().to_path_buf();
            if path.is_file() && voxora_core::is_supported(&path) {
                let category = match &self.category {
                    CategorySel::Id(id) => Some(id.clone()),
                    _ => None,
                };
                if self
                    .library
                    .add_sound(&path, category, self.settings.settings.copy_on_import, None)
                    .is_ok()
                {
                    added += 1;
                }
            }
        }
        if added > 0 {
            self.refresh_filtered();
            self.rebuild_hotkeys();
            let text = self.msg(
                &format!("{added} ses eklendi."),
                &format!("{added} sounds added."),
            );
            self.toast(text, widgets::ToastKind::Info);
        }
    }

    fn poll_devices(&mut self) {
        let Some(rx) = &self.device_rx else { return };
        let mut latest = None;
        while let Ok(pair) = rx.try_recv() {
            latest = Some(pair);
        }
        if let Some((outs, ins)) = latest {
            self.devices_out = outs;
            self.devices_in = ins;
        }
    }

    fn poll_update(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.update_progress {
            for (received, total) in rx.try_iter() {
                self.update_received = received;
                self.update_total = total;
            }
        }
        if let Some(rx) = &self.update_rx {
            match rx.try_recv() {
                Ok(Ok(path)) => {
                    self.update_rx = None;
                    self.update_progress = None;
                    self.toast(
                        self.msg("Güncelleme indirildi, kurucu başlatılıyor…", "Update downloaded, launching installer…"),
                        widgets::ToastKind::Info,
                    );
                    let _ = std::process::Command::new(&path)
                        .args(["/CLOSEAPPLICATIONS", "/NORESTART"])
                        .spawn();
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Ok(Err(e)) => {
                    self.update_rx = None;
                    self.update_progress = None;
                    self.toast(
                        self.lang.tf("DeviceName_Failed", &[&e]),
                        widgets::ToastKind::Error,
                    );
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_rx = None;
                }
            }
        }
    }

    fn start_update_download(&mut self) {
        if let Some(update) = &self.update {
            open_url(&update.url);
        }
    }

    fn poll_playing(&mut self) {
        let mut released: Vec<String> = Vec::new();
        self.holding.retain(|(id, gesture)| {
            if platform::gesture_down(gesture) {
                true
            } else {
                released.push(id.clone());
                false
            }
        });
        for id in released {
            self.engine.stop_id(&id);
            self.playing.remove(&id);
        }
        if !self.playing.is_empty() {
            let ids: Vec<String> = self.playing.iter().cloned().collect();
            for id in ids {
                if !self.engine.is_playing(&id) {
                    self.playing.remove(&id);
                }
            }
        }
        self.levels = self.engine.levels();
    }

    // ---------------------------------------------------------------- UI

    fn draw_main(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.bg);

        let header_rect = Rect::from_min_size(full.min, vec2(full.width(), 66.0));
        ui.painter().rect_filled(header_rect, 0.0, p.panel);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(header_rect.left(), header_rect.bottom() - 1.0),
                header_rect.right_bottom(),
            ),
            0.0,
            p.divider,
        );
        let mut hui = child(ui, header_rect.shrink2(vec2(20.0, 0.0)));
        self.draw_header(&mut hui, &p);

        let mut top = header_rect.bottom();
        if self.show_update_banner {
            let brect = Rect::from_min_size(pos2(full.left(), top), vec2(full.width(), 36.0));
            self.draw_update_banner(ui, &p, brect);
            top += 36.0;
        }

        let deck_h = 128.0;
        let body = Rect::from_min_max(
            pos2(full.left(), top),
            pos2(full.right(), full.bottom() - deck_h),
        );
        let deck = Rect::from_min_max(pos2(full.left(), body.bottom()), full.right_bottom());
        let rail = Rect::from_min_max(body.min, pos2(body.left() + 250.0, body.bottom()));
        let content = Rect::from_min_max(pos2(rail.right(), body.top()), body.max);

        ui.painter().rect_filled(rail, 0.0, p.rail);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rail.right() - 1.0, rail.top()),
                rail.right_bottom(),
            ),
            0.0,
            p.divider,
        );
        let mut rui = child(ui, rail);
        self.draw_rail(&mut rui, &p);

        let mut cui = child(ui, content);
        self.draw_content(&mut cui, &p);

        ui.painter().rect_filled(deck, 0.0, p.deck);
        ui.painter().rect_filled(
            Rect::from_min_max(deck.min, pos2(deck.right(), deck.top() + 1.0)),
            0.0,
            p.divider,
        );
        let mut dui = child(ui, deck.shrink2(vec2(20.0, 12.0)));
        self.draw_deck(&mut dui, &p);

        self.draw_toasts(ui, &p);
    }

    fn draw_header(&mut self, ui: &mut egui::Ui, p: &Palette) {
        {
            {
                ui.horizontal_centered(|ui| {
                    // marka
                    let (mark, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                    ui.painter().rect_filled(mark, 9.0, p.accent);
                    for (i, h) in [10.0f32, 18.0, 7.0].iter().enumerate() {
                        let x = mark.center().x + (i as f32 - 1.0) * 8.0;
                        ui.painter().rect_filled(
                            Rect::from_center_size(pos2(x, mark.center().y), vec2(3.0, *h)),
                            1.5,
                            p.on_accent,
                        );
                    }
                    ui.add_space(11.0);
                    ui.vertical(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("VOXORA")
                                .font(theme::font_display(17.0))
                                .color(p.ink),
                        );
                        ui.label(
                            RichText::new(self.t("Main_Tagline"))
                                .font(theme::font_mono(9.5))
                                .color(p.ink3),
                        );
                    });

                    // arama — marka ile eylemler arasında ortalanır (MaxWidth 460)
                    let actions_w = 560.0_f32.min(ui.available_width() * 0.52);
                    let mid_w = (ui.available_width() - actions_w).max(180.0);
                    let search_w = (mid_w - 10.0).min(460.0);
                    ui.allocate_ui_with_layout(
                        vec2(mid_w, 66.0),
                        Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            if features::is_enabled(
                                &self.settings.settings,
                                FeatureId::SmartSoundSearch,
                            ) {
                                Frame::NONE
                                    .fill(p.input)
                                    .stroke(Stroke::new(1.0, p.input_border))
                                    .corner_radius(9.0)
                                    .inner_margin(Margin {
                                        left: 13,
                                        right: 9,
                                        top: 0,
                                        bottom: 0,
                                    })
                                    .show(ui, |ui| {
                                        ui.set_width(search_w);
                                        ui.set_height(38.0);
                                        ui.horizontal_centered(|ui| {
                                            widgets::icon_glyph(ui, I_SEARCH, 13.0, p.ink3);
                                            ui.add_space(6.0);
                                            let hint = self.t("Main_SearchPlaceholder");
                                            let mut edit = TextEdit::singleline(&mut self.search)
                                                .frame(Frame::NONE)
                                                .desired_width((search_w - 92.0).max(60.0))
                                                .hint_text(hint)
                                                .text_color(p.ink)
                                                .font(theme::font_body(12.5));
                                            if self.focus_search {
                                                edit = edit.id(ui.make_persistent_id("voxora-search"));
                                            }
                                            let resp = ui.add(edit);
                                            if self.focus_search {
                                                resp.request_focus();
                                                self.focus_search = false;
                                            }
                                            if resp.changed() {
                                                self.refresh_filtered();
                                            }
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    widgets::chip(ui, "CTRL K", p.ink3, p);
                                                },
                                            );
                                        });
                                    });
                            }
                        },
                    );

                    // eylemler (sağdan sola)
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, I_SETTINGS, &self.t("Main_Settings"), p)
                            .clicked()
                        {
                            self.open_settings();
                        }
                        if features::is_enabled(
                            &self.settings.settings,
                            FeatureId::VoiceChanger,
                        ) && widgets::icon_text_button(
                            ui,
                            I_MIC,
                            &self.t("Feature_VoiceChanger_Name"),
                            widgets::ButtonKind::Soft,
                            p,
                        )
                        .clicked()
                        {
                            self.open_settings_tab(SettingsTab::Audio);
                        }
                        ui.add_space(12.0);
                        widgets::divider_v(ui, p);
                        ui.add_space(12.0);
                        if widgets::icon_button(
                            ui,
                            I_EXPORT_SOUNDS,
                            &self.t("Main_ExportSounds"),
                            p,
                        )
                        .clicked()
                        {
                            self.export_sounds_dialog();
                        }
                        if widgets::icon_button(ui, I_EXPORT, &self.t("Main_Export"), p).clicked() {
                            self.export_library_dialog();
                        }
                        if widgets::icon_button(ui, I_IMPORT, &self.t("Main_Import"), p).clicked() {
                            self.import_library_dialog();
                        }
                        if widgets::icon_button(ui, I_FOLDER, &self.t("Main_AddFolder"), p).clicked()
                        {
                            self.add_folder_dialog();
                        }
                        ui.add_space(12.0);
                        widgets::divider_v(ui, p);
                        ui.add_space(12.0);
                        if widgets::icon_text_button(
                            ui,
                            I_ADD,
                            &self.t("Main_AddSound"),
                            widgets::ButtonKind::Primary,
                            p,
                        )
                        .clicked()
                        {
                            self.add_files_dialog();
                        }
                    });
                });
            }
        }
    }

    fn draw_update_banner(&mut self, ui: &mut egui::Ui, p: &Palette, rect: Rect) {
        if !self.show_update_banner {
            return;
        }
        ui.painter().rect_filled(rect, 0.0, p.accent_dim);
        ui.painter().rect_filled(
            Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 1.0)),
            0.0,
            p.accent,
        );
        let mut bui = child(ui, rect.shrink2(vec2(20.0, 6.0)));
        bui.horizontal_centered(|ui| {
            widgets::icon_glyph(ui, I_INFO, 14.0, p.accent);
            ui.add_space(10.0);
            if let Some(update) = &self.update {
                ui.label(
                    RichText::new(format!(
                        "Voxora {} — {}",
                        update.latest,
                        self.t("Settings_UpdateAvailable")
                    ))
                    .font(theme::font_body(12.5))
                    .color(p.ink),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, I_CLOSE, &self.t("Common_Close"), p).clicked() {
                    self.show_update_banner = false;
                }
                if widgets::icon_text_button(
                    ui,
                    I_REFRESH,
                    &self.t("Settings_DownloadUpdate"),
                    widgets::ButtonKind::Plain,
                    p,
                )
                .clicked()
                {
                    if let Some(update) = &self.update {
                        open_url(&update.url);
                    }
                }
            });
        });
    }

    fn draw_rail(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        widgets::display_at(
            ui.painter(),
            pos2(rect.left() + 20.0, rect.top() + 18.0),
            &self.t("Main_Library"),
            10.5,
            p.ink2,
            Align2::LEFT_TOP,
        );

        // + Kategori Ekle
        let add = Rect::from_min_size(
            pos2(rect.left() + 14.0, rect.top() + 44.0),
            vec2(rect.width() - 28.0, 34.0),
        );
        let add_resp = ui.interact(add, ui.scope_id().with("add-category"), Sense::click());
        let add_bg = if add_resp.hovered() { p.surface_hi } else { p.surface };
        ui.painter().rect(
            add,
            8.0,
            add_bg,
            Stroke::new(1.0, p.input_border),
            StrokeKind::Inside,
        );
        widgets::paint_icon_text(
            ui,
            add.center(),
            I_ADD,
            &self.t("Main_AddCategory"),
            11.0,
            12.0,
            7.0,
            p.accent,
            p.ink,
        );
        if add_resp.clicked() {
            self.add_category_dialog();
        }

        // liste
        let cable_h = 54.0;
        let list_rect = Rect::from_min_max(
            pos2(rect.left(), rect.top() + 86.0),
            pos2(rect.right(), rect.bottom() - cable_h - 22.0),
        );
        let mut lui = child(ui, list_rect);
        let counts = self.library.category_counts();
        let all_count = counts.get(&None).copied().unwrap_or(0);
        let fav = self.library.data.sounds.iter().filter(|s| s.favorite).count();
        egui::ScrollArea::vertical()
            .id_salt("rail-scroll")
            .auto_shrink([false, false])
            .show(&mut lui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                self.category_row(ui, p, "\u{1F3A7}", &self.t("Category_All"), all_count, CategorySel::All);
                self.category_row(ui, p, "\u{2605}", &self.t("Category_Favorites"), fav, CategorySel::Favorites);
                let cats: Vec<_> = self.library.data.categories.clone();
                for cat in cats {
                    let count = counts.get(&Some(cat.id.clone())).copied().unwrap_or(0);
                    let icon = cat.icon.clone().unwrap_or_else(|| "\u{1F4C1}".into());
                    self.category_row(ui, p, &icon, &cat.name, count, CategorySel::Id(cat.id.clone()));
                }
            });

        // kablo durumu
        let cable = self
            .devices_out
            .iter()
            .any(|d| d.name.to_lowercase().contains("cable"))
            && self
                .devices_in
                .iter()
                .any(|d| d.name.to_lowercase().contains("cable"));
        let routed = self
            .settings
            .settings
            .output_device_id
            .as_deref()
            .map(|id| id.to_lowercase().contains("cable"))
            .unwrap_or(false);
        let dot = if !cable {
            p.danger
        } else if routed {
            p.success
        } else {
            p.warning
        };
        let text = if cable {
            if routed {
                self.t("Cable_Active")
            } else {
                self.t("Cable_InstalledNotRouted")
            }
        } else {
            self.t("Cable_Missing")
        };
        let card = Rect::from_min_size(
            pos2(rect.left() + 14.0, rect.bottom() - cable_h - 16.0),
            vec2(rect.width() - 28.0, cable_h),
        );
        let card_resp = ui.interact(card, ui.scope_id().with("cable-status"), Sense::click());
        ui.painter().rect(
            card,
            8.0,
            if card_resp.hovered() { p.surface_hi } else { p.surface },
            Stroke::new(1.0, p.divider),
            StrokeKind::Inside,
        );
        ui.painter()
            .circle_filled(pos2(card.left() + 16.0, card.top() + 16.0), 3.5, dot);
        let galley = ui.painter().layout(
            text,
            theme::font_body(11.0),
            p.ink2,
            card.width() - 40.0,
        );
        ui.painter().galley(
            pos2(card.left() + 26.0, card.top() + 10.0),
            galley,
            p.ink2,
        );
        if card_resp.clicked() {
            self.open_cable_guide();
        }
    }

    fn category_row(&mut self, ui: &mut egui::Ui, p: &Palette, icon: &str, name: &str, count: usize, sel: CategorySel) {
        let selected = self.category == sel;
        let width = ui.available_width().min(230.0);
        let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
        if response.hovered() || selected {
            ui.painter().rect_filled(rect, 8.0, p.surface_hi);
        }
        if selected {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - 9.0), vec2(3.0, 18.0)),
                1.5,
                p.accent,
            );
        }
        ui.painter().text(
            pos2(rect.left() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            theme::font_body(13.0),
            p.ink2,
        );
        ui.painter().text(
            pos2(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            theme::font_body(12.5),
            if selected { p.ink } else { p.ink2 },
        );
        let count_text = count.to_string();
        let galley = ui
            .painter()
            .layout_no_wrap(count_text, theme::font_mono(9.5), p.ink3);
        let chip_rect = Rect::from_center_size(
            pos2(rect.right() - 18.0, rect.center().y),
            vec2(galley.size().x + 12.0, 18.0),
        );
        ui.painter().rect_filled(chip_rect, 9.0, p.input);
        ui.painter().galley(
            chip_rect.center() - galley.size() / 2.0,
            galley,
            p.ink3,
        );
        if response.clicked() {
            self.category = sel.clone();
            self.refresh_filtered();
        }
        response.context_menu(|ui| {
            if let CategorySel::Id(id) = &sel {
                let id = id.clone();
                if ui.button(self.lang.t("Category_Rename")).clicked() {
                    self.rename_category_dialog(&id);
                    ui.close();
                }
                if ui.button(self.lang.t("Category_Delete")).clicked() {
                    self.delete_category_dialog(&id);
                    ui.close();
                }
            }
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        let title = match &self.category {
            CategorySel::All => self.t("Category_All"),
            CategorySel::Favorites => self.t("Category_Favorites"),
            CategorySel::Id(id) => self
                .library
                .data
                .categories
                .iter()
                .find(|c| &c.id == id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| self.t("Category_All")),
        };
        widgets::display_at(
            ui.painter(),
            pos2(rect.left() + 24.0, rect.top() + 20.0),
            &title,
            21.0,
            p.ink,
            Align2::LEFT_TOP,
        );
        widgets::display_at(
            ui.painter(),
            pos2(rect.left() + 25.0, rect.top() + 50.0),
            &format!("{}  {}", self.filtered.len(), self.t("Main_SongsUnit")),
            9.5,
            p.ink3,
            Align2::LEFT_TOP,
        );

        // Yönlendirme rehberi (sağ üst)
        let guide = Rect::from_min_size(
            pos2(rect.right() - 24.0 - 152.0, rect.top() + 26.0),
            vec2(152.0, 32.0),
        );
        let guide_resp = ui.interact(guide, ui.scope_id().with("routing-guide"), Sense::click());
        ui.painter().rect_filled(
            guide,
            8.0,
            if guide_resp.hovered() {
                p.surface_hi
            } else {
                p.accent_dim
            },
        );
        widgets::paint_icon_text(
            ui,
            guide.center(),
            I_INFO,
            &self.t("Main_RoutingGuide"),
            12.0,
            12.0,
            7.0,
            p.accent,
            p.accent,
        );
        if guide_resp.clicked() {
            self.open_cable_guide();
        }

        let scroll_rect = Rect::from_min_max(
            pos2(rect.left(), rect.top() + 66.0),
            pos2(rect.right() - 6.0, rect.bottom()),
        );
        let mut sui = child(ui, scroll_rect);
        if self.filtered.is_empty() {
            self.draw_empty(&mut sui, p);
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("sound-grid")
            .auto_shrink([false, false])
            .show(&mut sui, |ui| {
                ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                let width = ui.available_width() - 18.0;
                let per_row = ((width + 14.0) / 250.0).floor().max(1.0) as usize;
                let total = self.filtered.len();
                let row_h = 172.0 + 14.0;
                let clip = ui.clip_rect();
                let mut start = 0usize;
                while start < total {
                    let top = ui.cursor().top();
                    if top > clip.bottom() {
                        break;
                    }
                    if top + row_h < clip.top() {
                        ui.add_space(row_h);
                        start += per_row;
                        continue;
                    }
                    let end = (start + per_row).min(total);
                    let row: Vec<SoundItem> = self.filtered[start..end].to_vec();
                    ui.horizontal(|ui| {
                        for sound in &row {
                            self.draw_card(ui, p, sound);
                        }
                    });
                    start = end;
                }
            });
    }

    fn draw_empty(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        let center_x = rect.center().x;
        let top = rect.top() + 150.0;
        for i in 0..6 {
            let h = [34.0f32, 22.0, 10.0, 26.0, 16.0, 30.0][i];
            let x = center_x + (i as f32 - 2.5) * 16.0;
            let bar = Rect::from_min_max(
                pos2(x - 2.0, top + 34.0 - h),
                pos2(x + 2.0, top + 34.0),
            );
            ui.painter().rect_filled(bar, 2.0, p.accent);
        }
        let title = self.t("Main_EmptyTitle");
        ui.painter().text(
            pos2(center_x, top + 70.0),
            Align2::CENTER_TOP,
            title,
            theme::font_display(22.0),
            p.ink,
        );
        let mut body_ui = child(
            ui,
            Rect::from_min_size(pos2(center_x - 230.0, top + 106.0), vec2(460.0, 70.0)),
        );
        body_ui.vertical_centered(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(self.t("Main_EmptyBody"))
                        .font(theme::font_body(13.0))
                        .color(p.ink2),
                )
                .wrap(),
            );
        });
        let btn_y = top + 186.0;
        let a_w = 168.0;
        let b_w = 150.0;
        let gap = 10.0;
        let total = a_w + gap + b_w;
        let a_rect = Rect::from_min_size(
            pos2(center_x - total / 2.0, btn_y),
            vec2(a_w, 36.0),
        );
        let b_rect = Rect::from_min_size(
            pos2(a_rect.right() + gap, btn_y),
            vec2(b_w, 36.0),
        );
        let a_resp = ui.interact(a_rect, ui.scope_id().with("empty-add"), Sense::click());
        ui.painter().rect_filled(
            a_rect,
            8.0,
            if a_resp.hovered() { p.accent_hover } else { p.accent },
        );
        widgets::paint_icon_text(
            ui,
            a_rect.center(),
            I_ADD,
            &self.t("Main_AddSoundFile"),
            12.0,
            12.5,
            8.0,
            p.on_accent,
            p.on_accent,
        );
        let b_resp = ui.interact(b_rect, ui.scope_id().with("empty-guide"), Sense::click());
        ui.painter().rect(
            b_rect,
            8.0,
            if b_resp.hovered() { p.surface_hi } else { p.surface },
            Stroke::new(1.0, p.input_border),
            StrokeKind::Inside,
        );
        widgets::paint_icon_text(
            ui,
            b_rect.center(),
            I_INFO,
            &self.t("Main_MicGuide"),
            12.0,
            12.5,
            8.0,
            p.ink2,
            p.ink,
        );
        if a_resp.clicked() {
            self.add_files_dialog();
        }
        if b_resp.clicked() {
            self.open_cable_guide();
        }
    }

    fn draw_card(&mut self, ui: &mut egui::Ui, p: &Palette, sound: &SoundItem) {
        let size = vec2(236.0, 172.0);
        ui.add_space(0.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let id = ui.make_persistent_id(("card", sound.id.as_str()));
        let hover = response.hovered();
        let playing = self.playing.contains(&sound.id) || self.engine.is_playing(&sound.id);
        let fill = if playing {
            p.glow
        } else if hover {
            p.surface_hi
        } else {
            p.surface
        };
        let border = if playing { p.accent } else if hover { p.ink3 } else { p.divider };
        ui.painter()
            .rect(rect, 12.0, fill, Stroke::new(if playing { 1.5 } else { 1.0 }, border), StrokeKind::Inside);
        if playing {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, vec2(4.0, rect.height())),
                12.0,
                p.accent,
            );
        }

        let inner = rect.shrink2(vec2(16.0, 12.0));
        // ikon + favori
        let dot = sound
            .color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(p.accent);
        ui.painter()
            .circle_filled(pos2(inner.left() + 4.0, inner.top() + 8.0), 3.5, dot);
        if let Some(icon) = sound.icon.as_deref().filter(|i| !i.is_empty()) {
            ui.painter().text(
                pos2(inner.left() + 14.0, inner.top() + 8.0),
                Align2::LEFT_CENTER,
                icon,
                theme::font_body(12.0),
                p.ink2,
            );
        } else {
            widgets::icon_glyph_at(ui, pos2(inner.left() + 14.0, inner.top() + 3.0), I_MUSIC, 11.0, p.ink3);
        }
        if sound.favorite {
            widgets::icon_glyph_at(ui, pos2(inner.right() - 8.0, inner.top() + 3.0), I_STAR, 13.0, p.warning);
        }
        let remove_rect = Rect::from_center_size(
            pos2(inner.right() - 35.0, inner.top() + 7.0),
            vec2(26.0, 26.0),
        );
        let remove_response = ui.interact(remove_rect, id.with("remove"), Sense::click())
            .on_hover_text(self.t("Main_CardRemove"));
        if remove_response.hovered() {
            ui.painter().rect_filled(remove_rect, 6.0, p.surface_hi);
        }
        widgets::icon_glyph_at(ui, remove_rect.center(), I_DELETE, 13.0, p.ink2);

        // isim
        ui.painter().text(
            pos2(inner.left(), inner.top() + 38.0),
            Align2::LEFT_TOP,
            truncate(&sound.name, 22),
            theme::font_body(14.5),
            p.ink,
        );

        // süre + kısayol
        let duration = sound.duration_text();
        ui.painter().text(
            pos2(inner.left(), inner.top() + 72.0),
            Align2::LEFT_CENTER,
            &duration,
            theme::font_mono(10.0),
            p.ink3,
        );
        if let Some(hk) = &sound.hotkey {
            if hk.is_valid() {
                let text = hk.display();
                let galley = ui
                    .painter()
                    .layout_no_wrap(text, theme::font_mono(9.5), p.accent);
                let chip = Rect::from_min_size(
                    pos2(inner.left() + 46.0, inner.top() + 63.0),
                    galley.size() + vec2(10.0, 3.0),
                );
                ui.painter().rect_filled(chip, 4.0, p.accent_dim);
                ui.painter().galley(
                    chip.min + vec2(5.0, 1.5),
                    galley,
                    p.accent,
                );
            }
        }

        // LED
        if playing {
            let t = ui.ctx().input(|i| i.time) as f32;
            for i in 0..4 {
                let h = 6.0 + (t * (7.0 + i as f32 * 1.3) + i as f32).sin().abs() * 11.0;
                let x = inner.right() - 26.0 + i as f32 * 7.0;
                let bar = Rect::from_min_max(
                    pos2(x, inner.top() + 76.0 - h),
                    pos2(x + 3.0, inner.top() + 76.0),
                );
                ui.painter().rect_filled(bar, 1.5, p.accent);
            }
        }

        // fader
        let fader_rect = Rect::from_min_size(pos2(inner.left(), inner.bottom() - 22.0), vec2(inner.width() - 34.0, 22.0));
        let fader_response = ui.interact(fader_rect, id.with("fader"), Sense::click_and_drag());
        if fader_response.dragged() || fader_response.clicked() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                let rel = ((pos.x - fader_rect.left()) / fader_rect.width()).clamp(0.0, 1.0);
                let volume = rel * 1.5;
                self.engine.set_volume(&sound.id, if sound.is_muted { 0.0 } else { volume });
                self.update_sound(&sound.id, |s| s.volume = volume);
            }
        }
        let track = Rect::from_center_size(fader_rect.center(), vec2(fader_rect.width(), 6.0));
        ui.painter().rect_filled(track, 3.0, p.input);
        let fraction = (sound.volume / 1.5).clamp(0.0, 1.0);
        ui.painter().rect_filled(
            Rect::from_min_size(track.min, vec2(track.width() * fraction, 6.0)),
            3.0,
            p.accent,
        );
        let cap = Rect::from_center_size(
            pos2(track.left() + track.width() * fraction, track.center().y),
            vec2(12.0, 20.0),
        );
        ui.painter().rect(cap, 3.0, p.ink, Stroke::new(1.0, p.bg), StrokeKind::Inside);
        ui.painter().rect_filled(
            Rect::from_center_size(cap.center(), vec2(2.0, 9.0)),
            1.0,
            p.bg.gamma_multiply(0.6),
        );

        // mute
        let mute_rect = Rect::from_min_size(
            pos2(fader_rect.right() + 8.0, inner.bottom() - 22.0),
            vec2(24.0, 22.0),
        );
        let mute_response = ui.interact(mute_rect, id.with("mute"), Sense::click());
        if mute_response.hovered() {
            ui.painter().rect_filled(mute_rect, 6.0, p.surface_hi);
        }
        widgets::icon_glyph_at(
            ui,
            pos2(mute_rect.center().x, mute_rect.center().y),
            if sound.is_muted { I_MUTE } else { I_VOLUME },
            13.0,
            if sound.is_muted { p.danger } else { p.ink2 },
        );

        let mut action = None;
        if remove_response.clicked() {
            action = Some(CardAction::Remove);
        } else if response.clicked() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                if fader_rect.contains(pos) {
                    // fader yukarıda işlendi
                } else if mute_rect.contains(pos) {
                    action = Some(CardAction::Mute);
                } else {
                    action = Some(CardAction::Play);
                }
            } else {
                action = Some(CardAction::Play);
            }
        }
        response.context_menu(|ui| {
            if ui.button(self.t("Main_CardPlayStop")).clicked() {
                action = Some(CardAction::Play);
                ui.close();
            }
            if ui.button(self.t("Main_CardMuteToggle")).clicked() {
                action = Some(CardAction::Mute);
                ui.close();
            }
            if !sound.file_exists() {
                if ui.button(self.msg("Dosyayı Bul…", "Locate file…")).clicked() {
                    action = Some(CardAction::Locate);
                    ui.close();
                }
            }
            if ui.button(self.t("Main_CardEdit")).clicked() {
                action = Some(CardAction::Edit);
                ui.close();
            }
            if ui.button(self.t("Main_CardFavorite")).clicked() {
                action = Some(CardAction::ToggleFavorite);
                ui.close();
            }
            if ui.button(self.t("Main_CardRemove")).clicked() {
                action = Some(CardAction::Remove);
                ui.close();
            }
        });

        match action {
            Some(CardAction::Play) => self.trigger_sound(sound),
            Some(CardAction::Mute) => {
                let muted = !sound.is_muted;
                self.update_sound(&sound.id, |s| s.is_muted = muted);
                self.engine
                    .set_volume(&sound.id, if muted { 0.0 } else { sound.volume });
            }
            Some(CardAction::ToggleFavorite) => {
                self.update_sound(&sound.id, |s| s.favorite = !s.favorite);
                self.refresh_filtered();
            }
            Some(CardAction::Edit) => {
                self.edit_sound_dialog(&sound.id);
            }
            Some(CardAction::Locate) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Ses",
                        &["mp3", "wav", "ogg", "flac", "m4a", "aac", "wma", "opus"],
                    )
                    .pick_file()
                {
                    let id = sound.id.clone();
                    let duration = voxora_audio::decode_file(&path)
                        .map(|d| d.duration_seconds)
                        .unwrap_or(0.0);
                    let path_text = path.to_string_lossy().to_string();
                    self.update_sound(&id, |s| {
                        s.file_path = path_text.clone();
                        s.duration_seconds = duration;
                    });
                    self.decoded.remove(&id);
                    self.library.save();
                    self.refresh_filtered();
                }
            }
            Some(CardAction::Remove) => {
                self.dialog = Some(Dialog::SoundRemove {
                    id: sound.id.clone(),
                    name: sound.name.clone(),
                });
            }
            None => {}
        }
        ui.add_space(0.0);
    }

    fn draw_deck(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let rect = ui.max_rect();
        let master_w = 132.0;
        let flex = (rect.width() - master_w).max(300.0);
        let c1 = flex * (1.25 / 3.55);
        let c2 = flex * (1.05 / 3.55);
        let c3 = flex * (1.25 / 3.55);

        // ---------------- MİKROFON
        let mic_rect = Rect::from_min_size(rect.min, vec2(c1 - 26.0, rect.height()));
        let mut ui_mic = child(ui, mic_rect);
        ui_mic.horizontal(|ui| {
            widgets::section(ui, &self.t("Main_Microphone"), p);
            ui.add_space(8.0);
            let mut on = self.settings.settings.mic_enabled;
            if widgets::switch(ui, &mut on, &self.t("Main_MicActive"), p) {
                self.settings.settings.mic_enabled = on;
                self.mark_dirty();
                self.apply_engine_settings();
                self.restart_engine();
            }
        });
        ui_mic.add_space(4.0);
        let mut selected = self.settings.settings.mic_device_id.clone();
        let items: Vec<String> = self.devices_in.iter().map(|d| d.name.clone()).collect();
        if widgets::combo(&mut ui_mic, "mic-device", &mut selected, &items, mic_rect.width() - 4.0, p) {
            self.settings.settings.mic_device_id = selected;
            self.mark_dirty();
            self.restart_engine();
        }
        ui_mic.add_space(4.0);
        ui_mic.horizontal(|ui| {
            let mut volume = self.settings.settings.mic_volume;
            let width = (ui.available_width() - 52.0).max(100.0);
            if widgets::fader(ui, &mut volume, 1.5, width, p) {
                self.settings.settings.mic_volume = volume;
                self.apply_engine_settings();
                self.mark_dirty();
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{}%", (volume * 100.0).round() as i32))
                    .font(theme::font_mono(12.0))
                    .color(p.ink2),
            );
        });

        // ---------------- EFEKTLER
        let fx_rect = Rect::from_min_size(pos2(rect.left() + c1, rect.top()), vec2(c2 - 26.0, rect.height()));
        let mut ui_fx = child(ui, fx_rect);
        widgets::section(&mut ui_fx, &self.t("Main_EffectSounds"), p);
        ui_fx.add_space(4.0);
        ui_fx.horizontal(|ui| {
            let mut volume = self.settings.settings.fx_volume;
            let width = (ui.available_width() - 52.0).max(100.0);
            if widgets::fader(ui, &mut volume, 1.5, width, p) {
                self.settings.settings.fx_volume = volume;
                self.apply_engine_settings();
                self.mark_dirty();
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{}%", (volume * 100.0).round() as i32))
                    .font(theme::font_mono(12.0))
                    .color(p.ink2),
            );
        });
        ui_fx.add_space(10.0);
        ui_fx.horizontal(|ui| {
            ui.label(
                RichText::new("FX")
                    .font(theme::font_display(9.0))
                    .color(p.ink2),
            );
            ui.add_space(8.0);
            let width = ui.available_width();
            widgets::meter(ui, self.levels.0, width, p.accent, p);
        });
        ui_fx.add_space(5.0);
        ui_fx.horizontal(|ui| {
            ui.label(
                RichText::new("OUT")
                    .font(theme::font_display(9.0))
                    .color(p.ink2),
            );
            ui.add_space(8.0);
            let width = ui.available_width();
            widgets::meter(ui, self.levels.1, width, p.success, p);
        });

        // ---------------- ÇIKIŞ
        let out_rect = Rect::from_min_size(
            pos2(rect.left() + c1 + c2, rect.top()),
            vec2(c3 - 26.0, rect.height()),
        );
        let mut ui_out = child(ui, out_rect);
        ui_out.horizontal(|ui| {
            widgets::section(ui, &self.t("Main_OutputGameDiscord"), p);
            ui.add_space(8.0);
            let mut on = self.settings.settings.monitor_enabled;
            if widgets::switch(ui, &mut on, &self.t("Main_LocalMonitor"), p) {
                self.settings.settings.monitor_enabled = on;
                self.mark_dirty();
                self.restart_engine();
            }
        });
        ui_out.add_space(4.0);
        let mut selected = self.settings.settings.output_device_id.clone();
        let items: Vec<String> = self.devices_out.iter().map(|d| d.name.clone()).collect();
        if widgets::combo(&mut ui_out, "out-device", &mut selected, &items, out_rect.width() - 4.0, p) {
            self.settings.settings.output_device_id = selected;
            self.mark_dirty();
            self.restart_engine();
        }
        ui_out.add_space(7.0);
        let status = self.status.clone();
        ui_out.add(
            egui::Label::new(
                RichText::new(status)
                    .font(theme::font_body(11.0))
                    .color(p.ink3),
            )
            .wrap(),
        );

        // ---------------- MASTER
        let master_rect = Rect::from_min_size(
            pos2(rect.right() - master_w, rect.top()),
            vec2(master_w, rect.height()),
        );
        let mut ui_master = child(ui, master_rect);
        ui_master.vertical_centered(|ui| {
            ui.label(
                RichText::new(self.t("Main_Master"))
                    .font(theme::font_display(10.5))
                    .color(p.ink2),
            );
            ui.add_space(5.0);
            let peak = Rect::from_min_size(ui.cursor().min, vec2(master_w, 30.0));
            let resp = ui.allocate_rect(peak, Sense::hover());
            let _ = resp;
            ui.painter().rect(
                peak,
                8.0,
                p.surface,
                Stroke::new(1.0, p.divider),
                StrokeKind::Inside,
            );
            let text = format!("{}%", (self.levels.1 * 100.0).round() as i32);
            let galley = ui.painter().layout_no_wrap(
                text,
                theme::font_mono(15.0),
                p.success,
            );
            ui.painter().galley(
                pos2(peak.center().x - galley.size().x / 2.0 - 12.0, peak.center().y - galley.size().y / 2.0),
                galley,
                p.success,
            );
            ui.painter().text(
                pos2(peak.center().x + 30.0, peak.center().y),
                Align2::CENTER_CENTER,
                "PEAK",
                theme::font_display(8.5),
                p.ink2,
            );
            ui.add_space(8.0);
            let (btn, response) = ui.allocate_exact_size(vec2(master_w, 36.0), Sense::click());
            let bg = if response.hovered() {
                p.danger.gamma_multiply(1.1)
            } else {
                p.danger
            };
            ui.painter().rect_filled(btn, 8.0, bg);
            widgets::paint_icon_text(
                ui,
                btn.center(),
                I_STOP,
                &self.t("Main_Stop"),
                12.0,
                12.5,
                8.0,
                p.on_accent,
                p.on_accent,
            );
            if response.clicked() {
                self.engine.stop_all();
                self.playing.clear();
            }
        });
    }

    fn draw_toasts(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let now = Instant::now();
        self.toasts.retain(|t| now.duration_since(t.at).as_secs_f32() < 3.2);
        if self.toasts.is_empty() {
            return;
        }
        let screen = ui.max_rect();
        let mut y = screen.bottom() - 150.0;
        let toasts: Vec<(String, widgets::ToastKind, f32)> = self
            .toasts
            .iter()
            .map(|t| (t.text.clone(), t.kind, now.duration_since(t.at).as_secs_f32()))
            .collect();
        for (text, kind, age) in toasts {
            let rect = Rect::from_min_size(pos2(screen.left() + 20.0, y), vec2(360.0, 34.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            widgets::toast(&mut child, &text, kind, p, 360.0, age);
            y -= 40.0;
        }
    }

    fn rename_endpoints(&mut self) {
        if self.rename_rx.is_some() {
            return;
        }
        self.toast(
            self.lang.t("DeviceName_Working"),
            widgets::ToastKind::Info,
        );
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let outcome = crate::rename::rename_all();
            let _ = tx.send(outcome);
        });
        self.rename_rx = Some(rx);
    }

    fn open_cable_guide(&mut self) {
        self.view = View::Guide;
    }

    fn draw_guide(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.bg);
        let card = Rect::from_center_size(
            full.center(),
            vec2(760.0f32.min(full.width() - 60.0), 640.0f32.min(full.height() - 40.0)),
        );
        ui.painter().rect(
            card,
            16.0,
            p.panel,
            Stroke::new(1.0, p.divider),
            StrokeKind::Inside,
        );
        let mut cui = child(ui, card.shrink2(vec2(28.0, 24.0)));
        cui.horizontal(|ui| {
            widgets::icon_glyph(ui, I_MIC, 18.0, p.accent);
            ui.add_space(9.0);
            ui.label(
                RichText::new(self.lang.t("Guide_Title"))
                    .font(theme::font_display(21.0))
                    .color(p.ink),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, I_CLOSE, &self.lang.t("Common_Close"), &p).clicked() {
                    self.view = View::Main;
                }
            });
        });
        cui.add_space(2.0);
        cui.label(
            RichText::new(self.lang.t("Guide_Subtitle"))
                .font(theme::font_body(12.0))
                .color(p.ink2),
        );
        cui.add_space(14.0);
        for (step, body, action) in [
            (
                "Guide_Step1",
                "Guide_Step1Body",
                Some(("Guide_OpenDownload", "https://vb-audio.com/Cable/")),
            ),
            ("Guide_Step2", "Guide_Step2Body", None),
            ("Guide_Step3", "Guide_Step3Body", None),
        ] {
            Frame::NONE
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.divider))
                .corner_radius(10.0)
                .inner_margin(Margin::same(14))
                .show(&mut cui, |ui| {
                    ui.set_width(card.width() - 84.0);
                    ui.label(
                        RichText::new(self.lang.t(step))
                            .font(theme::font_display(10.5))
                            .color(p.accent),
                    );
                    ui.add_space(4.0);
                    let mut body_ui = ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(self.lang.t(body))
                                    .font(theme::font_body(12.0))
                                    .color(p.ink2),
                            )
                            .wrap(),
                        );
                    });
                    let _ = &mut body_ui;
                    if let Some((label, url)) = action {
                        ui.add_space(6.0);
                        if widgets::icon_text_button(
                            ui,
                            I_FOLDER,
                            &self.lang.t(label),
                            widgets::ButtonKind::Soft,
                            &p,
                        )
                        .clicked()
                        {
                            open_url(url);
                        }
                    }
                });
            cui.add_space(10.0);
        }
        cui.horizontal(|ui| {
            if widgets::icon_text_button(
                ui,
                I_SETTINGS,
                &self.lang.t("Guide_OpenSoundSettings"),
                widgets::ButtonKind::Plain,
                &p,
            )
            .clicked()
            {
                open_url("ms-settings:sound");
            }
            ui.add_space(8.0);
            if widgets::icon_text_button(
                ui,
                I_REFRESH,
                &self.lang.t("Settings_AutoSetup"),
                widgets::ButtonKind::Soft,
                &p,
            )
            .clicked()
            {
                self.auto_configure_cable();
            }
        });
        cui.add_space(8.0);
        cui.add(
            egui::Label::new(
                RichText::new(self.lang.t("Guide_AutoNote"))
                    .font(theme::font_body(11.0))
                    .color(p.ink3),
            )
            .wrap(),
        );
    }

    fn add_files_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Ses", &["mp3", "wav", "ogg", "flac", "m4a", "aac", "wma", "opus"])
            .pick_files()
        {
            for path in paths {
                if !path.is_file() {
                    continue;
                }
                match self.library.add_sound(
                    &path,
                    match &self.category {
                        CategorySel::Id(id) => Some(id.clone()),
                        _ => None,
                    },
                    self.settings.settings.copy_on_import,
                    None,
                ) {
                    Ok(_) => {}
                    Err(e) => self.toast(e.to_string(), widgets::ToastKind::Error),
                }
            }
            self.refresh_filtered();
            self.rebuild_hotkeys();
        }
    }

    fn add_folder_dialog(&mut self) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            if let Ok(entries) = std::fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && voxora_core::is_supported(&path) {
                        let _ = self.library.add_sound(
                            &path,
                            match &self.category {
                                CategorySel::Id(id) => Some(id.clone()),
                                _ => None,
                            },
                            self.settings.settings.copy_on_import,
                            None,
                        );
                    }
                }
            }
            self.refresh_filtered();
            self.rebuild_hotkeys();
        }
    }

    fn add_category_dialog(&mut self) {
        self.dialog = Some(Dialog::CategoryNew {
            name: String::new(),
        });
        self.dialog_capture = false;
    }

    fn rename_category_dialog(&mut self, id: &str) {
        if let Some(cat) = self.library.data.categories.iter().find(|c| c.id == id) {
            self.dialog = Some(Dialog::CategoryRename {
                id: id.to_string(),
                name: cat.name.clone(),
            });
            self.dialog_capture = false;
        }
    }

    fn delete_category_dialog(&mut self, id: &str) {
        if let Some(cat) = self.library.data.categories.iter().find(|c| c.id == id) {
            self.dialog = Some(Dialog::CategoryDelete {
                id: id.to_string(),
                name: cat.name.clone(),
            });
        }
    }

    fn edit_sound_dialog(&mut self, id: &str) {
        if let Some(sound) = self.library.data.sounds.iter().find(|s| s.id == id) {
            self.dialog = Some(Dialog::SoundEdit {
                id: id.to_string(),
                name: sound.name.clone(),
                hotkey: sound.hotkey.clone(),
                behavior: sound.behavior as i32,
                hold: sound.hold_to_play,
                favorite: sound.favorite,
                category: sound.category_id.clone(),
                icon: sound.icon.clone().unwrap_or_default(),
                color: sound.color.clone().unwrap_or_default(),
                file_path: sound.file_path.clone(),
                volume: sound.volume,
                muted: sound.is_muted,
            });
            self.dialog_capture = false;
        }
    }

    fn draw_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(dialog) = self.dialog.as_ref() else {
            return;
        };
        let p = self.palette();
        let ctx = ui.ctx().clone();
        ui.interact(
            ui.max_rect(),
            ui.make_persistent_id("voxora-modal-block"),
            Sense::click(),
        );
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, Color32::from_black_alpha(150));

        let mut save = false;
        let mut cancel = false;
        let confirm_delete = false;

        let (title, is_sound) = match dialog {
            Dialog::CategoryNew { .. } => (self.lang.t("Category_NewTitle"), false),
            Dialog::CategoryRename { .. } => (self.lang.t("Category_RenameTitle"), false),
            Dialog::CategoryDelete { .. } => (self.lang.t("Category_Delete"), false),
            Dialog::SoundRemove { .. } => (self.lang.t("Main_CardRemove"), false),
            Dialog::SoundEdit { .. } => (self.lang.t("Edit_Title"), true),
        };

        egui::Window::new("voxora-dialog")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::NONE
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(14.0)
                    .inner_margin(Margin::same(20)),
            )
            .show(&ctx, |ui| {
                ui.set_width(if is_sound { 420.0 } else { 360.0 });
                ui.label(
                    RichText::new(&title)
                        .font(theme::font_display(17.0))
                        .color(p.ink),
                );
                ui.add_space(10.0);
                match self.dialog.as_mut() {
                    Some(Dialog::CategoryNew { name }) => {
                        ui.label(
                            RichText::new(self.lang.t("Category_NamePrompt"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            TextEdit::singleline(name)
                                .desired_width(f32::INFINITY)
                                .hint_text(self.lang.t("Category_NewTitle"))
                                .text_color(p.ink),
                        );
                    }
                    Some(Dialog::CategoryRename { name, .. }) => {
                        ui.label(
                            RichText::new(self.lang.t("Category_RenamePrompt"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            TextEdit::singleline(name)
                                .desired_width(f32::INFINITY)
                                .text_color(p.ink),
                        );
                    }
                    Some(Dialog::CategoryDelete { name, .. }) => {
                        ui.label(
                            RichText::new(self.lang.tf("Category_DeleteConfirm", &[name]))
                                .font(theme::font_body(12.0))
                                .color(p.ink2),
                        );
                    }
                    Some(Dialog::SoundRemove { name, .. }) => {
                        let text = if self.lang.language() == "en" {
                            format!("Remove ‘{name}’ from the library? The audio file will not be deleted.")
                        } else {
                            format!("‘{name}’ kütüphaneden kaldırılsın mı? Ses dosyası silinmez.")
                        };
                        ui.label(RichText::new(text).font(theme::font_body(12.0)).color(p.ink2));
                    }
                    Some(Dialog::SoundEdit {
                        name,
                        hotkey,
                        behavior,
                        hold,
                        favorite,
                        category,
                        icon,
                        color,
                        file_path,
                        volume,
                        muted,
                        ..
                    }) => {
                        ui.label(
                            RichText::new(self.lang.t("Edit_Name"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        ui.add(
                            TextEdit::singleline(name)
                                .desired_width(f32::INFINITY)
                                .text_color(p.ink),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(self.lang.t("Edit_Icon"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            ui.add(
                                TextEdit::singleline(icon)
                                    .desired_width(54.0)
                                    .hint_text("🎵")
                                    .text_color(p.ink),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(self.lang.t("Edit_Category"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            ui.add(
                                TextEdit::singleline(color)
                                    .desired_width(78.0)
                                    .hint_text("#FF5C35")
                                    .text_color(p.ink),
                            );
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(self.lang.t("Edit_File"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(truncate(
                                        std::path::Path::new(file_path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or(file_path),
                                        30,
                                    ))
                                    .font(theme::font_mono(10.0))
                                    .color(p.ink3),
                                )
                                .truncate(),
                            );
                            if widgets::ghost_button(ui, &self.lang.t("Edit_Change"), &p, false)
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Ses",
                                        &["mp3", "wav", "ogg", "flac", "m4a", "aac", "wma", "opus"],
                                    )
                                    .pick_file()
                                {
                                    *file_path = path.to_string_lossy().to_string();
                                }
                            }
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(self.lang.t("Edit_Volume"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            let mut value = *volume;
                            if widgets::fader(ui, &mut value, 1.5, 180.0, &p) {
                                *volume = value;
                            }
                            ui.label(
                                RichText::new(format!("{}%", (*volume * 100.0).round() as i32))
                                    .font(theme::font_mono(11.0))
                                    .color(p.ink2),
                            );
                        });
                        ui.checkbox(muted, self.lang.t("Edit_Muted"));
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(self.lang.t("Edit_Category"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        let mut options: Vec<(Option<String>, String)> = vec![(
                            None,
                            self.lang.t("Category_NoCategory"),
                        )];
                        for cat in &self.library.data.categories {
                            options.push((Some(cat.id.clone()), cat.name.clone()));
                        }
                        let selected_label = options
                            .iter()
                            .find(|(id, _)| id == category)
                            .map(|(_, n)| n.clone())
                            .unwrap_or_else(|| self.lang.t("Category_NoCategory"));
                        egui::ComboBox::from_id_salt("dlg-category")
                            .width(300.0)
                            .selected_text(selected_label)
                            .show_ui(ui, |ui| {
                                for (id, label) in &options {
                                    if ui
                                        .selectable_label(category == id, label)
                                        .clicked()
                                    {
                                        *category = id.clone();
                                    }
                                }
                            });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(self.lang.t("Edit_Hotkey"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            let text = hotkey
                                .as_ref()
                                .map(|h| h.display())
                                .unwrap_or_else(|| "—".into());
                            let btn = if self.dialog_capture {
                                if self.lang.language() == "en" {
                                    "Press a key…".to_string()
                                } else {
                                    "Tuşa bas…".to_string()
                                }
                            } else {
                                text
                            };
                            if widgets::ghost_button(ui, &btn, &p, false).clicked() {
                                self.dialog_capture = true;
                            }
                            if hotkey.is_some()
                                && widgets::ghost_button(ui, &self.lang.t("Edit_Clear"), &p, true)
                                    .clicked()
                            {
                                *hotkey = None;
                            }
                        });
                        if self.dialog_capture {
                            if let Some(captured) = capture_hotkey(ui.ctx()) {
                                *hotkey = captured;
                                self.dialog_capture = false;
                            }
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let behavior_label = if self.lang.language() == "en" {
                                "Behavior"
                            } else {
                                "Davranış"
                            };
                            ui.label(
                                RichText::new(behavior_label)
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            );
                            egui::ComboBox::from_id_salt("dlg-behavior")
                                .width(180.0)
                                .selected_text(match behavior {
                                    1 => self.lang.t("Settings_BehaviorOverlap"),
                                    2 => self.lang.t("Settings_BehaviorToggle"),
                                    _ => self.lang.t("Settings_BehaviorRestart"),
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        behavior,
                                        0,
                                        self.lang.t("Settings_BehaviorRestart"),
                                    );
                                    ui.selectable_value(
                                        behavior,
                                        1,
                                        self.lang.t("Settings_BehaviorOverlap"),
                                    );
                                    ui.selectable_value(
                                        behavior,
                                        2,
                                        self.lang.t("Settings_BehaviorToggle"),
                                    );
                                });
                        });
                        ui.add_space(6.0);
                        let hold_label = if self.lang.language() == "en" {
                            "Hold to play".to_string()
                        } else {
                            "Basılı tutarak çal".to_string()
                        };
                        ui.checkbox(hold, hold_label);
                        ui.checkbox(favorite, self.lang.t("Edit_Favorite"));
                    }
                    None => {}
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let save_label = if matches!(self.dialog, Some(Dialog::SoundRemove { .. })) {
                        self.lang.t("Main_CardRemove")
                    } else if matches!(self.dialog, Some(Dialog::CategoryDelete { .. })) {
                        self.lang.t("Category_Delete")
                    } else {
                        self.lang.t("Common_Save")
                    };
                    if widgets::icon_text_button(
                        ui,
                        I_CHECK,
                        &save_label,
                        widgets::ButtonKind::Primary,
                        &p,
                    )
                    .clicked()
                    {
                        save = true;
                    }
                    if widgets::ghost_button(ui, &self.lang.t("Common_Cancel"), &p, false).clicked()
                    {
                        cancel = true;
                    }
                });
            });

        if save {
            match self.dialog.take() {
                Some(Dialog::CategoryNew { name }) => {
                    if !name.trim().is_empty() {
                        self.library.add_category(&name);
                        self.refresh_filtered();
                    }
                }
                Some(Dialog::CategoryRename { id, name }) => {
                    if !name.trim().is_empty() {
                        self.library.rename_category(&id, &name);
                        self.refresh_filtered();
                    }
                }
                Some(Dialog::CategoryDelete { id, .. }) => {
                    self.library.remove_category(&id);
                    if let CategorySel::Id(current) = &self.category {
                        if current == &id {
                            self.category = CategorySel::All;
                        }
                    }
                    self.refresh_filtered();
                }
                Some(Dialog::SoundRemove { id, .. }) => {
                    self.stop_sound(&id);
                    self.library.remove_sound(&id);
                    if let Some(samples) = self.decoded.remove(&id) {
                        self.decoded_bytes = self.decoded_bytes.saturating_sub(samples.len() * std::mem::size_of::<f32>());
                    }
                    self.decoded_order.retain(|key| key != &id);
                    self.refresh_filtered();
                    self.rebuild_hotkeys();
                }
                Some(Dialog::SoundEdit {
                    id,
                    name,
                    hotkey,
                    behavior,
                    hold,
                    favorite,
                    category,
                    icon,
                    color,
                    file_path,
                    volume,
                    muted,
                }) => {
                    let name_trim = name.trim().to_string();
                    self.update_sound(&id, |s| {
                        if !name_trim.is_empty() {
                            s.name = name_trim.clone();
                        }
                        s.hotkey = hotkey.clone();
                        s.behavior = PlaybackBehavior::from_i32(behavior);
                        s.hold_to_play = hold;
                        s.favorite = favorite;
                        s.category_id = category.clone();
                        s.icon = if icon.trim().is_empty() {
                            None
                        } else {
                            Some(icon.trim().to_string())
                        };
                        s.color = if color.trim().is_empty() {
                            None
                        } else {
                            Some(color.trim().to_string())
                        };
                        s.file_path = file_path.clone();
                        s.volume = volume.clamp(0.0, 1.5);
                        s.is_muted = muted;
                    });
                    self.decoded.remove(&id);
                    self.library.save();
                    self.refresh_filtered();
                    self.rebuild_hotkeys();
                }
                None => {}
            }
        } else if cancel {
            self.dialog = None;
            self.dialog_capture = false;
        }
        let _ = confirm_delete;
    }

    fn msg(&self, tr: &str, en: &str) -> String {
        if self.lang.language() == "en" {
            en.to_string()
        } else {
            tr.to_string()
        }
    }

    fn export_sounds_dialog(&mut self) {
        let Some(target) = rfd::FileDialog::new()
            .set_file_name("voxora-sesler.zip")
            .add_filter("ZIP", &["zip"])
            .save_file()
        else {
            return;
        };
        match crate::bundle::export_sounds(&self.library.data, &target) {
            Ok(n) => {
                let text = self.msg(
                    &format!("{n} ses dosyası dışa aktarıldı."),
                    &format!("{n} sound files exported."),
                );
                self.toast(text, widgets::ToastKind::Info);
            }
            Err(e) => self.toast(
                self.msg(
                    &format!("Dışa aktarma başarısız: {e}"),
                    &format!("Export failed: {e}"),
                ),
                widgets::ToastKind::Error,
            ),
        }
    }

    fn export_library_dialog(&mut self) {
        let Some(target) = rfd::FileDialog::new()
            .set_file_name("voxora-kitaplik.zip")
            .add_filter("Voxora kitaplığı", &["zip"])
            .save_file()
        else {
            return;
        };
        match crate::bundle::export_library(&self.library.data, &target) {
            Ok(n) => {
                let text = self.msg(
                    &format!("Kitaplık dışa aktarıldı ({n} dosya)."),
                    &format!("Library exported ({n} files)."),
                );
                self.toast(text, widgets::ToastKind::Info);
            }
            Err(e) => self.toast(
                self.msg(
                    &format!("Dışa aktarma başarısız: {e}"),
                    &format!("Export failed: {e}"),
                ),
                widgets::ToastKind::Error,
            ),
        }
    }

    fn import_library_dialog(&mut self) {
        let Some(source) = rfd::FileDialog::new()
            .add_filter("Voxora kitaplığı", &["zip"])
            .pick_file()
        else {
            return;
        };
        let dir = voxora_core::settings::sounds_dir();
        match crate::bundle::import_library(&source, &dir) {
            Ok(result) => {
                let mut added_categories = 0usize;
                for cat in result.categories {
                    let exists = self
                        .library
                        .data
                        .categories
                        .iter()
                        .any(|c| c.id == cat.id || c.name == cat.name);
                    if !exists {
                        let mut cat = cat;
                        cat.order = self.library.data.next_order;
                        self.library.data.next_order += 1;
                        self.library.data.categories.push(cat);
                        added_categories += 1;
                    }
                }
                let added_sounds = result.sounds.len();
                for mut sound in result.sounds {
                    sound.order = self.library.data.next_order;
                    self.library.data.next_order += 1;
                    self.library.data.sounds.push(sound);
                }
                self.library.save();
                self.refresh_filtered();
                self.rebuild_hotkeys();
                let mut text = self.msg(
                    &format!(
                        "{added_sounds} ses, {added_categories} kategori içe aktarıldı."
                    ),
                    &format!(
                        "{added_sounds} sounds and {added_categories} categories imported."
                    ),
                );
                if result.missing > 0 {
                    text.push(' ');
                    text.push_str(&self.msg(
                        &format!("{} dosya eksik.", result.missing),
                        &format!("{} files missing.", result.missing),
                    ));
                }
                self.toast(text, widgets::ToastKind::Info);
            }
            Err(e) => self.toast(
                self.msg(
                    &format!("İçe aktarma başarısız: {e}"),
                    &format!("Import failed: {e}"),
                ),
                widgets::ToastKind::Error,
            ),
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.rail);
        let nav_w = 212.0;
        let footer_h = 62.0;
        let header_h = 88.0;
        let nav = Rect::from_min_size(full.min, vec2(nav_w, full.height()));
        let footer = Rect::from_min_max(
            pos2(full.left() + nav_w, full.bottom() - footer_h),
            full.right_bottom(),
        );
        let header = Rect::from_min_max(
            pos2(full.left() + nav_w, full.top()),
            pos2(full.right(), full.top() + header_h),
        );
        let content = Rect::from_min_max(
            pos2(full.left() + nav_w, header.bottom()),
            pos2(full.right(), footer.top()),
        );

        ui.painter().rect_filled(
            Rect::from_min_max(pos2(nav.right() - 1.0, nav.top()), nav.right_bottom()),
            0.0,
            p.divider,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(header.min, pos2(header.right(), header.top() + 1.0)),
            0.0,
            p.divider,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(header.left(), header.bottom() - 1.0),
                header.right_bottom(),
            ),
            0.0,
            p.divider,
        );
        ui.painter().rect_filled(footer, 0.0, p.panel);
        ui.painter().rect_filled(
            Rect::from_min_max(footer.min, pos2(footer.right(), footer.top() + 1.0)),
            0.0,
            p.divider,
        );

        // ---------------- sol gezinme
        let nui = child(ui, nav);
        let logo = Rect::from_min_size(pos2(nav.left() + 18.0, nav.top() + 18.0), vec2(30.0, 30.0));
        nui.painter().rect_filled(logo, 8.0, p.accent);
        for (i, h) in [9.0f32, 15.0, 6.0].iter().enumerate() {
            let x = logo.center().x + (i as f32 - 1.0) * 7.0;
            nui.painter().rect_filled(
                Rect::from_center_size(pos2(x, logo.center().y), vec2(2.5, *h)),
                1.25,
                p.on_accent,
            );
        }
        widgets::display_at(
            nui.painter(),
            pos2(logo.right() + 10.0, logo.top() + 8.0),
            "VOXORA",
            14.0,
            p.ink,
            Align2::LEFT_CENTER,
        );
        widgets::mono_at(
            nui.painter(),
            pos2(logo.right() + 10.0, logo.bottom() - 5.0),
            &self.t("Settings_Title").to_uppercase(),
            9.0,
            p.ink3,
            Align2::LEFT_CENTER,
        );

        let tabs = [
            (SettingsTab::General, "Settings_NavGeneral"),
            (SettingsTab::Audio, "Settings_NavAudio"),
            (SettingsTab::Behavior, "Settings_NavBehavior"),
            (SettingsTab::Features, "Settings_NavFeatures"),
            (SettingsTab::Updates, "Settings_NavUpdates"),
            (SettingsTab::About, "Settings_NavAbout"),
        ];
        let mut y = nav.top() + 72.0;
        for (tab, key) in tabs {
            let row = Rect::from_min_size(pos2(nav.left() + 10.0, y), vec2(nav.width() - 20.0, 36.0));
            let resp = nui.interact(row, nui.make_persistent_id(("set-nav", key)), Sense::click());
            let selected = self.tab == tab;
            if selected || resp.hovered() {
                nui.painter().rect_filled(row, 8.0, p.surface_hi);
            }
            if selected {
                nui.painter().rect_filled(
                    Rect::from_min_size(
                        pos2(row.left() + 2.0, row.center().y - 9.0),
                        vec2(3.0, 18.0),
                    ),
                    1.5,
                    p.accent,
                );
            }
            nui.painter().text(
                pos2(row.left() + 16.0, row.center().y),
                Align2::LEFT_CENTER,
                self.t(key),
                theme::font_body(12.5),
                if selected { p.ink } else { p.ink2 },
            );
            if resp.clicked() {
                self.tab = tab;
            }
            y += 40.0;
        }
        // alt: kurulum sihirbazı
        let wizard = Rect::from_min_size(
            pos2(nav.left() + 14.0, nav.bottom() - 52.0),
            vec2(nav.width() - 28.0, 34.0),
        );
        let wr = nui.interact(wizard, nui.make_persistent_id("set-wizard"), Sense::click());
        nui.painter().rect(
            wizard,
            8.0,
            if wr.hovered() { p.surface_hi } else { p.surface },
            Stroke::new(1.0, p.input_border),
            StrokeKind::Inside,
        );
        widgets::paint_icon_text(
            &nui,
            wizard.center(),
            I_SETTINGS,
            &self.t("Settings_OnboardingButton"),
            11.0,
            11.5,
            7.0,
            p.ink2,
            p.ink,
        );
        if wr.clicked() {
            self.onboarding_step = 0;
            self.view = View::Onboarding;
        }

        // ---------------- başlık
        widgets::display_at(
            ui.painter(),
            pos2(header.left() + 28.0, header.top() + 20.0),
            &self.tab_title(),
            21.0,
            p.ink,
            Align2::LEFT_TOP,
        );
        widgets::text_at(
            ui.painter(),
            pos2(header.left() + 28.0, header.top() + 52.0),
            &self.lang.t(match self.tab {
                SettingsTab::General => "Settings_PageGeneralHint",
                SettingsTab::Audio => "Settings_PageAudioHint",
                SettingsTab::Behavior => "Settings_PageBehaviorHint",
                SettingsTab::Features => "Settings_PageFeaturesHint",
                SettingsTab::Updates => "Settings_PageUpdatesHint",
                SettingsTab::About => "Settings_PageAboutHint",
            }),
            12.0,
            p.ink2,
            Align2::LEFT_TOP,
        );

        // ---------------- alt eylemler
        let save = Rect::from_min_size(
            pos2(footer.right() - 20.0 - 110.0, footer.center().y - 17.0),
            vec2(110.0, 34.0),
        );
        let save_resp = ui.interact(save, ui.make_persistent_id("set-save"), Sense::click());
        ui.painter().rect_filled(
            save,
            8.0,
            if save_resp.hovered() { p.accent_hover } else { p.accent },
        );
        widgets::text_at(
            ui.painter(),
            save.center(),
            &self.t("Common_Save"),
            12.5,
            p.on_accent,
            Align2::CENTER_CENTER,
        );
        let cancel = Rect::from_min_size(
            pos2(save.left() - 10.0 - 110.0, save.top()),
            vec2(110.0, 34.0),
        );
        let cancel_resp = ui.interact(cancel, ui.make_persistent_id("set-cancel"), Sense::click());
        ui.painter().rect(
            cancel,
            8.0,
            if cancel_resp.hovered() { p.surface_hi } else { p.surface },
            Stroke::new(1.0, p.input_border),
            StrokeKind::Inside,
        );
        widgets::text_at(
            ui.painter(),
            cancel.center(),
            &self.t("Common_Cancel"),
            12.5,
            p.ink,
            Align2::CENTER_CENTER,
        );
        if save_resp.clicked() {
            self.save_settings(ui.ctx());
        }
        if cancel_resp.clicked() {
            self.cancel_settings();
        }

        // ---------------- içerik
        let mut cui = child(
            ui,
            Rect::from_min_max(
                pos2(content.left() + 28.0, content.top() + 18.0),
                pos2(content.right() - 22.0, content.bottom() - 18.0),
            ),
        );
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(&mut cui, |ui| {
                match self.tab {
                    SettingsTab::General => self.settings_general(ui, &p),
                    SettingsTab::Audio => self.settings_audio(ui, &p),
                    SettingsTab::Behavior => self.settings_behavior(ui, &p),
                    SettingsTab::Features => self.settings_features(ui, &p),
                    SettingsTab::Updates => self.settings_updates(ui, &p),
                    SettingsTab::About => self.settings_about(ui, &p),
                }
            });
    }

    fn tab_title(&self) -> String {
        self.t(match self.tab {
            SettingsTab::General => "Settings_NavGeneral",
            SettingsTab::Audio => "Settings_NavAudio",
            SettingsTab::Behavior => "Settings_NavBehavior",
            SettingsTab::Features => "Settings_NavFeatures",
            SettingsTab::Updates => "Settings_NavUpdates",
            SettingsTab::About => "Settings_NavAbout",
        })
    }

    fn settings_card(ui: &mut egui::Ui, p: &Palette, title: &str, add: impl FnOnce(&mut egui::Ui)) {
        Frame::NONE
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.divider))
            .corner_radius(12.0)
            .inner_margin(Margin::same(20))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(title)
                        .font(theme::font_display(14.0))
                        .color(p.ink),
                );
                ui.add_space(10.0);
                add(ui);
            });
        ui.add_space(14.0);
    }

    fn settings_general(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut draft = self.draft.take().unwrap_or_default();
        Self::settings_card(ui, p, &self.lang.t("Settings_Appearance"), |ui| {
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_Theme"));
                ui.add_space(10.0);
                let mut dark = draft.theme != "light";
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(if dark { self.lang.t("Settings_ThemeDark") } else { self.lang.t("Settings_ThemeLight") })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut dark, true, self.lang.t("Settings_ThemeDark"));
                        ui.selectable_value(&mut dark, false, self.lang.t("Settings_ThemeLight"));
                    });
                draft.theme = if dark { "dark".into() } else { "light".into() };
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_Language"));
                ui.add_space(10.0);
                let mut english = draft.language == "en";
                egui::ComboBox::from_id_salt("language")
                    .selected_text(if english { "English" } else { "Türkçe" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut english, false, "Türkçe");
                        ui.selectable_value(&mut english, true, "English");
                    });
                draft.language = if english { "en".into() } else { "tr".into() };
            });
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_Startup"), |ui| {
            ui.checkbox(&mut draft.auto_start, self.lang.t("Settings_AutoStart"));
            ui.checkbox(&mut draft.start_minimized, self.lang.t("Settings_StartMinimized"));
            ui.checkbox(&mut draft.close_to_tray, self.lang.t("Settings_CloseToTray"));
            ui.checkbox(&mut draft.copy_on_import, self.lang.t("Settings_CopyOnImport"));
        });
        if ui.button(self.lang.t("Settings_OnboardingButton")).clicked() {
            self.draft = Some(draft);
            self.cancel_settings();
            self.onboarding_step = 0;
            self.view = View::Onboarding;
            return;
        }
        self.draft = Some(draft);
    }

    fn settings_audio(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut draft = self.draft.take().unwrap_or_default();
        let mut auto_setup = false;
        let outs: Vec<String> = self.devices_out.iter().map(|d| d.name.clone()).collect();
        let ins: Vec<String> = self.devices_in.iter().map(|d| d.name.clone()).collect();
        Self::settings_card(ui, p, &self.lang.t("Settings_RoutingTitle"), |ui| {
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_Output"));
                ui.add_space(8.0);
                if widgets::combo(ui, "set-out", &mut draft.output_device_id, &outs, 300.0, p) {}
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_Microphone"));
                ui.add_space(8.0);
                if widgets::combo(ui, "set-mic", &mut draft.mic_device_id, &ins, 300.0, p) {}
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_Monitor"));
                ui.add_space(8.0);
                if widgets::combo(ui, "set-mon", &mut draft.monitor_device_id, &outs, 300.0, p) {}
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(self.lang.t("Settings_AutoSetup")).clicked() {
                    auto_setup = true;
                }
                ui.label(self.lang.t("Settings_DeviceNamesBody"));
            });
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_DeviceNamesTitle"), |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(self.lang.t("Settings_DeviceNamesBody"))
                        .font(theme::font_body(11.5))
                        .color(p.ink2),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            if widgets::icon_text_button(
                ui,
                I_MIC,
                &self.lang.t("Settings_DeviceNamesButton"),
                widgets::ButtonKind::Plain,
                p,
            )
            .clicked()
            {
                self.rename_endpoints();
            }
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_Ducking"), |ui| {
            ui.checkbox(&mut draft.duck_mic_while_playing, self.lang.t("Settings_DuckEnable"));
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_DuckRatio"));
                let mut value = draft.duck_factor;
                if widgets::fader(ui, &mut value, 1.0, 220.0, p) {
                    draft.duck_factor = value;
                }
            });
        });
        Self::settings_card(ui, p, &self.msg("Motor", "Engine"), |ui| {
            ui.horizontal(|ui| {
                ui.label(self.msg("Arabellek", "Buffer"));
                let mut value = draft.engine_latency_ms as f32;
                if widgets::fader(ui, &mut value, 200.0, 200.0, p) {
                    draft.engine_latency_ms = (value.round() as i32).clamp(20, 200);
                }
                ui.label(
                    RichText::new(format!("{} ms", draft.engine_latency_ms))
                        .font(theme::font_mono(11.0))
                        .color(p.ink2),
                );
            });
            ui.horizontal(|ui| {
                ui.label(self.msg("Önbellek", "Cache"));
                let mut value = draft.max_cache_mb as f32;
                if widgets::fader(ui, &mut value, 2048.0, 200.0, p) {
                    draft.max_cache_mb = (value.round() as i32).clamp(32, 2048);
                }
                ui.label(
                    RichText::new(format!("{} MB", draft.max_cache_mb))
                        .font(theme::font_mono(11.0))
                        .color(p.ink2),
                );
            });
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_VoiceChanger"), |ui| {
            ui.checkbox(&mut draft.voice_changer.enabled, self.lang.t("Settings_VoiceChangerEnable"));
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_VoicePreset"));
                let mut preset = draft.voice_changer.preset;
                egui::ComboBox::from_id_salt("voice-preset")
                    .selected_text(format!("{preset:?}"))
                    .show_ui(ui, |ui| {
                        for p in [
                            VoiceChangerPreset::Natural,
                            VoiceChangerPreset::Chipmunk,
                            VoiceChangerPreset::Deep,
                            VoiceChangerPreset::Robot,
                            VoiceChangerPreset::Echo,
                            VoiceChangerPreset::Telephone,
                            VoiceChangerPreset::Custom,
                        ] {
                            ui.selectable_value(&mut preset, p, format!("{p:?}"));
                        }
                    });
                if preset != draft.voice_changer.preset && preset != VoiceChangerPreset::Custom {
                    draft.voice_changer = VoiceChangerSettings::for_preset(preset);
                } else {
                    draft.voice_changer.preset = preset;
                }
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_VoicePitch"));
                let mut pitch = draft.voice_changer.pitch_semitones as f32;
                if widgets::fader(ui, &mut pitch, 24.0, 220.0, p) {
                    draft.voice_changer.pitch_semitones = (pitch.round() as i32 - 12).clamp(-12, 12);
                }
                ui.label(format!("{}", draft.voice_changer.pitch_semitones));
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_VoiceRobot"));
                let mut mix = draft.voice_changer.robot_mix;
                if widgets::fader(ui, &mut mix, 1.0, 220.0, p) {
                    draft.voice_changer.robot_mix = mix;
                }
            });
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_VoiceEcho"));
                let mut mix = draft.voice_changer.echo_mix;
                if widgets::fader(ui, &mut mix, 1.0, 220.0, p) {
                    draft.voice_changer.echo_mix = mix;
                }
            });
            ui.checkbox(&mut draft.voice_changer.telephone, self.lang.t("Settings_VoiceTelephone"));
            if ui.button(self.lang.t("Settings_VoiceReset")).clicked() {
                draft.voice_changer = VoiceChangerSettings::default();
            }
        });
        if auto_setup {
            self.draft = Some(draft);
            self.auto_configure_cable();
            draft = self.draft.take().unwrap_or_default();
        }
        self.draft = Some(draft);
    }

    fn settings_behavior(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut draft = self.draft.take().unwrap_or_default();
        Self::settings_card(ui, p, &self.lang.t("Settings_Behaviour"), |ui| {
            ui.checkbox(&mut draft.suppress_hotkeys, self.lang.t("Settings_SuppressHotkeys"));
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_DefaultBehavior"));
                let mut behavior = draft.default_behavior;
                egui::ComboBox::from_id_salt("default-behavior")
                    .selected_text(match behavior {
                        1 => self.lang.t("Settings_BehaviorOverlap"),
                        2 => self.lang.t("Settings_BehaviorToggle"),
                        _ => self.lang.t("Settings_BehaviorRestart"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut behavior, 0, self.lang.t("Settings_BehaviorRestart"));
                        ui.selectable_value(&mut behavior, 1, self.lang.t("Settings_BehaviorOverlap"));
                        ui.selectable_value(&mut behavior, 2, self.lang.t("Settings_BehaviorToggle"));
                    });
                draft.default_behavior = behavior;
            });
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_RadialTitle"), |ui| {
            ui.label(self.lang.t("Settings_RadialHint"));
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_RadialHotkey"));
                let text = draft
                    .radial_menu_hotkey
                    .as_ref()
                    .map(|h| h.display())
                    .unwrap_or_else(|| "—".into());
                if ui.button(text).clicked() {
                    self.hotkey_capture = Some(HotkeyTarget::Radial);
                }
            });
            if self.hotkey_capture == Some(HotkeyTarget::Radial) {
                ui.label("Bir tuşa basın… (Esc iptal)");
                if let Some(hk) = capture_hotkey(ui.ctx()) {
                    draft.radial_menu_hotkey = hk;
                    self.hotkey_capture = None;
                }
            }
        });
        Self::settings_card(ui, p, &self.lang.t("Settings_Hotkeys"), |ui| {
            ui.horizontal(|ui| {
                ui.label(self.lang.t("Settings_StopAllHotkey"));
                let text = draft
                    .stop_all_hotkey
                    .as_ref()
                    .map(|h| h.display())
                    .unwrap_or_else(|| "—".into());
                if ui.button(text).clicked() {
                    self.hotkey_capture = Some(HotkeyTarget::StopAll);
                }
            });
            if self.hotkey_capture == Some(HotkeyTarget::StopAll) {
                ui.label("Bir tuşa basın… (Esc iptal)");
                if let Some(hk) = capture_hotkey(ui.ctx()) {
                    draft.stop_all_hotkey = hk;
                    self.hotkey_capture = None;
                }
            }
            if !self.hotkey_conflicts.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "⚠ {}: {}",
                        self.lang.t("Settings_HotkeyConflictTitle"),
                        self.hotkey_conflicts.join(", ")
                    ))
                    .font(theme::font_body(11.0))
                    .color(p.warning),
                );
            }
        });
        self.draft = Some(draft);
    }

    fn settings_features(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut draft = self.draft.take().unwrap_or_default();
        Self::settings_card(ui, p, &self.lang.t("Settings_Features"), |ui| {
            for def in features::ALL_FEATURES {
                let mut enabled = draft
                    .features
                    .get(def.id.key())
                    .copied()
                    .unwrap_or(def.default_enabled);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut enabled, self.lang.t(def.id.name_key()));
                    match def.availability {
                        FeatureAvailability::Beta => {
                            widgets::chip(ui, &self.lang.t("FeatureStatus_Beta"), p.accent, p);
                        }
                        FeatureAvailability::ComingSoon => {
                            widgets::chip(ui, &self.lang.t("FeatureStatus_ComingSoon"), p.warning, p);
                        }
                        FeatureAvailability::Available => {}
                    }
                });
                ui.label(
                    RichText::new(self.lang.t(def.id.desc_key()))
                        .font(theme::font_body(11.0))
                        .color(p.ink3),
                );
                if def.availability == FeatureAvailability::ComingSoon {
                    continue;
                }
                draft.features.insert(def.id.key().to_string(), enabled);
                ui.add_space(6.0);
            }
        });
        self.draft = Some(draft);
    }

    fn settings_updates(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut draft = self.draft.take().unwrap_or_default();
        Self::settings_card(ui, p, &self.lang.t("Settings_Updates"), |ui| {
            ui.label(format!("Voxora v{APP_VERSION}"));
            ui.checkbox(&mut draft.check_for_updates, self.lang.t("Settings_CheckForUpdates"));
            if ui.button(self.lang.t("Settings_CheckNow")).clicked() {
                match platform::check_update(APP_VERSION) {
                    Some(info) => {
                        self.update = Some(info);
                        self.show_update_banner = true;
                        self.toast(self.lang.t("Settings_UpdateAvailable"), widgets::ToastKind::Info);
                    }
                    None => self.toast(self.lang.t("Settings_UpToDate"), widgets::ToastKind::Info),
                }
            }
            if let Some(update) = self.update.clone() {
                ui.label(format!(
                    "{}: v{}",
                    self.lang.t("Settings_UpdateAvailable"),
                    update.latest
                ));
                ui.add_space(4.0);
                if self.update_rx.is_some() {
                    let total = self.update_total.max(1);
                    let ratio = (self.update_received as f32 / total as f32).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(ratio)
                            .desired_width(320.0)
                            .text(format!(
                                "{} / {} MB",
                                self.update_received / 1_048_576,
                                total / 1_048_576
                            )),
                    );
                    ui.label(
                        RichText::new(self.msg(
                            "İndirme sürüyor; bitince kurucu açılır.",
                            "Downloading; installer will launch when done.",
                        ))
                        .font(theme::font_body(11.0))
                        .color(p.ink3),
                    );
                } else if widgets::icon_text_button(
                    ui,
                    I_REFRESH,
                    &self.lang.t("Settings_DownloadUpdate"),
                    widgets::ButtonKind::Primary,
                    p,
                )
                .clicked()
                {
                    self.start_update_download();
                }
                if widgets::ghost_button(ui, &self.t("Guide_OpenDownload"), p, false).clicked() {
                    open_url(&update.url);
                }
            }
        });
        self.draft = Some(draft);
    }

    fn settings_about(&mut self, ui: &mut egui::Ui, p: &Palette) {
        Self::settings_card(ui, p, &self.lang.t("Settings_About"), |ui| {
            ui.label(
                RichText::new(format!("Voxora v{APP_VERSION} — Rust portu"))
                    .font(theme::font_display(16.0))
                    .color(p.ink),
            );
            ui.label(
                RichText::new(self.lang.t("Settings_AboutTagline"))
                    .font(theme::font_body(12.0))
                    .color(p.ink2),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Veri: {}", voxora_core::settings::base_dir().display()))
                    .font(theme::font_mono(10.5))
                    .color(p.ink3),
            );
            ui.label(
                RichText::new("Orijinal C# uygulamasının özellikleriyle eşlenik Rust portu. Bazı Windows entegrasyonları (cihaz adı değiştirme, tepsi, güncelleme indirme) henüz taşınmadı.")
                    .font(theme::font_body(11.0))
                    .color(p.ink3),
            );
        });
    }

    fn draw_onboarding(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.bg);
        let card = Rect::from_center_size(
            full.center(),
            vec2(900.0f32.min(full.width() - 40.0), 625.0f32.min(full.height() - 40.0)),
        );
        ui.painter()
            .rect_filled(card.translate(vec2(0.0, 10.0)), 20.0, Color32::from_black_alpha(110));
        ui.painter()
            .rect(card, 20.0, p.panel, Stroke::new(1.0, p.divider), StrokeKind::Inside);

        let pad = 32.0;
        let header_y = card.top() + 24.0;
        let logo = Rect::from_min_size(pos2(card.left() + pad, header_y), vec2(34.0, 34.0));
        ui.painter().rect_filled(logo, 9.0, p.accent);
        for (i, h) in [10.0f32, 18.0, 7.0].iter().enumerate() {
            let x = logo.center().x + (i as f32 - 1.0) * 8.0;
            ui.painter().rect_filled(
                Rect::from_center_size(pos2(x, logo.center().y), vec2(3.0, *h)),
                1.5,
                p.on_accent,
            );
        }
        widgets::display_at(
            ui.painter(),
            pos2(logo.right() + 11.0, logo.center().y),
            "VOXORA",
            16.0,
            p.ink,
            Align2::LEFT_CENTER,
        );
        let sep_x = logo.right() + 70.0;
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(sep_x, logo.center().y), vec2(1.0, 18.0)),
            0.0,
            p.divider,
        );
        widgets::mono_at(
            ui.painter(),
            pos2(sep_x + 13.0, logo.center().y),
            &self.lang.tf(
                "Onboarding_StepOf",
                &[&(self.onboarding_step + 1).to_string(), "5"],
            ),
            10.0,
            p.ink3,
            Align2::LEFT_CENTER,
        );

        let close = Rect::from_min_size(pos2(card.right() - pad - 30.0, header_y + 2.0), vec2(30.0, 30.0));
        let close_resp = ui.interact(close, ui.scope_id().with("onb-close"), Sense::click());
        if close_resp.hovered() {
            ui.painter().rect_filled(close, 8.0, p.surface_hi);
        }
        widgets::icon_glyph_at(
            ui,
            close.center(),
            I_CLOSE,
            14.0,
            if close_resp.hovered() { p.ink } else { p.ink3 },
        );

        let step = self.onboarding_step.min(4);
        let (headline, body) = match step {
            0 => (
                self.t("Onboarding_WelcomeTitle"),
                self.t("Onboarding_WelcomeBody"),
            ),
            1 => (
                self.t("Onboarding_ThemeTitle"),
                self.t("Onboarding_ThemeBody"),
            ),
            2 => (
                self.t("Onboarding_AudioTitle"),
                self.t("Onboarding_AudioBody"),
            ),
            3 => (
                self.t("Onboarding_DeviceTitle"),
                self.t("Onboarding_DeviceBody"),
            ),
            _ => (
                self.t("Onboarding_ReadyTitle"),
                self.t("Onboarding_ReadyBody"),
            ),
        };

        let left_x = card.left() + pad;
        let left_w = 470.0;
        let text_top = card.top() + 150.0;
        widgets::display_at(
            ui.painter(),
            pos2(left_x, text_top),
            &headline,
            34.0,
            p.ink,
            Align2::LEFT_TOP,
        );
        let mut body_ui = child(
            ui,
            Rect::from_min_size(pos2(left_x, text_top + 58.0), vec2(left_w, 100.0)),
        );
        body_ui.add(
            egui::Label::new(
                RichText::new(body)
                    .font(theme::font_body(13.5))
                    .color(p.ink2),
            )
            .wrap(),
        );

        if step == 0 {
            for (i, key) in [
                "Onboarding_WelcomePoint1",
                "Onboarding_WelcomePoint2",
                "Onboarding_WelcomePoint3",
            ]
            .iter()
            .enumerate()
            {
                let row_y = card.top() + 300.0 + i as f32 * 42.0;
                let tile = Rect::from_center_size(pos2(left_x + 15.0, row_y + 15.0), vec2(30.0, 30.0));
                ui.painter().circle_filled(tile.center(), 15.0, p.accent_dim);
                let glyph = [I_ADD, I_VOLUME, I_MIC][i];
                widgets::icon_glyph_at(ui, tile.center(), glyph, 13.0, p.accent);
                widgets::text_at(
                    ui.painter(),
                    pos2(left_x + 42.0, row_y + 15.0),
                    &self.t(key),
                    12.5,
                    p.ink2,
                    Align2::LEFT_CENTER,
                );
            }
        }

        // ---------------- sağ panel
        let right_x = card.right() - pad - 336.0;
        let mut right_ui = child(
            ui,
            Rect::from_min_size(pos2(right_x, card.top() + 130.0), vec2(336.0, 420.0)),
        );
        match step {
            0 => {
                Frame::NONE
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(18))
                    .show(&mut right_ui, |ui| {
                        ui.set_width(300.0);
                        ui.label(
                            RichText::new(self.lang.t("Onboarding_LanguageTitle"))
                                .font(theme::font_body(14.0))
                                .color(p.ink),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(self.lang.t("Onboarding_LanguageBody"))
                                    .font(theme::font_body(11.5))
                                    .color(p.ink2),
                            )
                            .wrap(),
                        );
                        ui.add_space(14.0);
                        let is_tr = self.lang.language() == "tr";
                        if widgets::option_row(ui, "Türkçe", is_tr, None, &p).clicked() {
                            self.lang.set_language("tr");
                            self.settings.settings.language = "tr".into();
                        }
                        ui.add_space(8.0);
                        if widgets::option_row(ui, "English", !is_tr, None, &p).clicked() {
                            self.lang.set_language("en");
                            self.settings.settings.language = "en".into();
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("TR   ·   EN")
                                .font(theme::font_mono(9.0))
                                .color(p.ink3),
                        );
                    });
            }
            1 => {
                Frame::NONE
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(18))
                    .show(&mut right_ui, |ui| {
                        ui.set_width(300.0);
                        let dark = self.mode == ThemeMode::Dark;
                        if widgets::option_row(
                            ui,
                            &self.lang.t("Onboarding_ThemeDark"),
                            dark,
                            Some(Color32::from_rgb(0x0E, 0x0C, 0x0A)),
                            &p,
                        )
                        .clicked()
                        {
                            self.mode = ThemeMode::Dark;
                            self.settings.settings.theme = "dark".into();
                            theme::apply(ui.ctx(), self.mode);
                        }
                        ui.add_space(8.0);
                        if widgets::option_row(
                            ui,
                            &self.lang.t("Onboarding_ThemeLight"),
                            !dark,
                            Some(Color32::from_rgb(0xF1, 0xEC, 0xE3)),
                            &p,
                        )
                        .clicked()
                        {
                            self.mode = ThemeMode::Light;
                            self.settings.settings.theme = "light".into();
                            theme::apply(ui.ctx(), self.mode);
                        }
                    });
            }
            2 => {
                Frame::NONE
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(18))
                    .show(&mut right_ui, |ui| {
                        ui.set_width(300.0);
                        let outs: Vec<String> =
                            self.devices_out.iter().map(|d| d.name.clone()).collect();
                        let ins: Vec<String> =
                            self.devices_in.iter().map(|d| d.name.clone()).collect();
                        ui.label(
                            RichText::new(self.lang.t("Onboarding_OutputLabel"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        let mut out = self.settings.settings.output_device_id.clone();
                        if widgets::combo(ui, "onb-out", &mut out, &outs, 290.0, &p) {
                            self.settings.settings.output_device_id = out;
                            self.mark_dirty();
                            self.restart_engine();
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(self.lang.t("Onboarding_MicLabel"))
                                .font(theme::font_body(11.5))
                                .color(p.ink2),
                        );
                        let mut mic = self.settings.settings.mic_device_id.clone();
                        if widgets::combo(ui, "onb-mic", &mut mic, &ins, 290.0, &p) {
                            self.settings.settings.mic_device_id = mic;
                            self.mark_dirty();
                            self.restart_engine();
                        }
                        ui.add_space(12.0);
                        let mut monitor = self.settings.settings.monitor_enabled;
                        if widgets::switch(ui, &mut monitor, &self.lang.t("Onboarding_MonitorLabel"), &p)
                        {
                            self.settings.settings.monitor_enabled = monitor;
                            self.mark_dirty();
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if widgets::icon_text_button(
                                ui,
                                I_VOLUME,
                                &self.lang.t("Onboarding_TestTone"),
                                widgets::ButtonKind::Soft,
                                &p,
                            )
                            .clicked()
                            {
                                self.play_test_tone();
                            }
                            ui.add_space(8.0);
                            if widgets::icon_text_button(
                                ui,
                                I_SETTINGS,
                                &self.lang.t("Settings_AutoSetup"),
                                widgets::ButtonKind::Plain,
                                &p,
                            )
                            .clicked()
                            {
                                self.auto_configure_cable();
                            }
                        });
                    });
            }
            3 => {
                Frame::NONE
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(18))
                    .show(&mut right_ui, |ui| {
                        ui.set_width(300.0);
                        if widgets::icon_text_button(
                            ui,
                            I_MIC,
                            &self.lang.t("Onboarding_RenameButton"),
                            widgets::ButtonKind::Primary,
                            &p,
                        )
                        .clicked()
                        {
                            self.rename_endpoints();
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(self.lang.t("Onboarding_RenameAdmin"))
                                .font(theme::font_body(11.0))
                                .color(p.ink3),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(self.lang.t("Onboarding_DeviceLater"))
                                .font(theme::font_body(11.0))
                                .color(p.ink3),
                        );
                    });
            }
            _ => {
                Frame::NONE
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.divider))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(18))
                    .show(&mut right_ui, |ui| {
                        ui.set_width(300.0);
                        let out = self
                            .settings
                            .settings
                            .output_device_id
                            .clone()
                            .unwrap_or_else(|| "—".into());
                        let mic = self
                            .settings
                            .settings
                            .mic_device_id
                            .clone()
                            .unwrap_or_else(|| "—".into());
                        for (icon, text) in [
                            (I_CHECK, self.lang.tf("Onboarding_SummaryAudio", &[&truncate(&out, 26)])),
                            (I_CHECK, self.lang.tf("Onboarding_SummaryMic", &[&truncate(&mic, 26)])),
                            (I_CHECK, self.lang.t("Onboarding_ShortcutSearch")),
                            (I_CHECK, self.lang.t("Onboarding_ShortcutPlay")),
                            (I_CHECK, self.lang.t("Onboarding_ShortcutStop")),
                        ] {
                            ui.horizontal(|ui| {
                                widgets::icon_glyph(ui, icon, 12.0, p.success);
                                ui.add_space(7.0);
                                ui.label(
                                    RichText::new(text)
                                        .font(theme::font_body(11.5))
                                        .color(p.ink2),
                                );
                            });
                            ui.add_space(6.0);
                        }
                    });
            }
        }

        // ---------------- alt şerit: noktalar + Devam
        let dots_y = card.bottom() - 60.0;
        for i in 0..5usize {
            let active = i == step;
            let w = if active { 18.0 } else { 6.0 };
            let x = card.left() + pad + i as f32 * 12.0;
            let dot = Rect::from_min_size(pos2(x, dots_y), vec2(w, 3.0));
            ui.painter().rect_filled(
                dot,
                1.5,
                if active { p.accent } else { p.divider },
            );
        }
        let btn = Rect::from_min_size(
            pos2(card.right() - pad - 149.0, card.bottom() - 76.0),
            vec2(149.0, 36.0),
        );
        let resp = ui.interact(btn, ui.scope_id().with("onb-next"), Sense::click());
        ui.painter().rect_filled(
            btn,
            8.0,
            if resp.hovered() { p.accent_hover } else { p.accent },
        );
        let label = if step >= 4 {
            self.lang.t("Onboarding_Finish")
        } else {
            self.lang.t("Onboarding_Next")
        };
        widgets::text_at(
            ui.painter(),
            btn.center(),
            &label,
            12.5,
            p.on_accent,
            Align2::CENTER_CENTER,
        );

        let mut finish = false;
        let mut advance = false;
        if resp.clicked() {
            if step >= 4 {
                finish = true;
            } else {
                advance = true;
            }
        }
        if close_resp.clicked() {
            finish = true;
        }
        if finish {
            self.finish_onboarding();
        } else if advance {
            self.onboarding_step += 1;
        }
    }
}

impl eframe::App for VoxoraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !self.engine_ready {
            self.engine_ready = true;
            if std::env::var("VOXORA_NO_AUDIO").is_err() {
                self.restart_engine();
            }
        }
        self.handle_hotkeys();
        self.poll_playing();
        self.handle_tray(ui.ctx());
        self.poll_devices();
        self.poll_update(ui.ctx());
        if !self.start_hidden_done {
            self.start_hidden_done = true;
            if self.settings.settings.start_minimized {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if ui.ctx().input(|i| i.viewport().close_requested())
            && self.settings.settings.close_to_tray
            && !self.allow_close
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if let Some(rx) = &self.rename_rx {
            if let Ok(outcome) = rx.try_recv() {
                self.rename_rx = None;
                match outcome {
                    crate::rename::RenameOutcome::Renamed => {
                        self.toast(self.lang.t("DeviceName_Done"), widgets::ToastKind::Info);
                        self.devices_out = voxora_audio::output_devices();
                        self.devices_in = voxora_audio::input_devices();
                    }
                    crate::rename::RenameOutcome::AlreadyNamed => {
                        self.toast(
                            self.lang.t("DeviceName_AlreadyNamed"),
                            widgets::ToastKind::Info,
                        );
                    }
                    crate::rename::RenameOutcome::NotFound => {
                        self.toast(self.lang.t("DeviceName_NotFound"), widgets::ToastKind::Warning);
                    }
                    crate::rename::RenameOutcome::AccessDenied => {
                        self.toast(
                            self.lang.t("DeviceName_AccessDenied"),
                            widgets::ToastKind::Error,
                        );
                    }
                    crate::rename::RenameOutcome::UacCancelled => {
                        self.toast(
                            self.lang.t("DeviceName_UacDenied"),
                            widgets::ToastKind::Warning,
                        );
                    }
                    crate::rename::RenameOutcome::Failed(reason) => {
                        self.toast(
                            self.lang.tf("DeviceName_Failed", &[&reason]),
                            widgets::ToastKind::Error,
                        );
                    }
                }
            }
        }
        if let Some(dirty) = self.dirty_at {
            if dirty.elapsed() > Duration::from_millis(450) {
                self.save_now();
            }
        }
        if !self.playing.is_empty() {
            ui.ctx().request_repaint_after(Duration::from_millis(150));
        } else if !self.toasts.is_empty() || self.update_rx.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        // Boşta planlı kare yok: kısayol/tepsi/cihaz olayları request_repaint ile uyandırır.
        self.handle_drops(ui.ctx());
        if ui
            .ctx()
            .input(|i| i.key_pressed(egui::Key::K) && i.modifiers.ctrl)
            && self.view == View::Main
        {
            self.focus_search = true;
        }

        match self.view {
            View::Main => self.draw_main(ui),
            View::Settings => self.draw_settings(ui),
            View::Onboarding => self.draw_onboarding(ui),
            View::Guide => self.draw_guide(ui),
        }
        if self.dialog.is_some() {
            self.draw_dialog(ui);
        }

        while let Ok(event) = radial_shared().overlay.try_recv() {
            match event {
                crate::radial_overlay::RadialEvent::Selected(id) => {
                    if let Some(sound) = self.find_sound(&id) {
                        self.trigger_sound(&sound);
                    }
                }
                crate::radial_overlay::RadialEvent::Closed => {}
            }
        }

        if !self.first_frame_done {
            self.first_frame_done = true;
            if let Some(path) = std::env::var_os("VOXORA_READY_FILE") {
                use std::time::{SystemTime, UNIX_EPOCH};
                let ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let _ = std::fs::write(path, ms.to_string());
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Radyal katmanın arkası şeffaf olmalı; ana pencere her karede tam
        // arka planı boyadığı için bu güvenli.
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_now();
        self.engine.stop();
    }
}

fn device_signature(outs: &[DeviceInfo], ins: &[DeviceInfo]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for d in outs {
        parts.push(&d.name);
    }
    parts.push("|");
    for d in ins {
        parts.push(&d.name);
    }
    parts.join("\u{1}")
}

fn spawn_device_watcher() -> std::sync::mpsc::Receiver<(Vec<DeviceInfo>, Vec<DeviceInfo>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("voxora-devices".into())
        .spawn(move || {
            let mut last = String::new();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
                let outs = voxora_audio::output_devices();
                let ins = voxora_audio::input_devices();
                let sig = device_signature(&outs, &ins);
                if sig != last {
                    last = sig;
                    if tx.send((outs, ins)).is_err() {
                        break;
                    }
                    crate::platform::wake();
                }
            }
        })
        .ok();
    rx
}

fn child<'a>(ui: &'a mut egui::Ui, rect: Rect) -> egui::Ui {
    ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::LEFT)),
    )
}

fn parse_hex_color(text: &str) -> Option<egui::Color32> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn capture_hotkey(ctx: &egui::Context) -> Option<Option<HotkeyGesture>> {
    let mut modifiers = 0u32;
    ctx.input(|i| {
        if i.modifiers.ctrl {
            modifiers |= 1;
        }
        if i.modifiers.alt {
            modifiers |= 2;
        }
        if i.modifiers.shift {
            modifiers |= 4;
        }
        if i.modifiers.mac_cmd || i.modifiers.command {
            // Windows'ta command == ctrl; Win tuşu ayrı gelmiyor.
        }
    });
    let mut result = None;
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            result = Some(None);
            return;
        }
        for event in &i.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                ..
            } = event
            {
                let vk = key_to_vk(*key);
                if vk != 0 {
                    result = Some(Some(HotkeyGesture::new(vk, modifiers)));
                }
            }
        }
    });
    result
}

fn key_to_vk(key: egui::Key) -> i32 {
    use egui::Key;
    match key {
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7A,
        Key::F12 => 0x7B,
        Key::Space => 0x20,
        Key::Enter => 0x0D,
        Key::Tab => 0x09,
        Key::Backspace => 0x08,
        Key::Delete => 0x2E,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::ArrowUp => 0x26,
        Key::ArrowDown => 0x28,
        Key::ArrowLeft => 0x25,
        Key::ArrowRight => 0x27,
        _ => 0,
    }
}

fn open_url(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

fn parse_theme(value: &str) -> Option<ThemeMode> {
    match value {
        "dark" | "koyu" => Some(ThemeMode::Dark),
        "light" | "acik" | "açık" => Some(ThemeMode::Light),
        _ => None,
    }
}

pub fn run_cli() -> eframe::Result<()> {
    let mut theme = None;
    let mut data_dir = None;
    let mut page = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => theme = args.next().as_deref().and_then(parse_theme),
            "--data" => data_dir = args.next().map(PathBuf::from),
            "--page" => page = args.next(),
            _ => {}
        }
    }

    let icon = load_icon();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("Voxora")
            .with_app_id("voxora")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([1080.0, 680.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Voxora",
        options,
        Box::new(move |cc| {
            Ok(Box::new(VoxoraApp::new(cc, data_dir.clone(), theme, page.clone())))
        }),
    )
}

fn load_icon() -> Arc<egui::IconData> {
    let bytes = include_bytes!("../assets/app.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Arc::new(egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            })
        }
        Err(_) => Arc::new(egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        }),
    }
}

pub fn version() -> &'static str {
    APP_VERSION
}
