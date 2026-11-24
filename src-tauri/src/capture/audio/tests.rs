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
