# Voxora vs. Soundpad and Voicemod

Verified on 2026-11-17 from the vendors’ public pages and AlternativeTo listings. Vendor feature lists are marketing claims, not independent benchmarks. Voxora claims come from the Rust 1.0.0 source, README and release build.

## Snapshot

| | Voxora (Rust) | Soundpad | Voicemod |
|---|---|---|---|
| Licence | **MIT, open source** | Proprietary | Proprietary |
| Price | **Free** | US$4.99 one-time (official site; AlternativeTo lists $5) | Freemium; PRO subscription or lifetime licence, prices shown in the app |
| Platform | Windows 10/11, 64-bit (tested on Windows 11 25H2) | Windows 7–11, 32/64-bit | Windows/macOS desktop; companion mobile app |
| Current version | 1.0.0 (Rust); original C# edition 2.2.1 | 4.0.30 | Vendor does not publish a version number on the page |
| Account required | **No** | No | Yes for PRO/cloud features |
| Built-in virtual audio device | **No** — VB-CABLE or similar required | Yes, installed by Soundpad | Yes, Voicemod Virtual Microphone |
| Recorder / editor | No | Yes, built-in recorder and editor | Yes, recorder and Instant Replay |
| Community sound library | No | No | Yes, 800,000+ community sounds |
| AI voices | No | Not advertised | Yes, 200+ voices and Voicelab |
| Global hotkeys | Yes | Yes | Yes |
| Radial menu | **Yes** | Not listed | Not listed |
| Portable build | **Yes** | No (official site says no portable version) | Not listed |
| Sound organisation | Categories, favourites, search, per-sound volume/mute | Categories, sound lists, hotbar, search, normalisation | Soundboards, community library, keybinds |

## Where Voxora competes

- **Free and open source.** MIT licence with public source; Soundpad is paid and Voicemod is freemium/proprietary.
- **No account or bundled content.** Bring your own sounds; no community library, ads or analytics. Voicemod requires an account for its service features.
- **Native, small footprint.** Rust + egui/OpenGL application, no Electron/WebView runtime. Installer ~6 MB, portable ZIP ~5.3 MB, executable ~13.4 MB. This is a size statement, not a measured CPU/RAM claim.
- **Radial menu.** F1 opens a transparent ring over the current application without taking focus; the mouse wheel pages through sounds. This is a different interaction model from a list or grid.
- **Flexible local mixing.** Per-sound volume, mute, restart/overlap/toggle modes, microphone ducking, optional local monitor and a simple DSP voice changer.
- **Portable option.** A single ZIP can run without installation, alongside a per-user installer.

## Where Voxora is behind

- **No virtual audio driver.** The user must install VB-CABLE (free third-party) and select it as the output device. Soundpad and Voicemod install their own virtual microphone.
- **No recorder, waveform editor or normalisation.** Soundpad is stronger for capturing and trimming clips.
- **No community library or AI voices.** Voicemod has a much larger voice and sound catalogue, Voicelab and cloud features.
- **Basic voice changer.** Voxora offers pitch, robot, echo and telephone-style DSP; it is not a competitor to Voicemod’s AI voice platform.
- **Windows only.** No macOS, Linux or mobile edition.
- **Smaller ecosystem.** No mobile companion, cloud sync, Stream Deck plugin or integrations advertised by the larger products.
- **Unsigned builds and limited QA coverage.** SmartScreen may warn; multi-monitor, exclusive-fullscreen and live UAC endpoint-renaming paths are not fully verified.

## Honest positioning for AlternativeTo

Voxora is not a feature-for-feature replacement for Voicemod, and it is not a better recorder than Soundpad. It is a good fit for users who want a **free, open-source, local-first Windows soundboard** with hotkeys, a radial menu, a portable build and no account. The listing should make the VB-CABLE requirement and the missing recorder/community features explicit; otherwise reviewers and users will rightly treat the profile as misleading.

## Sources

- Soundpad official: https://www.soundpad.net/ — version 4.0.30, Windows 7–11, recorder/editor, normalisation, no virtual cables required, no portable version.
- Soundpad on AlternativeTo: https://alternativeto.net/software/soundpad/about/ — proprietary, US$5 one-time, Windows/Steam, 29 alternatives.
- Voicemod official: https://www.voicemod.net/ — real-time voice changer, soundboard, 200+ voices, 800,000+ community sounds, Voicelab, recorder, virtual microphone, mobile app.
- Voicemod on AlternativeTo: https://alternativeto.net/software/voicemod-for-desktop/about/ — freemium, proprietary, Windows/macOS/mobile, account-based PRO.
- Voxora Rust 1.0.0: https://github.com/ObsQrr/VoxoraRust and https://github.com/ObsQrr/VoxoraRust/releases/tag/v1.0.0 — MIT, Windows builds, VB-CABLE requirement, portable and installer packages.
