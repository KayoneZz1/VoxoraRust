use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use voxora_core::models::{Category, LibraryData, SoundItem};

/// Kitaplığı tek bir .zip paketine yazar: `library.json` + `sounds/<dosya>`.
/// Dönen değer: paketlenen ses dosyası sayısı.
pub fn export_library(lib: &LibraryData, target: &Path) -> std::io::Result<usize> {
    let file = std::fs::File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut exported = lib.clone();
    let mut count = 0usize;
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for sound in &mut exported.sounds {
        let path = Path::new(&sound.file_path);
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ses.bin")
            .to_string();
        let mut entry = name.clone();
        let mut n = 1;
        while used.contains(&entry) {
            let stem = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("ses");
            let ext = Path::new(&name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            entry = if ext.is_empty() {
                format!("{stem} ({n})")
            } else {
                format!("{stem} ({n}).{ext}")
            };
            n += 1;
        }
        used.insert(entry.clone());
        let mut data = std::fs::File::open(path)?;
        zip.start_file(format!("sounds/{entry}"), options)?;
        std::io::copy(&mut data, &mut zip)?;
        sound.file_path = format!("sounds/{entry}");
        count += 1;
    }
    zip.start_file("library.json", options)?;
    zip.write_all(&serde_json::to_vec_pretty(&exported)?)?;
    zip.finish()?;
    Ok(count)
}

/// Yalnızca ses dosyalarını paketler (kitaplık meta verisi olmadan).
pub fn export_sounds(lib: &LibraryData, target: &Path) -> std::io::Result<usize> {
    let file = std::fs::File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0usize;
    for sound in &lib.sounds {
        let path = Path::new(&sound.file_path);
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ses.bin");
        let mut data = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        data.read_to_end(&mut bytes)?;
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
        count += 1;
    }
    zip.finish()?;
    Ok(count)
}

/// Paketi çıkarır ve kitaplığa eklenecek kayıtları döndürür.
/// Ses dosyaları `sounds_dir` altına yazılır, yollar yeni konuma göre güncellenir.
pub struct ImportResult {
    pub categories: Vec<Category>,
    pub sounds: Vec<SoundItem>,
    pub missing: usize,
}

pub fn import_library(zip_path: &Path, sounds_dir: &Path) -> std::io::Result<ImportResult> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::create_dir_all(sounds_dir)?;

    let mut lib: Option<LibraryData> = None;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if name.contains("..") {
            continue;
        }
        if name == "library.json" {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            lib = serde_json::from_slice(&bytes).ok();
            continue;
        }
        if let Some(rest) = name.strip_prefix("sounds/") {
            if rest.is_empty() || rest.contains('/') || rest.contains(':') {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            entries.push((rest.to_string(), bytes));
        }
    }

    let Some(mut data) = lib else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "pakette library.json yok",
        ));
    };

    // Dosyaları yaz ve eşleşen seslerin yolunu güncelle.
    let mut missing = 0usize;
    let mut by_name: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    for (name, bytes) in entries {
        let mut target = sounds_dir.join(&name);
        let mut n = 1;
        let stem = Path::new(&name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ses")
            .to_string();
        let ext = Path::new(&name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        while target.exists() {
            target = sounds_dir.join(if ext.is_empty() {
                format!("{stem} ({n})")
            } else {
                format!("{stem} ({n}).{ext}")
            });
            n += 1;
        }
        std::fs::write(&target, bytes)?;
        by_name.insert(name, target);
    }

    let mut used_ids: std::collections::HashSet<String> =
        data.sounds.iter().map(|s| s.id.clone()).collect();
    for sound in &mut data.sounds {
        let original = Path::new(&sound.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if let Some(path) = by_name.get(&original) {
            sound.file_path = path.to_string_lossy().to_string();
        } else if !Path::new(&sound.file_path).is_file() {
            missing += 1;
        }
        if used_ids.contains(&sound.id) {
            sound.id = voxora_core::models::uuid_like();
            used_ids.insert(sound.id.clone());
        }
    }

    Ok(ImportResult {
        categories: std::mem::take(&mut data.categories),
        sounds: std::mem::take(&mut data.sounds),
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_same_named_files_preserves_each_sound() {
        let root = std::env::temp_dir().join(format!("voxora-bundle-{}", voxora_core::models::uuid_like()));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("sound.wav"), b"first").unwrap();
        std::fs::write(b.join("sound.wav"), b"second").unwrap();
        let mut lib = LibraryData::default();
        lib.sounds.push(SoundItem::new("first".into(), a.join("sound.wav").to_string_lossy().into_owned()));
        lib.sounds.push(SoundItem::new("second".into(), b.join("sound.wav").to_string_lossy().into_owned()));
        let zip = root.join("bundle.zip");
        assert_eq!(export_library(&lib, &zip).unwrap(), 2);
        let result = import_library(&zip, &root.join("imported")).unwrap();
        assert_eq!(result.missing, 0);
        assert_eq!(std::fs::read(&result.sounds[0].file_path).unwrap(), b"first");
        assert_eq!(std::fs::read(&result.sounds[1].file_path).unwrap(), b"second");
    }
}
