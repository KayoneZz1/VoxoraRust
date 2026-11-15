use crate::models::SoundItem;
use crate::settings::{library_path, load_json, save_json, sounds_dir, AppSettings};
use std::path::Path;

pub struct LibraryService {
    pub data: crate::models::LibraryData,
    path: std::path::PathBuf,
}

impl LibraryService {
    pub fn new(path: Option<std::path::PathBuf>) -> Self {
        let path = path.unwrap_or_else(library_path);
        let mut data = load_json::<crate::models::LibraryData>(&path).unwrap_or_default();
        let mut next = data.next_order.max(1);
        for s in &mut data.sounds {
            if s.order <= 0 {
                s.order = next;
                next += 1;
            }
        }
        for c in &mut data.categories {
            if c.order <= 0 {
                c.order = next;
                next += 1;
            }
        }
        data.next_order = next;
        Self { data, path }
    }

    pub fn save(&self) {
        let _ = save_json(&self.path, &self.data);
    }

    pub fn sounds(&self) -> &[SoundItem] {
        &self.data.sounds
    }

    pub fn categories(&self) -> &[crate::models::Category] {
        &self.data.categories
    }

    pub fn add_sound(
        &mut self,
        file: &Path,
        category_id: Option<String>,
        copy: bool,
        display_name: Option<String>,
    ) -> std::io::Result<SoundItem> {
        let target = if copy {
            let dir = sounds_dir();
            std::fs::create_dir_all(&dir)?;
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("ses");
            let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("");
            let mut candidate = dir.join(if ext.is_empty() {
                stem.to_string()
            } else {
                format!("{stem}.{ext}")
            });
            let mut n = 1;
            while candidate.exists() && !same_path(&candidate, file) {
                candidate = dir.join(if ext.is_empty() {
                    format!("{stem} ({n})")
                } else {
                    format!("{stem} ({n}).{ext}")
                });
                n += 1;
            }
            if !same_path(&candidate, file) {
                std::fs::copy(file, &candidate)?;
            }
            candidate
        } else {
            file.to_path_buf()
        };

        let name = display_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| {
                target
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Ses")
                    .to_string()
            });
        let mut item = SoundItem::new(name, target.to_string_lossy().to_string());
        item.category_id = category_id;
        item.order = self.data.next_order;
        self.data.next_order += 1;
        self.data.sounds.push(item.clone());
        self.save();
        Ok(item)
    }

    pub fn remove_sound(&mut self, id: &str) {
        self.data.sounds.retain(|s| s.id != id);
        self.save();
    }

    pub fn add_category(&mut self, name: &str) -> crate::models::Category {
        let cat = crate::models::Category {
            id: crate::models::uuid_like(),
            name: name.trim().to_string(),
            color: None,
            icon: None,
            order: self.data.next_order,
        };
        self.data.next_order += 1;
        self.data.categories.push(cat.clone());
        self.save();
        cat
    }

    pub fn rename_category(&mut self, id: &str, name: &str) {
        if let Some(c) = self.data.categories.iter_mut().find(|c| c.id == id) {
            c.name = name.trim().to_string();
            self.save();
        }
    }

    pub fn remove_category(&mut self, id: &str) {
        self.data.categories.retain(|c| c.id != id);
        for s in &mut self.data.sounds {
            if s.category_id.as_deref() == Some(id) {
                s.category_id = None;
            }
        }
        self.save();
    }

    /// Filtre: kategori kimliği (None = tümü, Some("") = favoriler) + arama.
    pub fn filtered(&self, category_id: Option<&str>, favorites: bool, query: &str) -> Vec<SoundItem> {
        let q = query.trim().to_lowercase();
        self.data
            .sounds
            .iter()
            .filter(|s| {
                if favorites && !s.favorite {
                    return false;
                }
                if let Some(cid) = category_id {
                    if !favorites && s.category_id.as_deref() != Some(cid) {
                        return false;
                    }
                }
                q.is_empty() || s.name.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn category_counts(&self) -> std::collections::HashMap<Option<String>, usize> {
        let mut map = std::collections::HashMap::new();
        map.insert(None, self.data.sounds.len());
        for s in &self.data.sounds {
            *map.entry(s.category_id.clone()).or_insert(0) += 1;
        }
        map
    }

    pub fn apply_to_settings_last_category(&self, settings: &mut AppSettings) {
        if !settings.remember_last_category {
            return;
        }
        // UI tarafında seçili kategori ayarlanır.
        let _ = settings;
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    ca.to_string_lossy().eq_ignore_ascii_case(&cb.to_string_lossy())
}

pub fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "wma" | "opus")
    )
}
