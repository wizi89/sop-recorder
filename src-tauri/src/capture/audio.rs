use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// A Send-safe handle to stop audio recording.
///
/// `audio_level` stores the latest peak magnitude [0..1] as `f32::to_bits`
/// so a polling task can emit a VU meter event without locking the audio
/// callback. Readers should use `f32::from_bits(atomic.load(Relaxed))`.
pub struct AudioHandle {
    stop_tx: Option<mpsc::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<Result<(), String>>>,
    output_path: PathBuf,
    pub audio_level: Arc<AtomicU32>,
}

unsafe impl Send for AudioHandle {}

impl AudioHandle {
    /// Start recording audio to a WAV file at the given path.
    pub fn start(output_path: &Path) -> Result<Self, String> {
        let output_path = output_path.to_path_buf();
        let path_clone = output_path.clone();

        let audio_level = Arc::new(AtomicU32::new(0));
        let level_clone = audio_level.clone();

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let join_handle = std::thread::spawn(move || {
            let result = Self::run_recording(&path_clone, stop_rx, ready_tx.clone(), level_clone);
            // Most setup steps return early with `?`. Without this their reason
            // never reached `start`, and the log said only that the thread died.
            // After a successful start nobody listens any more, and the send
            // is dropped.
            if let Err(e) = &result {
                let _ = ready_tx.send(Err(e.clone()));
            }
            result
        });

        ready_rx
            .recv()
            .map_err(|_| "Audio thread died before starting".to_string())?
            .map_err(|e| format!("Audio init failed: {}", e))?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
            output_path,
            audio_level,
        })
    }

    fn run_recording(
        output_path: &Path,
        stop_rx: mpsc::Receiver<()>,
        ready_tx: mpsc::Sender<Result<(), String>>,
        audio_level: Arc<AtomicU32>,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input audio device found")?;

        log::info!("Audio device: {:?}", device.name());

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default audio config: {}", e))?;

        let device_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let sample_format = config.sample_format();

        log::info!(
            "Audio config: {}Hz, {} ch, {:?} -> resampling to {}Hz mono",
            device_rate,
            channels,
            sample_format,
            TARGET_SAMPLE_RATE
        );

        let writer = WavWriter::create(output_path, wav_spec())
            .map_err(|e| format!("Failed to create WAV file: {}", e))?;
        let writer = Arc::new(Mutex::new(Some(writer)));
        let writer_clone = writer.clone();

        let err_fn = |err: cpal::StreamError| {
            log::error!("Audio stream error: {}", err);
        };

        // Resampling state shared with the callback via Arc<Mutex<>>
        let resample_state = match ResampleState::new(device_rate, TARGET_SAMPLE_RATE) {
            Ok(rs) => Arc::new(Mutex::new(rs)),
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };
        let resample_clone = resample_state.clone();

        // Per-callback helper: compute peak abs level over a mono slice and
        // store it into the shared atomic as f32 bits for a polling task to
        // read at ~10 Hz. Using `store` rather than `fetch_max` means the
        // atomic always reflects the LATEST chunk's peak -- the poller drains
        // it and the next chunk overwrites, so silence is visible immediately.
        let write_peak = |samples: &[f32], level: &Arc<AtomicU32>| {
            let peak = samples
                .iter()
                .copied()
                .fold(0.0_f32, |acc, s| acc.max(s.abs()))
                .clamp(0.0, 1.0);
            level.store(peak.to_bits(), Ordering::Relaxed);
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let level_cb = audio_level.clone();
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            // Mix to mono
                            let mono: Vec<f32> = data
                                .chunks(channels)
                                .map(|frame| {
                                    let sum: f32 = frame.iter().sum();
                                    sum / channels as f32
                                })
                                .collect();

                            write_peak(&mono, &level_cb);

                            // Resample and write
                            if let (Ok(mut rs), Ok(mut guard)) =
                                (resample_clone.lock(), writer_clone.lock())
                            {
                                if let Some(ref mut w) = *guard {
                                    for sample in rs.process(&mono) {
                                        let _ = w.write_sample(to_pcm(sample));
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?
            }
            SampleFormat::I16 => {
                let level_cb = audio_level.clone();
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let mono: Vec<f32> = data
                                .chunks(channels)
                                .map(|frame| {
                                    let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                                    (sum as f32) / (channels as f32 * 32768.0)
                                })
                                .collect();

                            write_peak(&mono, &level_cb);

                            if let (Ok(mut rs), Ok(mut guard)) =
                                (resample_clone.lock(), writer_clone.lock())
                            {
                                if let Some(ref mut w) = *guard {
                                    for sample in rs.process(&mono) {
                                        let _ = w.write_sample(to_pcm(sample));
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?
            }
            _ => {
                let _ = ready_tx.send(Err(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )));
                return Err(format!("Unsupported sample format: {:?}", sample_format));
            }
        };

        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        log::info!("Audio recording started: {}", output_path.display());
        let _ = ready_tx.send(Ok(()));

        // Block until stop signal
        let _ = stop_rx.recv();
        drop(stream);

        // Flush the resampler's tail, then finalize the WAV
        if let Ok(mut guard) = writer.lock() {
            if let Some(mut w) = guard.take() {
                if let Ok(mut rs) = resample_state.lock() {
                    for sample in rs.finish() {
                        let _ = w.write_sample(to_pcm(sample));
                    }
                }
                w.finalize()
                    .map_err(|e| format!("Failed to finalize WAV: {}", e))?;
            }
        }

        log::info!("Audio recording stopped: {}", output_path.display());
        Ok(())
    }

    /// Stop recording, finalize WAV, return the path.
    pub fn stop(mut self) -> Result<PathBuf, String> {
        drop(self.stop_tx.take());

        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .map_err(|_| "Audio thread panicked".to_string())??;
        }

        Ok(self.output_path.clone())
    }
}

fn to_pcm(sample: f32) -> i16 {
    (sample * 32767.0).clamp(-32768.0, 32767.0) as i16
}

/// Band-limited resampler from the device rate to 16 kHz.
///
/// Linear interpolation (what this used to be) has no anti-aliasing filter: at
/// 48 -> 16 kHz everything between 8 and 24 kHz -- sibilants, fan and keyboard
/// noise -- folds back into the speech band the recognizer listens to. rubato's
/// FFT resampler low-passes below the new Nyquist frequency first.
///
/// The output stays aligned with the input: the filter's start-up delay is
/// dropped and `finish` flushes the tail, so the WAV holds exactly
/// `frames_in * out / in` samples and narration timestamps still line up with
/// the steps captured at the same moment.
struct ResampleState {
    /// `None` when the device already runs at the target rate.
    fft: Option<Fft<f32>>,
    input_rate: u64,
    output_rate: u64,
    /// Input not yet a whole resampler chunk.
    pending: Vec<f32>,
    out_buf: Vec<f32>,
    /// Output frames of filter delay still to drop.
    skip: usize,
    frames_in: u64,
    frames_out: u64,
}

impl ResampleState {
    fn new(input_rate: u32, output_rate: u32) -> Result<Self, String> {
        let fft = if input_rate == output_rate {
            None
        } else {
            // ~20 ms chunks: short enough that the WAV grows while recording,
            // long enough for a sharp filter.
            let chunk = (input_rate / 50).max(1) as usize;
            Some(
                Fft::<f32>::new(
                    input_rate as usize,
                    output_rate as usize,
                    chunk,
                    1,
                    FixedSync::Both,
                )
                .map_err(|e| format!("Failed to create resampler: {}", e))?,
            )
        };
        let (skip, out_len) = fft
            .as_ref()
            .map_or((0, 0), |f| (f.output_delay(), f.output_frames_max()));
        Ok(Self {
            fft,
            input_rate: input_rate as u64,
            output_rate: output_rate as u64,
            pending: Vec::new(),
            out_buf: vec![0.0; out_len],
            skip,
            frames_in: 0,
            frames_out: 0,
        })
    }

    /// Process a chunk of mono input samples, return resampled output samples.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        self.frames_in += input.len() as u64;
        let mut output = Vec::new();
        if self.fft.is_none() {
            self.frames_out += input.len() as u64;
            output.extend_from_slice(input);
            return output;
        }
        self.pending.extend_from_slice(input);
        while let Some(need) = self.fft.as_ref().map(|f| f.input_frames_next()) {
            if self.pending.len() < need || !self.run_chunk(need, &mut output) {
                break;
            }
            self.pending.drain(..need);
        }
        output
    }

    /// Flush what is still buffered, at the end of a recording. Returns the
    /// remaining output, trimmed so the total matches the input duration.
    pub fn finish(&mut self) -> Vec<f32> {
        let mut output = Vec::new();
        let expected = (self.frames_in * self.output_rate + self.input_rate / 2) / self.input_rate;
        while let Some(need) = self.fft.as_ref().map(|f| f.input_frames_next()) {
            if self.frames_out >= expected {
                break;
            }
            let take = self.pending.len().min(need);
            if !self.run_chunk(take, &mut output) {
                break;
            }
            self.pending.drain(..take);
        }
        let excess = self.frames_out.saturating_sub(expected) as usize;
        output.truncate(output.len().saturating_sub(excess));
        self.frames_out -= excess as u64;
        output
    }

    /// Resample the first `take` pending frames (zero-padded to a full chunk).
    /// False if rubato refused, which with these fixed sizes means a bug: the
    /// recording keeps going and loses the rest of the audio rather than the app.
    fn run_chunk(&mut self, take: usize, output: &mut Vec<f32>) -> bool {
        let Some(fft) = self.fft.as_mut() else {
            return false;
        };
        let out_len = self.out_buf.len();
        let result = InterleavedSlice::new(&self.pending[..take], 1, take)
            .map_err(|e| e.to_string())
            .and_then(|input| {
                let mut out = InterleavedSlice::new_mut(&mut self.out_buf[..], 1, out_len)
                    .map_err(|e| e.to_string())?;
                fft.process_into_buffer(&input, &mut out, Some(&Indexing::new().partial_len(take)))
                    .map_err(|e| e.to_string())
            });
        let written = match result {
            Ok((_, written)) => written,
            Err(e) => {
                log::error!("Resampling failed: {}", e);
                return false;
            }
        };
        let skipped = self.skip.min(written);
        self.skip -= skipped;
        output.extend_from_slice(&self.out_buf[skipped..written]);
        self.frames_out += (written - skipped) as u64;
        true
    }
}

/// 16 kHz mono 16-bit: what Azure Speech expects.
fn wav_spec() -> WavSpec {
    WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

/// One second of silence in the recording format, for a recording made
/// without a working microphone.
pub fn write_silent_wav(path: &Path) -> Result<(), String> {
    let mut writer = WavWriter::create(path, wav_spec())
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;
    for _ in 0..TARGET_SAMPLE_RATE {
        writer.write_sample(0i16).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE_RATES: [u32; 3] = [44_100, 48_000, 96_000];

    /// Against the real default microphone: `cargo test --lib live_microphone -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_microphone() {
        let path = std::env::temp_dir().join("live-mic.wav");
        let handle = AudioHandle::start(&path).expect("audio start");
        std::thread::sleep(std::time::Duration::from_secs(2));
        let saved = handle.stop().expect("audio stop");
        let frames = hound::WavReader::open(&saved).unwrap().duration();
        println!("recorded {frames} frames to {}", saved.display());
        assert!(frames > 16_000);
    }

    #[test]
    fn the_silent_stand_in_is_a_valid_recording_of_one_second_of_silence() {
        let path = std::env::temp_dir().join(format!("silent-{}.wav", std::process::id()));
        write_silent_wav(&path).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec(), wav_spec());
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        let _ = std::fs::remove_file(&path);
        assert_eq!(samples.len(), TARGET_SAMPLE_RATE as usize);
        assert!(samples.iter().all(|&s| s == 0));
    }

    fn sine(freq: f32, rate: u32, seconds: f32) -> Vec<f32> {
        let n = (rate as f32 * seconds) as usize;
        // Phase in f64: in f32 it loses precision over a second of samples,
        // and that rounding noise alone measures around -60 dB.
        (0..n)
            .map(|i| {
                let phase = 2.0 * std::f64::consts::PI * freq as f64 * i as f64 / rate as f64;
                (0.5 * phase.sin()) as f32
            })
            .collect()
    }

    /// Amplitude of the `freq` component, over the middle half of the signal
    /// (away from start-up and flush edges).
    fn level_at(samples: &[f32], freq: f32, rate: u32) -> f32 {
        let mid = &samples[samples.len() / 4..samples.len() * 3 / 4];
        let (mut re, mut im) = (0.0_f64, 0.0_f64);
        for (i, &x) in mid.iter().enumerate() {
            let phase = 2.0 * std::f64::consts::PI * freq as f64 * i as f64 / rate as f64;
            re += x as f64 * phase.cos();
            im += x as f64 * phase.sin();
        }
        (2.0 * (re * re + im * im).sqrt() / mid.len() as f64) as f32
    }

    fn rms(samples: &[f32]) -> f32 {
        let mid = &samples[samples.len() / 4..samples.len() * 3 / 4];
        (mid.iter().map(|x| x * x).sum::<f32>() / mid.len() as f32).sqrt()
    }

    /// Feed `input` the way cpal does -- in callback-sized pieces -- then flush.
    fn resample(rate: u32, input: &[f32], callback_frames: usize) -> Vec<f32> {
        let mut rs = ResampleState::new(rate, TARGET_SAMPLE_RATE).unwrap();
        let mut out: Vec<f32> = input
            .chunks(callback_frames)
            .flat_map(|c| rs.process(c))
            .collect();
        out.extend(rs.finish());
        out
    }

    #[test]
    fn speech_band_tones_keep_their_level() {
        for rate in DEVICE_RATES {
            for freq in [300.0, 1_000.0, 3_400.0, 6_000.0] {
                let out = resample(rate, &sine(freq, rate, 1.0), 480);
                let level = level_at(&out, freq, TARGET_SAMPLE_RATE);
                assert!(
                    (level - 0.5).abs() < 0.01,
                    "{freq} Hz from {rate} Hz: amplitude {level}, expected 0.5"
                );
            }
        }
    }

    #[test]
    fn tones_above_8khz_do_not_fold_back_into_the_speech_band() {
        // A 12 kHz tone at 48 kHz lands on 4 kHz after naive decimation. The
        // linear interpolator this replaced left it there at full strength
        // (0 dB at 48 kHz, -1 to -3 dB at 44.1 kHz); rubato measures below
        // -140 dB.
        for rate in DEVICE_RATES {
            for freq in [9_000.0, 12_000.0, 15_000.0] {
                let out = resample(rate, &sine(freq, rate, 1.0), 480);
                let residue = rms(&out) / (0.5 / 2f32.sqrt());
                assert!(
                    residue < 1e-5,
                    "{freq} Hz from {rate} Hz left {:.1} dB in the output",
                    20.0 * residue.log10()
                );
            }
        }
    }

    #[test]
    fn output_length_matches_the_recording_duration() {
        for rate in DEVICE_RATES {
            for frames in [0, 1, 999, rate as usize * 7 / 3] {
                for callback in [1, 441, 480, 1_024, 4_096] {
                    let input = vec![0.1_f32; frames];
                    let out = resample(rate, &input, callback);
                    let expected =
                        (frames as u64 * TARGET_SAMPLE_RATE as u64 + rate as u64 / 2) / rate as u64;
                    assert_eq!(
                        out.len() as u64,
                        expected,
                        "{frames} frames at {rate} Hz in {callback}-frame callbacks"
                    );
                }
            }
        }
    }

    #[test]
    fn output_is_not_delayed() {
        // Narration timestamps are matched to steps by sample position, so a
        // click 250 ms in must stay 250 ms in.
        for rate in DEVICE_RATES {
            let mut input = vec![0.0_f32; rate as usize];
            input[rate as usize / 4] = 1.0;
            let out = resample(rate, &input, 480);
            let peak = out
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
                .unwrap()
                .0;
            let expected = TARGET_SAMPLE_RATE as usize / 4;
            assert!(
                peak.abs_diff(expected) <= 1,
                "click from {rate} Hz landed at {peak}, expected {expected}"
            );
        }
    }

    #[test]
    fn callback_size_does_not_change_the_output() {
        for rate in DEVICE_RATES {
            let input = sine(440.0, rate, 0.5);
            let whole = resample(rate, &input, input.len());
            for callback in [1, 37, 480, 1_000] {
                assert_eq!(
                    resample(rate, &input, callback),
                    whole,
                    "{callback}-frame callbacks at {rate} Hz"
                );
            }
        }
    }

    #[test]
    fn silence_stays_silent() {
        let out = resample(48_000, &vec![0.0; 48_000], 480);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn empty_input_produces_nothing() {
        let mut rs = ResampleState::new(48_000, TARGET_SAMPLE_RATE).unwrap();
        assert!(rs.process(&[]).is_empty());
        assert!(rs.finish().is_empty());
    }

    #[test]
    fn device_already_at_16khz_passes_through_untouched() {
        let input: Vec<f32> = (0..1_000).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(resample(TARGET_SAMPLE_RATE, &input, 160), input);
    }

    #[test]
    fn pcm_conversion_clamps_overshoot() {
        // The anti-aliasing filter rings slightly past full scale on clipped
        // input; that must clip, not wrap around.
        assert_eq!(to_pcm(1.2), i16::MAX);
        assert_eq!(to_pcm(-1.2), i16::MIN);
        assert_eq!(to_pcm(0.0), 0);
        assert_eq!(to_pcm(0.5), 16_383);
    }
}
