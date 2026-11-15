use voxora_core::settings::VoiceChangerSettings;

/// Mikrofon hattına uygulanan gerçek zamanlı ses değiştirici.
///
/// Perde kaydırma çift geçişli (two-tap) değişken gecikme hattıyla yapılır;
/// robot ring-mod, yankı geri beslemeli gecikme ve telefon bandı biquad
/// filtreleri eklenir.
pub struct VoiceProcessor {
    settings: VoiceChangerSettings,
    pitch: Option<PitchShifter>,
    pitch_semitones: i32,
    robot_phase: f64,
    echo: Vec<f32>,
    echo_index: usize,
    echo_frames: usize,
    hp: Biquad,
    lp: Biquad,
    channels: usize,
    sample_rate: f32,
}

impl VoiceProcessor {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let mut p = Self {
            settings: VoiceChangerSettings::default(),
            pitch: None,
            pitch_semitones: 0,
            robot_phase: 0.0,
            echo: Vec::new(),
            echo_index: 0,
            echo_frames: 0,
            hp: Biquad::high_pass(sample_rate as f32, 300.0, channels),
            lp: Biquad::low_pass(sample_rate as f32, 3400.0, channels),
            channels,
            sample_rate: sample_rate as f32,
        };
        p.configure_echo();
        p
    }

    pub fn update(&mut self, settings: &VoiceChangerSettings) {
        let settings = settings.clamp_copy();
        if settings.enabled && settings.pitch_semitones != 0 {
            if self.pitch_semitones != settings.pitch_semitones || self.pitch.is_none() {
                self.pitch = Some(PitchShifter::new(
                    self.sample_rate,
                    self.channels,
                    settings.pitch_semitones,
                ));
                self.pitch_semitones = settings.pitch_semitones;
            }
        } else {
            self.pitch = None;
            self.pitch_semitones = 0;
        }
        if settings.echo_delay_ms != self.settings.echo_delay_ms {
            self.settings.echo_delay_ms = settings.echo_delay_ms;
            self.configure_echo();
        }
        self.settings = settings;
        self.hp = Biquad::high_pass(self.sample_rate, 300.0, self.channels);
        self.lp = Biquad::low_pass(self.sample_rate, 3400.0, self.channels);
    }

    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn configure_echo(&mut self) {
        let frames = (self.sample_rate * self.settings.echo_delay_ms.max(50) as f32 / 1000.0) as usize;
        self.echo_frames = frames.max(1);
        self.echo = vec![0.0; self.echo_frames * self.channels];
        self.echo_index = 0;
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        if !self.settings.enabled || buf.is_empty() {
            return;
        }

        if self.settings.pitch_semitones != 0 {
            if let Some(pitch) = self.pitch.as_mut() {
                pitch.process(buf, self.channels);
            }
        }

        if self.settings.telephone {
            self.hp.process(buf);
            self.lp.process(buf);
        }

        if self.settings.robot_mix > 0.001 {
            let mix = self.settings.robot_mix;
            let step = 2.0 * std::f64::consts::PI * 55.0 / self.sample_rate as f64;
            for frame in buf.chunks_mut(self.channels) {
                let carrier = self.robot_phase.sin() as f32;
                self.robot_phase += step;
                if self.robot_phase > 2.0 * std::f64::consts::PI {
                    self.robot_phase -= 2.0 * std::f64::consts::PI;
                }
                for s in frame.iter_mut() {
                    *s = *s * (1.0 - mix) + *s * carrier * mix;
                }
            }
        }

        if self.settings.echo_mix > 0.001 && self.echo_frames > 0 {
            let mix = self.settings.echo_mix;
            let feedback = 0.35f32;
            let channels = self.channels;
            for frame in buf.chunks_mut(channels) {
                let idx = self.echo_index * channels;
                for ch in 0..channels {
                    let dry = frame[ch];
                    let delayed = self.echo[idx + ch];
                    self.echo[idx + ch] = dry + delayed * feedback;
                    frame[ch] = dry * (1.0 - mix) + delayed * mix;
                }
                self.echo_index += 1;
                if self.echo_index >= self.echo_frames {
                    self.echo_index = 0;
                }
            }
        }
    }
}

/// İki geçişli değişken gecikme ile perde kaydırma.
/// Gecikme yavaşça kayar; iki okuma ucu ters ağırlıklarla çapraz geçiş yapar.
struct PitchShifter {
    channels: usize,
    buffers: Vec<Vec<f32>>,
    size: usize,
    write: usize,
    phase: f64,
    step: f64,
}

impl PitchShifter {
    fn new(sample_rate: f32, channels: usize, semitones: i32) -> Self {
        let size = (sample_rate * 0.08) as usize; // 80 ms tampon
        let size = size.max(1024);
        let ratio = 2f64.powf(semitones as f64 / 12.0);
        Self {
            channels,
            buffers: vec![vec![0.0; size]; channels],
            size,
            write: 0,
            phase: 0.5,
            step: (1.0 - ratio) / (size as f64 * 0.5),
        }
    }

    fn process(&mut self, buf: &mut [f32], channels: usize) {
        if channels != self.channels || self.size == 0 {
            return;
        }
        let half = self.size / 2;
        for frame in buf.chunks_mut(channels) {
            for ch in 0..channels {
                self.buffers[ch][self.write] = frame[ch];
            }

            let p1 = self.phase.rem_euclid(1.0);
            let p2 = (p1 + 0.5f64).rem_euclid(1.0);
            let d1 = (p1 * half as f64) as usize;
            let d2 = (p2 * half as f64) as usize;
            let weight = (2.0 * p1 - 1.0).abs() as f32; // 0 → tap1, 1 → tap2

            for ch in 0..channels {
                let i1 = (self.write + self.size - d1) % self.size;
                let i2 = (self.write + self.size - d2) % self.size;
                let s1 = self.buffers[ch][i1];
                let s2 = self.buffers[ch][i2];
                frame[ch] = s1 * (1.0 - weight) + s2 * weight;
            }

            self.write = (self.write + 1) % self.size;
            self.phase += self.step;
        }
    }
}

/// RBJ biquad (high-pass / low-pass), kanal başına durum.
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: Vec<f32>,
    x2: Vec<f32>,
    y1: Vec<f32>,
    y2: Vec<f32>,
}

impl Biquad {
    pub fn high_pass(sample_rate: f32, frequency: f32, channels: usize) -> Self {
        Self::new(sample_rate, frequency, channels, true)
    }

    pub fn low_pass(sample_rate: f32, frequency: f32, channels: usize) -> Self {
        Self::new(sample_rate, frequency, channels, false)
    }

    fn new(sample_rate: f32, frequency: f32, channels: usize, high: bool) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let cos = w0.cos();
        let alpha = w0.sin() / (2.0 * 0.707);
        let (b0, b1, b2) = if high {
            ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0)
        } else {
            ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0)
        };
        let a0 = 1.0 + alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            x1: vec![0.0; channels],
            x2: vec![0.0; channels],
            y1: vec![0.0; channels],
            y2: vec![0.0; channels],
        }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        let channels = self.x1.len().max(1);
        for frame in buf.chunks_mut(channels) {
            for ch in 0..channels {
                let x = frame[ch];
                let y = self.b0 * x + self.b1 * self.x1[ch] + self.b2 * self.x2[ch]
                    - self.a1 * self.y1[ch]
                    - self.a2 * self.y2[ch];
                self.x2[ch] = self.x1[ch];
                self.x1[ch] = x;
                self.y2[ch] = self.y1[ch];
                self.y1[ch] = y;
                frame[ch] = y;
            }
        }
    }
}
