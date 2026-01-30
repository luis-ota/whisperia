use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct AudioRecorder {
    host: cpal::Host,
    device: cpal::Device,
    config: StreamConfig,
    sample_format: SampleFormat,
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no input device available")?;

        let config = device.default_input_config()?;
        let sample_format = config.sample_format();
        let config: StreamConfig = config.config();

        info!("audio device: {:?}", device.name()?);
        info!(
            "sample format: {:?}, sample rate: {}",
            sample_format, config.sample_rate.0
        );

        Ok(Self {
            host,
            device,
            config,
            sample_format,
        })
    }

    pub fn record_for_seconds(&self, seconds: u64) -> Result<Vec<f32>> {
        info!("recording for {} seconds...", seconds);

        let samples_needed = (self.config.sample_rate.0 as u64 * seconds) as usize;
        let recorded_samples = Arc::new(Mutex::new(Vec::with_capacity(samples_needed)));
        let samples_clone = recorded_samples.clone();

        let err_fn = move |err| {
            eprintln!("audio stream error: {}", err);
        };

        let stream = match self.sample_format {
            SampleFormat::F32 => {
                let samples = samples_clone.clone();
                self.device.build_input_stream(
                    &self.config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut vec = samples.lock().unwrap();
                        for &sample in data {
                            if vec.len() < samples_needed {
                                vec.push(sample);
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let samples = samples_clone.clone();
                self.device.build_input_stream(
                    &self.config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut vec = samples.lock().unwrap();
                        for &sample in data {
                            if vec.len() < samples_needed {
                                vec.push(sample as f32 / 32768.0);
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => anyhow::bail!("unsupported sample format"),
        };

        stream.play()?;

        // wait for recording
        std::thread::sleep(std::time::Duration::from_secs(seconds));

        drop(stream);

        let samples = recorded_samples.lock().unwrap().clone();
        info!("recorded {} samples", samples.len());

        // resample to 16khz if needed
        if self.config.sample_rate.0 != 16000 {
            let resampled = Self::resample(&samples, self.config.sample_rate.0, 16000);
            Ok(resampled)
        } else {
            Ok(samples)
        }
    }

    fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return input.to_vec();
        }

        let ratio = to_rate as f64 / from_rate as f64;
        let output_len = (input.len() as f64 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 / ratio;
            let src_idx_floor = src_idx.floor() as usize;
            let src_idx_ceil = (src_idx_floor + 1).min(input.len() - 1);
            let t = src_idx - src_idx_floor as f64;

            let sample = input[src_idx_floor] * (1.0 - t as f32) + input[src_idx_ceil] * t as f32;
            output.push(sample);
        }

        output
    }
}
