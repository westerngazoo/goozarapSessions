//! [`CpalBackend`] — the real device backend, via cpal.
//!
//! Behaviour on a real device is verified by ear through the `record_playback`
//! demo, not in CI (CI has no sound card). Doc examples that open a device are
//! marked `no_run`.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::backend::{AudioBackend, AudioStream};
use crate::error::AudioError;

/// A backend backed by the default system input and output devices.
///
/// Reports the **input** device's sample rate and channel count (the format a
/// recorded [`crate::Take`] is stamped with); the output device's channel count
/// is available via [`CpalBackend::output_channels`] for playback adaptation.
/// v0 requires an `f32` stream and does not resample.
///
/// ```no_run
/// use gooz_audio::{AudioBackend, CpalBackend};
///
/// let backend = CpalBackend::with_defaults().expect("default devices");
/// println!("recording at {} Hz", backend.sample_rate());
/// ```
pub struct CpalBackend {
    input_device: cpal::Device,
    output_device: cpal::Device,
    input_config: cpal::StreamConfig,
    output_config: cpal::StreamConfig,
    input_channels: u16,
    input_sample_rate: u32,
    output_channels: u16,
}

impl CpalBackend {
    /// Opens the default host's default input and output devices, requiring an
    /// `f32` configuration on each.
    ///
    /// Errors: [`AudioError::NoInputDevice`] / [`AudioError::NoOutputDevice`] if
    /// a default device is absent; [`AudioError::UnsupportedConfig`] if the
    /// default config cannot be queried or is not `f32`.
    ///
    /// ```no_run
    /// # use gooz_audio::CpalBackend;
    /// let backend = CpalBackend::with_defaults().unwrap();
    /// ```
    pub fn with_defaults() -> Result<CpalBackend, AudioError> {
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;
        let output_device = host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;

        let input_supported = input_device
            .default_input_config()
            .map_err(|_| AudioError::UnsupportedConfig)?;
        let output_supported = output_device
            .default_output_config()
            .map_err(|_| AudioError::UnsupportedConfig)?;
        if input_supported.sample_format() != cpal::SampleFormat::F32
            || output_supported.sample_format() != cpal::SampleFormat::F32
        {
            return Err(AudioError::UnsupportedConfig);
        }

        Ok(CpalBackend {
            input_channels: input_supported.channels(),
            input_sample_rate: input_supported.sample_rate().0,
            output_channels: output_supported.channels(),
            input_config: input_supported.config(),
            output_config: output_supported.config(),
            input_device,
            output_device,
        })
    }

    /// The output device's channel count, for adapting a take before playback.
    /// (The input sample rate and channels are the [`AudioBackend`] methods.)
    pub fn output_channels(&self) -> u16 {
        self.output_channels
    }
}

impl AudioBackend for CpalBackend {
    fn sample_rate(&self) -> u32 {
        self.input_sample_rate
    }

    fn channels(&self) -> u16 {
        self.input_channels
    }

    fn open_input(
        &self,
        capture: Box<dyn FnMut(&[f32]) + Send>,
    ) -> Result<AudioStream, AudioError> {
        let mut capture = capture;
        let stream = self
            .input_device
            .build_input_stream(
                &self.input_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| capture(data),
                |_err| {}, // v0: stream errors are ignored, never panic
                None,
            )
            .map_err(|_| AudioError::StreamBuild)?;
        stream.play().map_err(|_| AudioError::StreamPlay)?;
        Ok(AudioStream::new(stream))
    }

    fn open_output(
        &self,
        render: Box<dyn FnMut(&mut [f32]) + Send>,
    ) -> Result<AudioStream, AudioError> {
        let mut render = render;
        let stream = self
            .output_device
            .build_output_stream(
                &self.output_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| render(data),
                |_err| {},
                None,
            )
            .map_err(|_| AudioError::StreamBuild)?;
        stream.play().map_err(|_| AudioError::StreamPlay)?;
        Ok(AudioStream::new(stream))
    }
}
