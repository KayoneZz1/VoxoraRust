use std::io::Write;
use voxora_audio::{decode_file, VoiceProcessor};
use voxora_core::settings::{VoiceChangerPreset, VoiceChangerSettings};

fn write_wav(path: &std::path::Path, freq: f64, seconds: f64, rate: u32, mono: bool) {
    let frames = (rate as f64 * seconds) as usize;
    let channels = if mono { 1 } else { 2 };
    let data_len = frames * channels * 2;
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len as u32).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&(channels as u16).to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * channels as u32 * 2).to_le_bytes()).unwrap();
    f.write_all(&((channels * 2) as u16).to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&(data_len as u32).to_le_bytes()).unwrap();
    for i in 0..frames {
        let v = (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() * 0.5;
        let s = (v * i16::MAX as f64) as i16;
        for _ in 0..channels {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }
}

#[test]
fn decodes_mono_wav_to_stereo_48k() {
    let dir = std::env::temp_dir().join(format!("voxora-audio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.wav");
    write_wav(&path, 440.0, 0.5, 44_100, true);
    let decoded = decode_file(&path).expect("decode");
    assert!(decoded.samples.len() > 40_000);
    assert!((decoded.duration_seconds - 0.5).abs() < 0.02);
    assert!(decoded.samples.iter().any(|s| s.abs() > 0.1));
    assert!(decoded.samples.iter().all(|s| s.is_finite()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dsp_robot_and_telephone_change_signal() {
    let mut p = VoiceProcessor::new(48_000, 2);
    p.update(&VoiceChangerSettings::for_preset(VoiceChangerPreset::Robot));
    let mut buf: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
    let before = buf.clone();
    p.process(&mut buf);
    assert!(buf.iter().all(|s| s.is_finite()));
    assert!(buf.iter().zip(before.iter()).any(|(a, b)| (a - b).abs() > 0.01));

    let mut p = VoiceProcessor::new(48_000, 2);
    p.update(&VoiceChangerSettings::for_preset(VoiceChangerPreset::Telephone));
    let mut high: Vec<f32> = (0..4800)
        .map(|i| (i as f32 * 0.9).sin() * 0.3)
        .collect();
    let before_rms = rms(&high);
    p.process(&mut high);
    assert!(rms(&high) < before_rms * 0.5);
}

#[test]
fn dsp_pitch_shift_stays_finite_and_non_silent() {
    let mut p = VoiceProcessor::new(48_000, 2);
    let mut settings = VoiceChangerSettings::for_preset(VoiceChangerPreset::Chipmunk);
    settings.enabled = true;
    p.update(&settings);
    let mut buf: Vec<f32> = (0..48_000)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin() * 0.4)
        .collect();
    p.process(&mut buf);
    assert!(buf.iter().all(|s| s.is_finite()));
    assert!(rms(&buf) > 0.05);
}

#[test]
fn lists_audio_devices_without_panicking() {
    let outputs = voxora_audio::output_devices();
    let inputs = voxora_audio::input_devices();
    assert!(outputs.iter().all(|d| !d.name.is_empty()));
    assert!(inputs.iter().all(|d| !d.name.is_empty()));
}

fn rms(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt()
}
