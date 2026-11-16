#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use std::fs;
use std::io::Cursor;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONERROR, MB_ICONQUESTION, MB_YESNO, MessageBoxW,
};
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VERSION: &str = "1.0.0";
const PAYLOAD: &[u8] = include_bytes!("../../../dist/VoxoraRust-1.0.0-win-x64.zip");
const UNINSTALL_SCRIPT: &str = include_str!("../../../installer/uninstall.ps1");

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn message(text: &str, title: &str, style: u32) -> i32 {
    unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide(title).as_ptr(), style) }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let silent = args.iter().any(|a| a == "--silent" || a == "/S");
    let no_launch = args.iter().any(|a| a == "--no-launch");
    let no_shortcuts = args.iter().any(|a| a == "--no-shortcuts");
    let no_registry = args.iter().any(|a| a == "--no-registry");

    let default_dest = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Programs")
        .join("VoxoraRust");
    let dest = args
        .iter()
        .position(|a| a == "--dest")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or(default_dest);

    if !silent {
        let text = format!(
            "Voxora Rust {} kurulsun mu?\n\nKurulum yeri:\n{}",
            VERSION,
            dest.display()
        );
        if message(
            &text,
            "Voxora Rust 1.0.0",
            MB_YESNO | MB_ICONQUESTION,
        ) != IDYES
        {
            return;
        }
    }

    if let Err(err) = install(&dest, no_shortcuts, no_registry) {
        message(
            &format!("Kurulum tamamlanamadi:\n\n{err}"),
            "Voxora Rust",
            MB_ICONERROR,
        );
        std::process::exit(1);
    }

    let exe = dest.join("voxora.exe");
    if !no_launch {
        let _ = Command::new(&exe).current_dir(&dest).spawn();
    }

    if !silent {
        message(
            "Voxora Rust kuruldu.\n\nKaldirmak icin: Ayarlar > Uygulamalar > Voxora Rust.",
            "Voxora Rust",
            0,
        );
    }
}

fn install(dest: &Path, no_shortcuts: bool, no_registry: bool) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dest)?;

    let mut archive = zip::ZipArchive::new(Cursor::new(PAYLOAD))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let target = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut output)?;
    }

    let script_path = dest.join("uninstall.ps1");
    fs::write(&script_path, UNINSTALL_SCRIPT)?;

    if !no_shortcuts {
        create_shortcuts(dest)?;
    }
    if !no_registry {
        register_uninstall(dest, &script_path)?;
    }
    Ok(())
}

fn create_shortcuts(dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let exe = dest.join("voxora.exe");
    let quote = |value: &Path| format!("'{}'", value.to_string_lossy().replace('\'', "''"));
    let icon = format!("{},0", exe.to_string_lossy().replace('\'', "''"));
    // OneDrive yonlendirmesi icin masaustu/Baslat yollari PowerShell'de cozulur.
    let script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $paths = @((Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs\\Voxora Rust.lnk'), \
                     (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Voxora Rust.lnk')); \
         foreach ($p in $paths) {{ \
           $s = $ws.CreateShortcut($p); \
           $s.TargetPath = {exe}; \
           $s.WorkingDirectory = {work}; \
           $s.IconLocation = '{icon}'; \
           $s.Save() \
         }}",
        exe = quote(&exe),
        work = quote(dest),
        icon = icon
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if !status.success() {
        return Err("Kisayollar olusturulamadi.".into());
    }
    Ok(())
}

fn register_uninstall(dest: &Path, script: &Path) -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\VoxoraRust"
    ))?;
    let exe = dest.join("voxora.exe");
    let uninstall = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
        script.display()
    );
    key.set_value("DisplayName", &"Voxora Rust")?;
    key.set_value("DisplayVersion", &VERSION)?;
    key.set_value("Publisher", &"ObsQrr")?;
    key.set_value("InstallLocation", &dest.to_string_lossy().to_string())?;
    key.set_value("DisplayIcon", &exe.to_string_lossy().to_string())?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    key.set_value("UninstallString", &uninstall)?;
    key.set_value("QuietUninstallString", &uninstall)?;
    Ok(())
}
