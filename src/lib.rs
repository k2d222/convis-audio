use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use anyhow::Context;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream,
};
use napi::{
    bindgen_prelude::Float32Array,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use sonora::AudioProcessing;

const SAMPLE_RATE: u32 = 24_000;
const NUM_CHANNELS: u16 = 1;
const FRAME_SIZE: usize = (SAMPLE_RATE / 100) as usize; // 10 ms
const RING_SIZE: usize = FRAME_SIZE * 10000; // number of frames to store in the buffer. Corresponds to 100s of audio.

#[napi]
pub struct AudioHardware {
    host: cpal::Host,
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    input_callback: Option<Arc<ThreadsafeFunction<Float32Array>>>,
    output_producer: Option<<HeapRb<f32> as Split>::Prod>,
    output_to_discard: Arc<AtomicUsize>,
    apm: Arc<Mutex<AudioProcessing>>,
}

#[napi(object)]
pub struct StreamConfig {
    pub device_id: String,
}

#[napi(object)]
pub struct DeviceProperties {
    pub input: bool,
    pub output: bool,
    pub name: String,
    pub id: String,
}

#[napi]
impl AudioHardware {
    #[napi(constructor)]
    pub fn new() -> Self {
        let apm_config = sonora::Config {
            echo_canceller: Some(sonora::config::EchoCanceller::default()),
            noise_suppression: Some(sonora::config::NoiseSuppression::default()),
            gain_controller2: Some(sonora::config::GainController2::default()),
            ..Default::default()
        };

        let stream_config = sonora::StreamConfig::new(SAMPLE_RATE, NUM_CHANNELS);

        let apm = AudioProcessing::builder()
            .config(apm_config)
            .capture_config(stream_config)
            .render_config(stream_config)
            .build();

        Self {
            host: cpal::default_host(),
            input_stream: None,
            output_stream: None,
            input_callback: None,
            output_producer: None,
            output_to_discard: Arc::new(AtomicUsize::new(0)),
            apm: Arc::new(Mutex::new(apm)),
        }
    }

    #[napi]
    pub fn api_name(&self) -> String {
        self.host.id().to_string()
    }

    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        SAMPLE_RATE as u32
    }

    #[napi]
    pub fn clear_output_queue(&mut self) {
        if let Some(producer) = &self.output_producer {
            self.output_to_discard
                .store(producer.occupied_len(), Ordering::Release);
        }
    }

    #[napi]
    pub fn default_input(&self) -> Option<String> {
        self.host
            .default_input_device()
            .and_then(|device| device.id().ok())
            .map(|id| id.to_string())
    }

    #[napi]
    pub fn default_output(&self) -> Option<String> {
        self.host
            .default_output_device()
            .and_then(|device| device.id().ok())
            .map(|id| id.to_string())
    }

    #[napi]
    pub fn list_devices(&self) -> anyhow::Result<Vec<DeviceProperties>> {
        let props = match self.host.devices() {
            Ok(devices) => devices
                .filter_map(|device| {
                    let id = device.id().ok()?;
                    let desc = device.description().ok()?;
                    Some(DeviceProperties {
                        name: desc.name().to_string(),
                        id: id.to_string(),
                        input: desc.supports_input(),
                        output: desc.supports_output(),
                    })
                })
                .collect(),
            Err(e) => {
                eprintln!("failed to list audio devices: {e}");
                vec![]
            }
        };

        Ok(props)
    }

    #[napi]
    pub fn open_output_stream(&mut self, cfg: StreamConfig) -> anyhow::Result<()> {
        self.output_stream.take(); // close the stream
        self.output_producer.take();

        let id = cfg.device_id.parse()?;
        let device = self.host.device_by_id(&id).context("device unavailable")?;

        let cpal_config = cpal::StreamConfig {
            channels: NUM_CHANNELS,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };

        let (output_producer, mut output_consumer) = HeapRb::<f32>::new(RING_SIZE).split();

        let samples_to_discard = self.output_to_discard.clone();
        let apm = self.apm.clone();

        let stream = device
            .build_output_stream(
                &cpal_config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let discard = samples_to_discard.load(Ordering::Acquire);
                    if discard > 0 {
                        let skip = discard.min(output_consumer.occupied_len());
                        output_consumer.skip(skip);
                        samples_to_discard.fetch_sub(skip, Ordering::Release);
                    }
                    let len = output_consumer.pop_slice(data);
                    data[len..].fill(0f32);

                    // Tell AEC what we are sending to the speakers.
                    if let Ok(mut apm) = apm.lock() {
                        apm.process_render_f32(&[&data], &mut []).ok();
                    }
                },
                |err| eprintln!("output stream error: {err}"),
                None,
            )
            .context("failed to build output stream")?;

        stream.play().context("failed to play output stream")?;

        self.output_producer = Some(output_producer);
        self.output_stream = Some(stream);

        Ok(())
    }

    #[napi]
    pub fn open_input_stream(&mut self, cfg: StreamConfig) -> anyhow::Result<()> {
        self.input_stream.take(); // close the stream

        let id = cfg.device_id.parse()?;
        let device = self.host.device_by_id(&id).context("device unavailable")?;

        let cpal_config = cpal::StreamConfig {
            channels: NUM_CHANNELS,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };

        let callback = self.input_callback.clone();

        let apm = self.apm.clone();
        let mut processed = vec![0.0f32; FRAME_SIZE];

        let input_stream = device
            .build_input_stream(
                &cpal_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Some(cb) = &callback {
                        if processed.len() != data.len() {
                            eprintln!("unexpected: processed.len() != data.len()");
                            return;
                        }

                        // in case AEC fails, we still have the unprocessed data in `processed`
                        processed.copy_from_slice(data);

                        // Process capture (apply AEC to mic input).
                        if let Ok(mut apm) = apm.lock() {
                            apm.process_capture_f32(&[data], &mut [&mut processed]).ok();
                        }

                        cb.call(
                            Ok(Float32Array::from(processed.clone())),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                    }
                },
                |err| eprintln!("input stream error: {err}"),
                None,
            )
            .context("failed to build input stream")?;

        input_stream.play().context("failed to play input stream")?;
        self.input_stream = Some(input_stream);

        Ok(())
    }

    #[napi]
    pub fn write(&mut self, pcm: &[f32]) {
        if let Some(output_producer) = &mut self.output_producer {
            let len = output_producer.push_slice(pcm);
            if len < pcm.len() {
                eprintln!("audio overrun! pushed {len} bytes out of {}, buffer capacity: {}, buffer vacant: {}", pcm.len(), output_producer.capacity(), output_producer.vacant_len());
            }
        }
    }

    #[napi]
    pub fn set_input_callback(&mut self, cb: Arc<ThreadsafeFunction<Float32Array>>) {
        self.input_callback = Some(cb);
    }
}
