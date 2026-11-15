use anyhow::{anyhow, Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const ENGINE_RATE: u32 = 48_000;
const MAX_SAMPLES: usize = 256 * 1024 * 1024 / std::mem::size_of::<f32>();

pub struct DecodedSound {
    pub samples: Vec<f32>,
    pub duration_seconds: f64,
}

pub fn decode_file(path: &Path) -> Result<DecodedSound> {
    let file = std::fs::File::open(path).with_context(|| format!("dosya açılamadı: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| anyhow!("ses biçimi tanınamadı: {e}"))?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("ses parçası bulunamadı"))?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow!("kodek açılamadı: {e}"))?;

    let mut stereo = Vec::<f32>::new();
    let mut source_format: Option<(u32, usize)> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let spec = *decoded.spec();
        let source_channels = spec.channels.count().max(1);
        if let Some(previous) = source_format {
            if previous != (spec.rate, source_channels) {
                return Err(anyhow!("ses biçimi dosya içinde değişiyor"));
            }
        } else {
            source_format = Some((spec.rate, source_channels));
        }
        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let interleaved = sample_buf.samples();
        let frames = interleaved.len() / source_channels;
        if stereo.len().saturating_add(frames.saturating_mul(2)) > MAX_SAMPLES * 2 {
            return Err(anyhow!("ses dosyası çok büyük"));
        }
        stereo.reserve(frames * 2);
        for frame in interleaved.chunks_exact(source_channels) {
            stereo.push(frame[0]);
            stereo.push(if source_channels == 1 { frame[0] } else { frame[1] });
        }
    }

    if stereo.is_empty() {
        return Err(anyhow!("ses verisi çözülemedi"));
    }
    let source_rate = source_format.unwrap().0;

    let samples = if source_rate != ENGINE_RATE {
        resample_stereo(&stereo, source_rate, ENGINE_RATE)
    } else {
        stereo
    };

    let duration_seconds = samples.len() as f64 / 2.0 / ENGINE_RATE as f64;
    Ok(DecodedSound {
        samples,
        duration_seconds,
    })
}

fn resample_stereo(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let in_frames = input.len() / 2;
    let ratio = to as f64 / from as f64;
    let out_frames = (in_frames as f64 * ratio).floor() as usize;
    let mut out = vec![0.0f32; out_frames * 2];
    for i in 0..out_frames {
        let pos = i as f64 / ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let idx2 = (idx + 1).min(in_frames - 1);
        for ch in 0..2 {
            let a = input[idx * 2 + ch];
            let b = input[idx2 * 2 + ch];
            out[i * 2 + ch] = a + (b - a) * frac;
        }
    }
    out
}
