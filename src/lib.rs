use std::sync::{
    atomic::{AtomicU8, AtomicUsize, Ordering},
    Arc, Mutex,
};

use anyhow::Context;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    DeviceDescription, Stream,
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

// output channels the mono audio is played on, stored in `AudioHardware::output_channel`
const CHANNEL_MONO: u8 = 0;
const CHANNEL_LEFT: u8 = 1;
const CHANNEL_RIGHT: u8 = 2;

#[napi]
pub struct AudioHardware {
    host: cpal::Host,
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    input_callback: Option<Arc<ThreadsafeFunction<Float32Array>>>,
    output_producer: Option<<HeapRb<f32> as Split>::Prod>,
    output_to_discard: Arc<AtomicUsize>,
    output_channel: Arc<AtomicU8>,
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
            output_channel: Arc::new(AtomicU8::new(CHANNEL_MONO)),
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
                        name: device_name(&desc),
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

        // the stream has the channels of the device, so the mono audio can be played on the
        // left or right channel only.
        let channels = device
            .default_output_config()
            .map(|c| c.channels())
            .unwrap_or(NUM_CHANNELS)
            .max(NUM_CHANNELS);

        let cpal_config = cpal::StreamConfig {
            channels,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };

        let (output_producer, mut output_consumer) = HeapRb::<f32>::new(RING_SIZE).split();

        let samples_to_discard = self.output_to_discard.clone();
        let output_channel = self.output_channel.clone();
        let apm = self.apm.clone();
        let channels = channels as usize;
        let mut mono = vec![0.0f32; FRAME_SIZE];

        let stream = device
            .build_output_stream(
                &cpal_config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / channels;
                    mono.resize(frames, 0f32);

                    let discard = samples_to_discard.load(Ordering::Acquire);
                    if discard > 0 {
                        let skip = discard.min(output_consumer.occupied_len());
                        output_consumer.skip(skip);
                        samples_to_discard.fetch_sub(skip, Ordering::Release);
                    }
                    let len = output_consumer.pop_slice(&mut mono);
                    mono[len..].fill(0f32);

                    // mono plays on every channel (surround too), and so does a mono device
                    // whatever the selected channel. Left and right are the first two channels.
                    let only = match output_channel.load(Ordering::Relaxed) {
                        _ if channels == 1 => None,
                        CHANNEL_LEFT => Some(0),
                        CHANNEL_RIGHT => Some(1),
                        _ => None,
                    };
                    for (frame, &sample) in data.chunks_exact_mut(channels).zip(mono.iter()) {
                        match only {
                            Some(channel) => {
                                frame.fill(0f32);
                                frame[channel] = sample;
                            }
                            None => frame.fill(sample),
                        }
                    }

                    // Tell AEC what we are sending to the speakers.
                    if let Ok(mut apm) = apm.lock() {
                        apm.process_render_f32(&[&mono], &mut []).ok();
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

        // the stream has the channels of the device and is downmixed here: some multi-channel
        // devices deliver silence when Windows converts their input to mono.
        let channels = device
            .default_input_config()
            .map(|c| c.channels())
            .unwrap_or(NUM_CHANNELS)
            .max(NUM_CHANNELS);

        let cpal_config = cpal::StreamConfig {
            channels,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE as u32),
        };

        let callback = self.input_callback.clone();

        let apm = self.apm.clone();
        let channels = channels as usize;
        // mono samples not processed yet, the device does not always deliver whole frames
        let mut pending: Vec<f32> = Vec::with_capacity(FRAME_SIZE * 4);
        let mut processed = vec![0.0f32; FRAME_SIZE];

        let input_stream = device
            .build_input_stream(
                &cpal_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let Some(cb) = &callback else {
                        return;
                    };

                    // downmix to mono the way Windows does, the average of the channels
                    pending.extend(
                        data.chunks_exact(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32),
                    );

                    // the AEC works on frames of exactly FRAME_SIZE samples
                    while pending.len() >= FRAME_SIZE {
                        let frame = &pending[..FRAME_SIZE];

                        // in case AEC fails, we still have the unprocessed data in `processed`
                        processed.copy_from_slice(frame);

                        // Process capture (apply AEC to mic input).
                        if let Ok(mut apm) = apm.lock() {
                            apm.process_capture_f32(&[frame], &mut [&mut processed])
                                .ok();
                        }

                        cb.call(
                            Ok(Float32Array::from(processed.clone())),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                        pending.drain(..FRAME_SIZE);
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

    /// Sets the output channels the audio is played on: "mono" (all channels), "left" or
    /// "right". It applies to the current and future output streams, and is ignored by mono
    /// devices.
    #[napi]
    pub fn set_output_channel(&self, channel: String) -> anyhow::Result<()> {
        let value = match channel.as_str() {
            "mono" => CHANNEL_MONO,
            "left" => CHANNEL_LEFT,
            "right" => CHANNEL_RIGHT,
            _ => anyhow::bail!("unknown output channel: {channel}"),
        };
        self.output_channel.store(value, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn set_input_callback(&mut self, cb: Arc<ThreadsafeFunction<Float32Array>>) {
        self.input_callback = Some(cb);
    }
}

/// The name of a device as the OS shows it. On Windows that is the friendly name, e.g.
/// "Speakers (USB Audio Device)" rather than the "Speakers" of [`DeviceDescription::name`];
/// cpal puts it in the extended lines when it differs from the name. The launcher names the
/// devices the same way (`apps/launcher/src-tauri/src/audio_devices.rs` in convis).
fn device_name(desc: &DeviceDescription) -> String {
    #[cfg(windows)]
    if let Some(friendly_name) = desc.extended().first() {
        return friendly_name.clone();
    }
    desc.name().to_string()
}
