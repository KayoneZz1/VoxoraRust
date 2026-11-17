# Voxora — English copy for AlternativeTo

## Tagline (optional, 60 characters max)

`Your sounds, one key away.`

## Short description

`Free, open-source Windows soundboard with global hotkeys, a radial menu, and simple microphone effects.`

## Full description

Voxora is a free, open-source soundboard for Windows built with Rust. It keeps a local library of your own audio clips and plays them instantly from global hotkeys or an on-screen radial menu, so you can trigger sounds while another application has focus. Sounds can be organised into categories, searched, marked as favourites, muted, and balanced with a per-sound volume slider. The mixer combines sound effects with your microphone and can optionally lower the microphone while a clip is playing.

For voice chat, Voxora routes audio through a virtual audio cable such as VB-CABLE. The cable is installed separately and selected as Voxora’s output device; the same signal can be monitored on headphones. A compact voice changer adds pitch shift, robot, echo and telephone-style effects. It is intentionally simple and is not an AI voice platform.

Voxora has no account, no ads, no analytics and no bundled sound library. Its interface is available in English and Turkish, with dark and light themes. It ships as a per-user Windows installer and as a portable build. The project is MIT-licensed. The original C# edition (2.2.1) is also available; the Rust edition (1.0.0) is the current release.

## Key features

- Local sound library with categories, favourites, instant search, per-sound volume and mute.
- Global hotkeys, restart/overlap/toggle playback modes and hold-to-play behaviour.
- A radial menu that opens without stealing focus from the game or voice chat.
- Microphone chain with pitch shift, robot, echo and telephone effects, ducking and a limiter.
- Optional local monitor on a separate output device.
- Drag-and-drop import plus ZIP library import and export.
- English/Turkish interface, dark/light themes, tray icon and start-with-Windows option.
- Per-user installer and a portable ZIP; no .NET, Electron or WebView runtime required.

## System requirements

- Windows 10 or Windows 11, 64-bit. Tested on Windows 11 25H2; Windows 10 is expected to work but was not tested.
- A 48 kHz stereo output device. A microphone is optional.
- VB-CABLE or another compatible virtual audio cable for routing sound into Discord or game voice chat. Voxora does not install a virtual audio driver.
- About 15 MB of disk space for the installed application; the installer is about 6 MB and the portable ZIP about 5.3 MB.

## Licensing and distribution

Voxora is released under the MIT licence. Source code and builds are published on GitHub. The installer is per-user and does not require administrator rights; installing a virtual audio cable may require administrator rights.

## Known limitations (do not omit)

- No built-in virtual audio driver; VB-CABLE (third-party) is required for voice-chat routing.
- No audio recorder, waveform editor, normalisation, AI voices or community sound library.
- Windows only; no macOS, Linux or mobile edition.
- The voice changer is a simple DSP effect set, not a real-time AI voice platform.
- The radial menu may not appear over exclusive-fullscreen games; multi-monitor/DPI coverage is limited.
- Builds are currently unsigned, so Windows SmartScreen may warn.
- The optional UAC path for renaming audio endpoints has not been verified on a live approval flow.
- Updates are not installed automatically; the app opens the GitHub release page.
- A few status strings may remain Turkish in the English interface.
- No account, ads, analytics or bundled sounds. If “Check for updates” is enabled, the app makes a request to the GitHub releases API; otherwise it does not need the network.

## Natural search phrases used in the copy

These phrases appear only where they read naturally: Windows soundboard, open-source soundboard, free soundpad alternative, soundboard for Discord, sound effects, global hotkeys, real-time microphone effects, portable soundboard.
