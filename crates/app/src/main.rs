#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bundle;
mod lang;
mod platform;
mod radial_overlay;
mod rename;
mod theme;
mod tray;
mod widgets;


fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("Voxora {} (Rust port)", app::version());
        return Ok(());
    }
    if args.iter().any(|a| a == rename::RENAME_SWITCH) {
        std::process::exit(rename::rename_elevated());
    }
    if args.iter().any(|a| a == "--self-test") {
        return run_self_test();
    }

    let _instance = match platform::SingleInstance::acquire("VoxoraRust_SingleInstance") {
        Some(instance) => instance,
        None => {
            focus_existing();
            return Ok(());
        }
    };

    app::run_cli()
}

fn run_self_test() -> eframe::Result<()> {
    let settings = voxora_core::SettingsService::new(None);
    let library = voxora_core::LibraryService::new(None);
    let outputs = voxora_audio::output_devices();
    let inputs = voxora_audio::input_devices();
    let lang = lang::Lang::new(&settings.settings.language);
    let key = lang.t("Main_AddSound");
    println!("settings: {}", settings.path().display());
    println!("theme: {}", settings.settings.theme);
    println!("library sounds: {}", library.sounds().len());
    println!("outputs: {}", outputs.len());
    println!("inputs: {}", inputs.len());
    println!("localized: {key}");
    if outputs.is_empty() {
        println!("SELFTEST: output device yok (sanal makine olabilir)");
        return Ok(());
    }
    if let Some(sound) = library.sounds().first() {
        match voxora_audio::decode_file(std::path::Path::new(&sound.file_path)) {
            Ok(decoded) => {
                println!(
                    "decoded: {} ({:.2} sn, {} örnek)",
                    sound.name,
                    decoded.duration_seconds,
                    decoded.samples.len()
                );
                let mut engine = voxora_audio::AudioEngine::new();
                if let Err(e) = engine.start(None, None, None) {
                    println!("SELFTEST: motor başlatılamadı: {e}");
                    return Ok(());
                }
                engine.play("selftest", std::sync::Arc::new(decoded.samples), 0.6);
                std::thread::sleep(std::time::Duration::from_millis(350));
                let (fx, out) = engine.levels();
                println!("levels: fx={fx:.3} out={out:.3}");
                engine.stop();
                if fx <= 0.001 && out <= 0.001 {
                    println!("SELFTEST FAIL: ses üretilmedi");
                    std::process::exit(2);
                }
            }
            Err(e) => {
                println!("SELFTEST: çözme hatası: {e}");
                std::process::exit(3);
            }
        }
    }
    // İkinci çıkış (lokal dinleme) akışının gerçekten açıldığını doğrula.
    let outs = voxora_audio::output_devices();
    let default_name = outs
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.clone());
    if let Some(monitor) = outs
        .iter()
        .map(|d| d.name.clone())
        .find(|n| Some(n) != default_name.as_ref())
    {
        let mut engine2 = voxora_audio::AudioEngine::new();
        match engine2.start(None, Some(&monitor), None) {
            Ok(()) => {
                println!(
                    "monitor: {} active={} error={:?}",
                    monitor,
                    engine2.monitor_active(),
                    engine2.last_error
                );
                let active = engine2.monitor_active();
                engine2.stop();
                if !active {
                    println!("SELFTEST FAIL: monitör akışı açılmadı");
                    std::process::exit(4);
                }
            }
            Err(e) => println!("monitor başlatılamadı: {e}"),
        }
    }
    println!("SELFTEST OK");
    Ok(())
}


fn focus_existing() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let title: Vec<u16> = "Voxora"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}
