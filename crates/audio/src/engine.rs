use crate::decode::ENGINE_RATE;
use crate::dsp::VoiceProcessor;
use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use voxora_core::settings::VoiceChangerSettings;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn output_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    host.output_devices()
        .map(|iter| {
            iter.filter_map(|d| d.name().ok())
                .map(|name| DeviceInfo {
                    is_default: default_name.as_deref() == Some(name.as_str()),
                    name,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn input_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    host.input_devices()
        .map(|iter| {
            iter.filter_map(|d| d.name().ok())
                .map(|name| DeviceInfo {
                    is_default: default_name.as_deref() == Some(name.as_str()),
                    name,
                })
                .collect()
        })
        .unwrap_or_default()
}

struct Voice {
    id: String,
    samples: Arc<Vec<f32>>,
    pos: usize,
    volume: f32,
}

struct EngineState {
    voices: Vec<Voice>,
    mic_ring: VecDeque<f32>,
    mic_enabled: bool,
    mic_volume: f32,
    fx_volume: f32,
    duck_enabled: bool,
    duck_factor: f32,
    duck_current: f32,
    mic: VoiceProcessor,
    fx_peak: f32,
    out_peak: f32,
    mic_scratch: Vec<f32>,
    wake: Arc<Condvar>,
}

impl EngineState {
    fn new(
        _channels: usize,
        sample_rate: u32,
        wake: Arc<Condvar>,
    ) -> Self {
        Self {
            voices: Vec::new(),
            mic_ring: VecDeque::new(),
            mic_enabled: true,
            mic_volume: 1.0,
            fx_volume: 1.0,
            duck_enabled: false,
            duck_factor: 0.25,
            duck_current: 1.0,
            mic: VoiceProcessor::new(sample_rate, 2),
            fx_peak: 0.0,
            out_peak: 0.0,
            mic_scratch: vec![0.0; 960],
            wake,
        }
    }

    /// Stereo (2 kanal) tampon üretir.
    fn render(&mut self, data: &mut [f32], monitor: &mut [f32]) {
        let frames = data.len() / 2;
        if data.is_empty() {
            return;
        }

        if self.mic_scratch.len() < frames * 2 {
            self.mic_scratch.resize(frames * 2, 0.0);
        }
        self.mic_scratch[..frames * 2].fill(0.0);
        if self.mic_enabled && !self.mic_ring.is_empty() {
            for i in 0..frames {
                let l = self.mic_ring.pop_front().unwrap_or(0.0);
                let r = self.mic_ring.pop_front().unwrap_or(l);
                self.mic_scratch[i * 2] = l;
                self.mic_scratch[i * 2 + 1] = r;
            }
            self.mic.process(&mut self.mic_scratch[..frames * 2]);
        }

        let any_fx = !self.voices.is_empty();
        let duck_target = if self.duck_enabled && any_fx {
            self.duck_factor
        } else {
            1.0
        };
        self.duck_current += (duck_target - self.duck_current) * 0.2;
        let mic_gain = if self.mic_enabled {
            self.mic_volume * self.duck_current
        } else {
            0.0
        };

        self.fx_peak = 0.0;
        self.out_peak = 0.0;

        for frame in 0..frames {
            let mut fx_l = 0.0f32;
            let mut fx_r = 0.0f32;
            for voice in self.voices.iter_mut() {
                if voice.pos + 1 < voice.samples.len() {
                    fx_l += voice.samples[voice.pos] * voice.volume;
                    fx_r += voice.samples[voice.pos + 1] * voice.volume;
                    voice.pos += 2;
                }
            }
            fx_l *= self.fx_volume;
            fx_r *= self.fx_volume;

            let mic_l = self.mic_scratch[frame * 2] * mic_gain;
            let mic_r = self.mic_scratch[frame * 2 + 1] * mic_gain;

            let out_l = limit(fx_l + mic_l);
            let out_r = limit(fx_r + mic_r);

            data[frame * 2] = out_l;
            data[frame * 2 + 1] = out_r;
            // Lokal monitör yalnız efektleri duyar (mikrofon geri beslemesi yok).
            monitor[frame * 2] = limit(fx_l);
            monitor[frame * 2 + 1] = limit(fx_r);

            self.fx_peak = self.fx_peak.max(fx_l.abs()).max(fx_r.abs());
            self.out_peak = self.out_peak.max(out_l.abs()).max(out_r.abs());
        }

        self.voices.retain(|v| v.pos + 1 < v.samples.len());
    }

    fn push_mic(&mut self, samples: impl Iterator<Item = f32>) {
        const MAX: usize = ENGINE_RATE as usize * 2;
        if !self.mic_enabled {
            return;
        }
        let was_empty = self.mic_ring.is_empty();
        for s in samples {
            if self.mic_ring.len() >= MAX {
                self.mic_ring.pop_front();
            }
            self.mic_ring.push_back(s);
        }
        if was_empty && !self.mic_ring.is_empty() {
            self.wake.notify_one();
        }
    }
}

const RING_CAP: usize = ENGINE_RATE as usize * 2 / 2; // 500 ms stereo

type Ring = Arc<Mutex<VecDeque<f32>>>;

fn push_ring(ring: &Ring, data: &[f32]) {
    if let Ok(mut q) = ring.lock() {
        for &s in data {
            if q.len() >= RING_CAP {
                q.pop_front();
            }
            q.push_back(s);
        }
    }
}

pub struct AudioEngine {
    state: Arc<Mutex<EngineState>>,
    levels: Arc<Mutex<(f32, f32)>>,
    main_ring: Ring,
    monitor_ring: Ring,
    mixer_run: Arc<AtomicBool>,
    mixer: Option<std::thread::JoinHandle<()>>,
    block_ms: u32,
    wake: Arc<Condvar>,
    monitor_enabled: Arc<AtomicBool>,
    output_stream: Option<Stream>,
    monitor_stream: Option<Stream>,
    input_stream: Option<Stream>,
    pub output_name: Option<String>,
    pub monitor_name: Option<String>,
    pub mic_name: Option<String>,
    pub last_error: Option<String>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let wake = Arc::new(Condvar::new());
        Self {
            state: Arc::new(Mutex::new(EngineState::new(
                2,
                ENGINE_RATE,
                wake.clone(),
            ))),
            levels: Arc::new(Mutex::new((0.0, 0.0))),
            main_ring: Arc::new(Mutex::new(VecDeque::new())),
            monitor_ring: Arc::new(Mutex::new(VecDeque::new())),
            mixer_run: Arc::new(AtomicBool::new(false)),
            mixer: None,
            block_ms: 10,
            wake,
            monitor_enabled: Arc::new(AtomicBool::new(false)),
            output_stream: None,
            monitor_stream: None,
            input_stream: None,
            output_name: None,
            monitor_name: None,
            mic_name: None,
            last_error: None,
        }
    }

    /// `monitor` verilirse ses aynı anda ikinci aygıta da gider (lokal dinleme).
    /// Mikser blok süresi (motor gecikmesi). 20-200 ms arası.
    pub fn configure_latency(&mut self, ms: u32) {
        self.block_ms = ms.clamp(20, 200);
    }

    pub fn start(
        &mut self,
        output: Option<&str>,
        monitor: Option<&str>,
        mic: Option<&str>,
    ) -> Result<()> {
        let result = self.start_inner(output, monitor, mic);
        if result.is_err() {
            self.stop();
        }
        result
    }

    fn start_inner(
        &mut self,
        output: Option<&str>,
        monitor: Option<&str>,
        mic: Option<&str>,
    ) -> Result<()> {
        self.stop();
        self.last_error = None;
        let host = cpal::default_host();
        let device = select_output(&host, output)?;
        let supported = choose_output_config(&device)?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.config();
        if config.channels != 2 {
            return Err(anyhow!("çıkış aygıtı stereo desteklemiyor ({} kanal)", config.channels));
        }
        if config.sample_rate.0 != ENGINE_RATE {
            return Err(anyhow!(
                "çıkış aygıtı 48 kHz desteklemiyor (desteklenen: {} Hz)",
                config.sample_rate.0
            ));
        }
        let channels = 2usize;

        {
            let mut state = self.state.lock().unwrap();
            *state = EngineState::new(channels, ENGINE_RATE, self.wake.clone());
        }
        if let Ok(mut q) = self.main_ring.lock() {
            q.clear();
        }
        if let Ok(mut q) = self.monitor_ring.lock() {
            q.clear();
        }

        // Ortak miks: tek yerde render edilir, iki çıkış aynı kuyruktan okur.
        self.mixer_run.store(true, Ordering::SeqCst);
        let state = self.state.clone();
        let levels = self.levels.clone();
        let main_ring = self.main_ring.clone();
        let monitor_ring = self.monitor_ring.clone();
        let running = self.mixer_run.clone();
        let wake = self.wake.clone();
        let monitor_enabled = self.monitor_enabled.clone();
        let block_frames = (ENGINE_RATE as usize * self.block_ms as usize / 1000).max(64);
        let sleep_ms = self.block_ms;
        self.mixer = Some(std::thread::Builder::new()
                .name("voxora-mixer".into())
                .spawn(move || {
                    let mut buf = vec![0.0f32; block_frames * 2];
                    let mut mon = vec![0.0f32; block_frames * 2];
                    while running.load(Ordering::SeqCst) {
                        let mut st = state.lock().unwrap();
                        while st.voices.is_empty() && st.mic_ring.is_empty() && running.load(Ordering::SeqCst) {
                            if let Ok(mut l) = levels.lock() {
                                *l = (0.0, 0.0);
                            }
                            st = wake.wait(st).unwrap();
                        }
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        st.render(&mut buf, &mut mon);
                        if let Ok(mut l) = levels.lock() {
                            *l = (st.fx_peak, st.out_peak);
                        }
                        drop(st);
                        push_ring(&main_ring, &buf);
                        if monitor_enabled.load(Ordering::Relaxed) {
                            push_ring(&monitor_ring, &mon);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(sleep_ms as u64));
                    }
                })
                .context("mikser iş parçacığı başlatılamadı")?);

        let err_fn = |err| eprintln!("voxora audio output error: {err}");
        let stream = match sample_format {
            SampleFormat::F32 => {
                build_output::<f32>(&device, &config, self.main_ring.clone(), err_fn)?
            }
            SampleFormat::I16 => {
                build_output::<i16>(&device, &config, self.main_ring.clone(), err_fn)?
            }
            SampleFormat::U16 => {
                build_output::<u16>(&device, &config, self.main_ring.clone(), err_fn)?
            }
            other => return Err(anyhow!("desteklenmeyen çıkış örnek biçimi: {other:?}")),
        };
        stream.play().context("çıkış akışı başlatılamadı")?;
        self.output_stream = Some(stream);
        self.output_name = Some(device.name().unwrap_or_else(|_| "Varsayılan".into()));

        // Lokal dinleme: farklı aygıt seçildiyse ikinci akış.
        if let Some(monitor_name) = monitor {
            let same = self
                .output_name
                .as_deref()
                .map(|o| o.eq_ignore_ascii_case(monitor_name))
                .unwrap_or(false);
            if !same {
                match self.start_monitor(&host, monitor_name) {
                    Ok(()) => {}
                    Err(e) => self.last_error = Some(format!("Lokal dinleme: {e}")),
                }
            }
        }

        if let Some(mic_name) = mic {
            match self.start_mic(&host, mic_name) {
                Ok(()) => {}
                Err(e) => self.last_error = Some(e.to_string()),
            }
        }
        Ok(())
    }

    fn start_monitor(&mut self, host: &cpal::Host, name: &str) -> Result<()> {
        let device = select_output(host, Some(name))?;
        let supported = choose_output_config(&device)?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.config();
        if config.channels != 2 {
            return Err(anyhow!("monitör aygıtı stereo desteklemiyor ({} kanal)", config.channels));
        }
        if config.sample_rate.0 != ENGINE_RATE {
            return Err(anyhow!(
                "48 kHz desteklemiyor ({} Hz)",
                config.sample_rate.0
            ));
        }
        let err_fn = |err| eprintln!("voxora monitor error: {err}");
        let ring = self.monitor_ring.clone();
        let stream = match sample_format {
            SampleFormat::F32 => build_output::<f32>(&device, &config, ring, err_fn)?,
            SampleFormat::I16 => build_output::<i16>(&device, &config, ring, err_fn)?,
            SampleFormat::U16 => build_output::<u16>(&device, &config, ring, err_fn)?,
            other => return Err(anyhow!("desteklenmeyen biçim: {other:?}")),
        };
        stream.play().context("monitör akışı başlatılamadı")?;
        self.monitor_stream = Some(stream);
        self.monitor_enabled.store(true, Ordering::Relaxed);
        self.monitor_name = Some(device.name().unwrap_or_else(|_| name.to_string()));
        Ok(())
    }

    fn start_mic(&mut self, host: &cpal::Host, name: &str) -> Result<()> {
        let device = select_input(host, Some(name))?;
        let supported = choose_input_config(&device)?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.config();
        let channels = config.channels as usize;
        if channels != 1 && channels != 2 {
            return Err(anyhow!("mikrofon mono veya stereo desteklemiyor ({channels} kanal)"));
        }
        if config.sample_rate.0 != ENGINE_RATE {
            return Err(anyhow!("mikrofon 48 kHz desteklemiyor"));
        }
        let state = self.state.clone();
        let err_fn = |err| eprintln!("voxora audio input error: {err}");
        let stream = match sample_format {
            SampleFormat::F32 => build_input::<f32>(&device, &config, state, channels, err_fn)?,
            SampleFormat::I16 => build_input::<i16>(&device, &config, state, channels, err_fn)?,
            SampleFormat::U16 => build_input::<u16>(&device, &config, state, channels, err_fn)?,
            other => return Err(anyhow!("desteklenmeyen mikrofon biçimi: {other:?}")),
        };
        stream.play().context("mikrofon akışı başlatılamadı")?;
        self.input_stream = Some(stream);
        self.mic_name = Some(device.name().unwrap_or_else(|_| name.to_string()));
        Ok(())
    }

    pub fn stop(&mut self) {
        self.monitor_enabled.store(false, Ordering::Relaxed);
        self.input_stream = None;
        self.monitor_stream = None;
        self.output_stream = None;
        self.monitor_name = None;
        self.mic_name = None;
        self.output_name = None;
        {
            let _state = self.state.lock().unwrap();
            self.mixer_run.store(false, Ordering::SeqCst);
            self.wake.notify_one();
        }
        if let Some(handle) = self.mixer.take() {
            let _ = handle.join();
        }
        if let Ok(mut state) = self.state.lock() {
            state.voices.clear();
            state.mic_ring.clear();
        }
        if let Ok(mut q) = self.main_ring.lock() {
            q.clear();
        }
        if let Ok(mut q) = self.monitor_ring.lock() {
            q.clear();
        }
    }

    pub fn play(&self, id: &str, samples: Arc<Vec<f32>>, volume: f32) {
        {
            let mut state = self.state.lock().unwrap();
            state.voices.push(Voice {
                id: id.to_string(),
                samples,
                pos: 0,
                volume: volume.clamp(0.0, 1.5),
            });
            self.wake.notify_one();
        }
    }

    pub fn stop_id(&self, id: &str) {
        let mut state = self.state.lock().unwrap();
        state.voices.retain(|v| v.id != id);
    }

    pub fn stop_all(&self) {
        let mut state = self.state.lock().unwrap();
        state.voices.clear();
    }

    pub fn is_playing(&self, id: &str) -> bool {
        let state = self.state.lock().unwrap();
        state.voices.iter().any(|v| v.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.state.lock().unwrap().voices.len()
    }

    pub fn set_volume(&self, id: &str, volume: f32) {
        let mut state = self.state.lock().unwrap();
        if let Some(v) = state.voices.iter_mut().find(|v| v.id == id) {
            v.volume = volume.clamp(0.0, 1.5);
        }
    }

    pub fn configure_mix(
        &self,
        mic_enabled: bool,
        mic_volume: f32,
        fx_volume: f32,
        duck: bool,
        duck_factor: f32,
    ) {
        let mut state = self.state.lock().unwrap();
        state.mic_enabled = mic_enabled;
        if !mic_enabled {
            state.mic_ring.clear();
        }
        state.mic_volume = mic_volume;
        state.fx_volume = fx_volume;
        state.duck_enabled = duck;
        state.duck_factor = duck_factor;
    }

    pub fn set_voice_settings(&self, settings: &VoiceChangerSettings) {
        let mut state = self.state.lock().unwrap();
        state.mic.update(settings);
    }

    pub fn levels(&self) -> (f32, f32) {
        *self.levels.lock().unwrap()
    }

    pub fn is_running(&self) -> bool {
        self.output_stream.is_some() && self.mixer_run.load(Ordering::SeqCst)
    }

    pub fn monitor_active(&self) -> bool {
        self.monitor_stream.is_some()
    }
}

fn select_output(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device> {
    if let Some(name) = name {
        if let Ok(iter) = host.output_devices() {
            for d in iter {
                if d.name().ok().as_deref() == Some(name) {
                    return Ok(d);
                }
            }
        }
    }
    host.default_output_device()
        .ok_or_else(|| anyhow!("çıkış aygıtı bulunamadı"))
}

fn select_input(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device> {
    if let Some(name) = name {
        if let Ok(iter) = host.input_devices() {
            for d in iter {
                if d.name().ok().as_deref() == Some(name) {
                    return Ok(d);
                }
            }
        }
    }
    host.default_input_device()
        .ok_or_else(|| anyhow!("mikrofon bulunamadı"))
}

fn choose_output_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_output_config()?;
    if default.channels() == 2 && default.sample_rate().0 == ENGINE_RATE {
        return Ok(default);
    }
    if let Ok(ranges) = device.supported_output_configs() {
        for range in ranges {
            if range.channels() == 2
                && range.min_sample_rate().0 <= ENGINE_RATE
                && range.max_sample_rate().0 >= ENGINE_RATE
            {
                return Ok(range.with_sample_rate(cpal::SampleRate(ENGINE_RATE)));
            }
        }
    }
    Ok(default)
}

fn choose_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_input_config()?;
    if (default.channels() == 2 || default.channels() == 1)
        && default.sample_rate().0 == ENGINE_RATE
    {
        return Ok(default);
    }
    if let Ok(ranges) = device.supported_input_configs() {
        for range in ranges {
            if (range.channels() == 2 || range.channels() == 1)
                && range.min_sample_rate().0 <= ENGINE_RATE
                && range.max_sample_rate().0 >= ENGINE_RATE
            {
                return Ok(range.with_sample_rate(cpal::SampleRate(ENGINE_RATE)));
            }
        }
    }
    Ok(default)
}

fn build_output<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    ring: Ring,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            if let Ok(mut q) = ring.lock() {
                for dst in data.iter_mut() {
                    let sample = q.pop_front().unwrap_or(0.0);
                    *dst = T::from_sample(sample);
                }
            } else {
                for dst in data.iter_mut() {
                    *dst = T::from_sample(0.0f32);
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn build_input<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    state: Arc<Mutex<EngineState>>,
    channels: usize,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: SizedSample + Sample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        {
            let mut converted = Vec::<f32>::new();
            let mut stereo = Vec::<f32>::new();
            move |data: &[T], _| {
                converted.clear();
                converted.reserve(data.len());
                for &sample in data {
                    converted.push(f32::from_sample(sample));
                }
                if let Ok(mut st) = state.lock() {
                    if channels == 1 {
                        stereo.clear();
                        stereo.reserve(converted.len() * 2);
                        for &v in &converted {
                            stereo.push(v);
                            stereo.push(v);
                        }
                        st.push_mic(stereo.iter().copied());
                    } else {
                        st.push_mic(converted.iter().copied());
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn limit(x: f32) -> f32 {
    if x > 0.95 || x < -0.95 {
        const KNEE: f32 = 0.05;
        let sign = x.signum();
        let ax = x.abs();
        if ax >= 1.0 {
            return sign * 0.999;
        }
        let t = (ax - 0.95) / KNEE;
        sign * (0.95 + KNEE * t.tanh() * 0.9)
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn microphone_data_wakes_waiting_mixer() {
        let wake = Arc::new(Condvar::new());
        let state = Arc::new(Mutex::new(EngineState::new(2, ENGINE_RATE, wake.clone())));
        let (ready_tx, ready_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            let mut st = worker_state.lock().unwrap();
            ready_tx.send(()).unwrap();
            while st.mic_ring.is_empty() {
                let (next, timeout) = wake.wait_timeout(st, Duration::from_secs(2)).unwrap();
                st = next;
                if timeout.timed_out() { break; }
            }
            result_tx.send(!st.mic_ring.is_empty()).unwrap();
        });
        ready_rx.recv().unwrap();
        state.lock().unwrap().push_mic([0.5, 0.5].into_iter());
        assert!(result_rx.recv_timeout(Duration::from_millis(250)).unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn disabled_microphone_does_not_fill_queue() {
        let wake = Arc::new(Condvar::new());
        let mut state = EngineState::new(2, ENGINE_RATE, wake);
        state.mic_enabled = false;
        state.push_mic([0.5, 0.5].into_iter());
        assert!(state.mic_ring.is_empty());
    }

    #[test]
    fn monitor_excludes_microphone_but_keeps_effects() {
        let wake = Arc::new(Condvar::new());
        let mut state = EngineState::new(2, ENGINE_RATE, wake);
        state.push_mic([0.25, 0.25].into_iter());
        state.voices.push(Voice {
            id: "effect".into(),
            samples: Arc::new(vec![0.1, 0.1]),
            pos: 0,
            volume: 1.0,
        });
        let mut main = [0.0; 2];
        let mut monitor = [0.0; 2];
        state.render(&mut main, &mut monitor);
        assert!((monitor[0] - 0.1).abs() < 0.001);
        assert!((main[0] - 0.35).abs() < 0.001);
    }

    #[test]
    fn same_sound_can_overlap_and_stop_together() {
        let engine = AudioEngine::new();
        let samples = Arc::new(vec![0.1, 0.1]);
        engine.play("effect", samples.clone(), 1.0);
        engine.play("effect", samples, 1.0);
        assert_eq!(engine.active_count(), 2);
        engine.stop_id("effect");
        assert_eq!(engine.active_count(), 0);
    }
}
