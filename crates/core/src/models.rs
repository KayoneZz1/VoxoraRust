use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HotkeyGesture {
    #[serde(rename = "Modifiers", default)]
    pub modifiers: u32,
    #[serde(rename = "VirtualKey", default)]
    pub virtual_key: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum HotkeyModifiers {
    #[default]
    None = 0,
    Ctrl = 1,
    Alt = 2,
    Shift = 4,
    Win = 8,
}

impl HotkeyGesture {
    pub fn new(virtual_key: i32, modifiers: u32) -> Self {
        Self { modifiers, virtual_key }
    }

    pub fn is_valid(&self) -> bool {
        self.virtual_key != 0
    }

    pub fn has(&self, flag: HotkeyModifiers) -> bool {
        self.modifiers & (flag as u32) != 0
    }

    pub fn display(&self) -> String {
        if !self.is_valid() {
            return String::new();
        }
        let mut parts: Vec<&str> = Vec::new();
        if self.has(HotkeyModifiers::Ctrl) {
            parts.push("Ctrl");
        }
        if self.has(HotkeyModifiers::Alt) {
            parts.push("Alt");
        }
        if self.has(HotkeyModifiers::Shift) {
            parts.push("Shift");
        }
        if self.has(HotkeyModifiers::Win) {
            parts.push("Win");
        }
        parts.push(""); // replaced below
        let key = vk_name(self.virtual_key);
        let mut out = parts[..parts.len() - 1].join(" + ");
        if !out.is_empty() {
            out.push_str(" + ");
        }
        out.push_str(&key);
        out
    }
}

pub fn vk_name(vk: i32) -> String {
    match vk {
        0x01 => "Sol Tık".into(),
        0x02 => "Sağ Tık".into(),
        0x04 => "Orta Tık".into(),
        0x25 => "Sol".into(),
        0x26 => "Yukarı".into(),
        0x27 => "Sağ".into(),
        0x28 => "Aşağı".into(),
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x21 => "PgUp".into(),
        0x22 => "PgDn".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x5B => "Sol Win".into(),
        0x5C => "Sağ Win".into(),
        0x6A => "Num *".into(),
        0x6B => "Num +".into(),
        0x6D => "Num -".into(),
        0x6E => "Num .".into(),
        0x6F => "Num /".into(),
        0x90 => "Num Lock".into(),
        0x91 => "Scroll Lock".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        _ => format!("0x{vk:02X}"),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[repr(i32)]
pub enum PlaybackBehavior {
    #[default]
    Restart = 0,
    Overlap = 1,
    Toggle = 2,
}

impl From<i32> for PlaybackBehavior {
    fn from(value: i32) -> Self {
        Self::from_i32(value)
    }
}

impl From<PlaybackBehavior> for i32 {
    fn from(value: PlaybackBehavior) -> Self {
        value as i32
    }
}

impl PlaybackBehavior {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Overlap,
            2 => Self::Toggle,
            _ => Self::Restart,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Category {
    #[serde(rename = "Id", default = "new_id")]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Color", default)]
    pub color: Option<String>,
    #[serde(rename = "Icon", default)]
    pub icon: Option<String>,
    #[serde(rename = "Order", default)]
    pub order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundItem {
    #[serde(rename = "Id", default = "new_id")]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "FilePath", default)]
    pub file_path: String,
    #[serde(rename = "CategoryId", default)]
    pub category_id: Option<String>,
    #[serde(rename = "Color", default)]
    pub color: Option<String>,
    #[serde(rename = "Icon", default)]
    pub icon: Option<String>,
    #[serde(rename = "Hotkey", default)]
    pub hotkey: Option<HotkeyGesture>,
    #[serde(rename = "Volume", default = "one")]
    pub volume: f32,
    #[serde(rename = "IsMuted", default)]
    pub is_muted: bool,
    #[serde(rename = "Behavior", default)]
    pub behavior: PlaybackBehavior,
    #[serde(rename = "HoldToPlay", default)]
    pub hold_to_play: bool,
    #[serde(rename = "Favorite", default)]
    pub favorite: bool,
    #[serde(rename = "Order", default)]
    pub order: i32,
    #[serde(rename = "AddedUtc", default)]
    pub added_utc: String,
    #[serde(rename = "LastPlayedUtc", default)]
    pub last_played_utc: Option<String>,
    #[serde(rename = "DurationSeconds", default)]
    pub duration_seconds: f64,
}

fn one() -> f32 {
    1.0
}

fn new_id() -> String {
    uuid_like()
}

pub fn uuid_like() -> String {
    // Zaman + sayaç tabanlı, bağımlılıksız benzersiz kimlik (C# Guid biçiminde).
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let a = (nanos >> 32) as u32;
    let b = (nanos >> 16) as u16;
    let d = ((nanos as u16) ^ (c as u16)) as u16;
    let e = (c as u64 & 0xFFFF_FFFF_FFFF) | 0x0000_0000_0000_1000;
    format!("{a:08x}-{b:04x}-4{:03x}-{d:04x}-{e:012x}", (nanos as u16) & 0x0FFF)
}

impl SoundItem {
    pub fn new(name: String, file_path: String) -> Self {
        Self {
            id: uuid_like(),
            name,
            file_path,
            category_id: None,
            color: None,
            icon: None,
            hotkey: None,
            volume: 1.0,
            is_muted: false,
            behavior: PlaybackBehavior::Restart,
            hold_to_play: false,
            favorite: false,
            order: 0,
            added_utc: String::new(),
            last_played_utc: None,
            duration_seconds: 0.0,
        }
    }

    pub fn file_exists(&self) -> bool {
        std::path::Path::new(&self.file_path).is_file()
    }

    pub fn duration_text(&self) -> String {
        if !self.file_exists() {
            return "Dosya yok".into();
        }
        if self.duration_seconds <= 0.0 {
            return String::new();
        }
        let total = self.duration_seconds;
        if total >= 60.0 {
            format!("{}:{:02}", (total / 60.0) as u64, (total % 60.0) as u64)
        } else {
            format!("{:.1} sn", total)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryData {
    #[serde(default = "one_i32")]
    pub version: i32,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub sounds: Vec<SoundItem>,
    #[serde(rename = "nextOrder", default = "one_i32")]
    pub next_order: i32,
}

fn one_i32() -> i32 {
    1
}

impl Default for LibraryData {
    fn default() -> Self {
        Self {
            version: 1,
            categories: Vec::new(),
            sounds: Vec::new(),
            next_order: 1,
        }
    }
}
