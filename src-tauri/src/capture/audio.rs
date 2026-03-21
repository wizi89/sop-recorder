use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// A Send-safe handle to stop audio recording.
pub struct AudioHandle {
    stop_tx: Option<mpsc::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<Result<(), String>>>,
    output_path: PathBuf,
}

unsafe impl Send for AudioHandle {}

impl AudioHandle {
    /// Start recording audio to a WAV file at the given path.
    pub fn start(output_path: &Path) -> Result<Self, String> {
        let output_path = output_path.to_path_buf();
        let path_clone = output_path.clone();

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let join_handle = std::thread::spawn(move || {
            Self::run_recording(&path_clone, stop_rx, ready_tx)
        });

        ready_rx
            .recv()
            .map_err(|_| "Audio thread died before starting".to_string())?
            .map_err(|e| format!("Audio init failed: {}", e))?;

        Ok(Self {
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
            output_path,
        })
    }

    fn run_recording(
        output_path: &Path,
        stop_rx: mpsc::Receiver<()>,
        ready_tx: mpsc::Sender<Result<(), String>>,
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

        // WAV output: 16kHz mono 16-bit (what Azure Speech SDK expects)
        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = WavWriter::create(output_path, spec)
            .map_err(|e| format!("Failed to create WAV file: {}", e))?;
        let writer = Arc::new(Mutex::new(Some(writer)));
        let writer_clone = writer.clone();

        let err_fn = |err: cpal::StreamError| {
            log::error!("Audio stream error: {}", err);
        };

        // Resampling state shared with the callback via Arc<Mutex<>>
        let resample_state = Arc::new(Mutex::new(ResampleState::new(device_rate, TARGET_SAMPLE_RATE)));
        let resample_clone = resample_state.clone();

        let stream = match sample_format {
            SampleFormat::F32 => {
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

                            // Resample and write
                            if let (Ok(mut rs), Ok(mut guard)) =
                                (resample_clone.lock(), writer_clone.lock())
                            {
                                if let Some(ref mut w) = *guard {
                                    for sample in rs.process(&mono) {
                                        let pcm = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                        let _ = w.write_sample(pcm);
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

                            if let (Ok(mut rs), Ok(mut guard)) =
                                (resample_clone.lock(), writer_clone.lock())
                            {
                                if let Some(ref mut w) = *guard {
                                    for sample in rs.process(&mono) {
                                        let pcm = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                        let _ = w.write_sample(pcm);
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

        // Finalize WAV
        if let Ok(mut guard) = writer.lock() {
            if let Some(w) = guard.take() {
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

/// Simple linear resampler that converts from one sample rate to another.
/// Maintains fractional position across calls for seamless resampling.
struct ResampleState {
    ratio: f64,         // input_rate / output_rate
    fractional: f64,    // accumulated fractional position
    last_sample: f32,   // previous sample for interpolation
}

impl ResampleState {
    fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            ratio: input_rate as f64 / output_rate as f64,
            fractional: 0.0,
            last_sample: 0.0,
        }
    }

    /// Process a chunk of mono input samples, return resampled output samples.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }

        let mut output = Vec::new();

        while (self.fractional as usize) < input.len() {
            let idx = self.fractional as usize;
            let frac = (self.fractional - idx as f64) as f32;

            // Linear interpolation between adjacent samples
            let s0 = if idx == 0 { self.last_sample } else { input[idx - 1] };
            let s1 = if idx < input.len() { input[idx] } else { *input.last().unwrap() };
            let interpolated = s0 + (s1 - s0) * frac;

            output.push(interpolated);
            self.fractional += self.ratio;
        }

        // Keep fractional position relative to next buffer
        self.fractional -= input.len() as f64;

        // Remember last sample for interpolation across buffers
        self.last_sample = *input.last().unwrap_or(&0.0);

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_to_16k_reduces_sample_count() {
        let mut rs = ResampleState::new(48000, 16000);
        // 480 input samples at 48kHz = 10ms -> should produce ~160 at 16kHz
        let input: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0).sin()).collect();
        let output = rs.process(&input);

        // Should produce roughly 160 samples (ratio 3:1)
        assert!(output.len() >= 158 && output.len() <= 162,
            "Expected ~160 samples, got {}", output.len());
    }

    #[test]
    fn resample_preserves_silence() {
        let mut rs = ResampleState::new(48000, 16000);
        let input = vec![0.0_f32; 480];
        let output = rs.process(&input);

        for s in &output {
            assert!(*s == 0.0, "Expected silence, got {}", s);
        }
    }

    #[test]
    fn resample_empty_input() {
        let mut rs = ResampleState::new(48000, 16000);
        let output = rs.process(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn resample_same_rate_preserves_count() {
        let mut rs = ResampleState::new(16000, 16000);
        let input: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let output = rs.process(&input);

        // Same rate should produce same number of samples
        assert_eq!(output.len(), input.len());
        // After first sample (which interpolates from 0), values should track closely
        for (a, b) in input[1..].iter().zip(output[1..].iter()) {
            assert!((a - b).abs() < 0.02, "Mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn resample_across_multiple_chunks_is_seamless() {
        let mut rs = ResampleState::new(48000, 16000);
        // Process two chunks separately
        let chunk1: Vec<f32> = (0..480).map(|i| (i as f32 * 0.01).sin()).collect();
        let chunk2: Vec<f32> = (480..960).map(|i| (i as f32 * 0.01).sin()).collect();
        let out1 = rs.process(&chunk1);
        let out2 = rs.process(&chunk2);

        // Process both at once
        let mut rs2 = ResampleState::new(48000, 16000);
        let combined: Vec<f32> = (0..960).map(|i| (i as f32 * 0.01).sin()).collect();
        let out_combined = rs2.process(&combined);

        // Total sample count should match
        let total_separate = out1.len() + out2.len();
        assert!((total_separate as i32 - out_combined.len() as i32).abs() <= 1,
            "Chunk processing produced {} vs combined {}", total_separate, out_combined.len());
    }

    #[test]
    fn resample_output_stays_in_range() {
        let mut rs = ResampleState::new(48000, 16000);
        // Feed max/min values
        let input: Vec<f32> = (0..480).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let output = rs.process(&input);

        for s in &output {
            assert!(*s >= -1.0 && *s <= 1.0, "Out of range: {}", s);
        }
    }
}
