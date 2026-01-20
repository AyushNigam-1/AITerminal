use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct AudioRecorder {
    stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioRecorder {
    pub fn start() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();

        // 1. Get Default Input Device
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device found"))?;

        // 2. Configure Stream (Standard mono, 16kHz is usually best for Whisper)
        let config = device.default_input_config()?;
        let stream_config: cpal::StreamConfig = config.clone().into();

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = buffer.clone();

        // 3. Build the Input Stream
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| {
                // Low-latency callback: simply push data to our buffer
                if let Ok(mut b) = buffer_clone.lock() {
                    b.extend_from_slice(data);
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None, // None = blocking, but cpal handles this internal timeout usually
        )?;

        stream.play()?;

        Ok(Self { stream, buffer })
    }

    pub fn stop_and_save(self, file_path: &str) -> Result<(), anyhow::Error> {
        // Stop stream logic happens when 'stream' is dropped
        drop(self.stream);

        let data = self.buffer.lock().unwrap();

        // 4. Save to WAV (Required for API)
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100, // Or whatever the device default was
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(file_path, spec)?;

        // Convert f32 samples (-1.0 to 1.0) to i16 for WAV
        for &sample in data.iter() {
            let amplitude = i16::MAX as f32;
            writer.write_sample((sample * amplitude) as i16)?;
        }

        writer.finalize()?;
        Ok(())
    }
}
