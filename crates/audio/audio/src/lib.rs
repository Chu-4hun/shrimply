use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use shrimply_math_core::Fraction;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use shrimply_project::project::{Project, Time};

pub use shrimply_project::project;

use shrimply_math_media as math;

pub mod beat;
mod beat_cache;
mod beat_math;
mod effects;
pub mod modifier_cache;
mod opus_cache;
mod output;
pub mod pneuma;
pub mod recording;
pub mod streaming;
pub mod waveform;
mod worker;

use output::{OutputState, PlaybackWindow};
use worker::AudioCommand;

const CHANNELS: usize = 2;
const SCRUB_PREVIEW_MS: u64 = 120;
const OUTPUT_PERIOD_MS: u32 = 40;

#[derive(Default)]
pub struct AudioLevels {
    peaks: [AtomicU32; CHANNELS],
}

pub type SharedAudioLevels = Arc<AudioLevels>;

impl AudioLevels {
    pub fn take_peaks(&self) -> [f32; CHANNELS] {
        self.peaks
            .each_ref()
            .map(|peak| f32::from_bits(peak.swap(0, Ordering::Relaxed)))
    }

    fn record(&self, left: f32, right: f32) {
        for (peak, sample) in self.peaks.iter().zip([left, right]) {
            let sample = sample.abs();
            if sample.is_finite() {
                peak.fetch_max(sample.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

pub struct AudioPlayer {
    _stream: cpal::Stream,
    command_tx: Sender<AudioCommand>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stopped: AtomicBool,
    playing: Arc<AtomicBool>,
    previewing: Arc<AtomicBool>,
    cursor_frame: Arc<AtomicU64>,
    preview_end_frame: Arc<AtomicU64>,
    duration_frames: Arc<AtomicU64>,
    sample_rate: u32,
    failure: Arc<Mutex<Option<String>>>,
}

impl AudioPlayer {
    pub fn new(project: &Project, levels: SharedAudioLevels) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No default audio output device".to_string())?;
        let default_config = device
            .default_output_config()
            .map_err(|error| error.to_string())?;
        let output_config = device
            .supported_output_configs()
            .map_err(|error| error.to_string())?
            .find(|range| {
                range.channels() == default_config.channels()
                    && range.sample_format() == default_config.sample_format()
                    && range.min_sample_rate() <= 48_000
                    && range.max_sample_rate() >= 48_000
            })
            .map(|range| range.with_sample_rate(48_000))
            .unwrap_or(default_config);
        let mut config = output_config.config();
        if let cpal::SupportedBufferSize::Range { min, max } = *output_config.buffer_size() {
            let period_frames = config
                .sample_rate
                .saturating_mul(OUTPUT_PERIOD_MS)
                .div_ceil(1_000)
                .clamp(min, max);
            config.buffer_size = cpal::BufferSize::Fixed(period_frames);
        }
        let output_channels = config.channels as usize;
        let sample_rate = config.sample_rate;
        tracing::info!(
            "Building streaming audio output: device={}, sample_format={:?}, sample_rate={}, channels={}, buffer_size={:?}",
            device,
            output_config.sample_format(),
            sample_rate,
            output_channels,
            config.buffer_size,
        );

        let window = Arc::new(RwLock::new(Arc::new(PlaybackWindow::new())));
        let playing = Arc::new(AtomicBool::new(false));
        let previewing = Arc::new(AtomicBool::new(false));
        let cursor_frame = Arc::new(AtomicU64::new(0));
        let preview_end_frame = Arc::new(AtomicU64::new(0));
        let duration_frames = Arc::new(AtomicU64::new(
            project.duration().as_sample_frame(sample_rate),
        ));
        let failure = Arc::new(Mutex::new(None));
        let recoverable_error_log = Arc::new(Mutex::new(None));
        let output_state = OutputState {
            window: window.clone(),
            playing: playing.clone(),
            previewing: previewing.clone(),
            cursor_frame: cursor_frame.clone(),
            preview_end_frame: preview_end_frame.clone(),
            duration_frames: duration_frames.clone(),
            output_channels,
            levels,
            failure: failure.clone(),
            recoverable_error_log,
        };
        let stream = output::build_stream(
            &device,
            &config,
            output_config.sample_format(),
            output_state,
        )?;
        let (command_tx, worker) = worker::spawn(
            project.clone(),
            sample_rate,
            window,
            playing.clone(),
            previewing.clone(),
            cursor_frame.clone(),
            preview_end_frame.clone(),
            duration_frames.clone(),
            failure.clone(),
        );
        stream.play().map_err(|error| error.to_string())?;
        tracing::info!("Streaming audio output ready");

        Ok(Self {
            _stream: stream,
            command_tx,
            worker: Mutex::new(Some(worker)),
            stopped: AtomicBool::new(false),
            playing,
            previewing,
            cursor_frame,
            preview_end_frame,
            duration_frames,
            sample_rate,
            failure,
        })
    }

    pub fn set_project(&self, project: &Project) {
        tracing::trace!("Updating streaming audio timeline");
        self.duration_frames.store(
            project.duration().as_sample_frame(self.sample_rate),
            Ordering::SeqCst,
        );
        self.send(AudioCommand::SetProject(Box::new(project.clone())));
    }

    pub fn seek(&self, position: Time) {
        let frame = position
            .as_sample_frame(self.sample_rate)
            .min(self.duration_frames.load(Ordering::SeqCst));
        tracing::trace!(
            "Streaming audio seek to {} (frame {frame})",
            position.as_label()
        );
        self.cursor_frame.store(frame, Ordering::SeqCst);
        self.previewing.store(false, Ordering::SeqCst);
        self.send(AudioCommand::Seek { frame });
    }

    pub fn preview_from(&self, position: Time) {
        let frame = position
            .as_sample_frame(self.sample_rate)
            .min(self.duration_frames.load(Ordering::SeqCst));
        let preview_frames = self.sample_rate as u64 * SCRUB_PREVIEW_MS / 1_000;
        let end_frame = frame
            .saturating_add(preview_frames)
            .min(self.duration_frames.load(Ordering::SeqCst));
        if frame >= end_frame {
            return;
        }

        tracing::trace!(
            "Streaming audio scrub preview from {} (frames {frame}..{end_frame})",
            position.as_label()
        );
        self.cursor_frame.store(frame, Ordering::SeqCst);
        self.preview_end_frame.store(end_frame, Ordering::SeqCst);
        self.previewing.store(false, Ordering::SeqCst);
        self.send(AudioCommand::Preview {
            frame,
            frames: end_frame.saturating_sub(frame) as usize,
        });
    }

    pub fn set_playback_speed(&self, playback_speed: Fraction) {
        self.send(AudioCommand::SetPlaybackSpeed(playback_speed));
    }

    pub fn set_playing(&self, playing: bool) {
        tracing::trace!("Streaming audio set playing={playing}");
        if playing {
            self.previewing.store(false, Ordering::SeqCst);
            self.playing.store(true, Ordering::SeqCst);
            self.send(AudioCommand::PlayFrom {
                frame: self.cursor_frame.load(Ordering::SeqCst),
            });
        } else {
            self.playing.store(false, Ordering::SeqCst);
            self.previewing.store(false, Ordering::SeqCst);
            self.send(AudioCommand::Pause);
        }
    }

    pub fn stop(&self) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::debug!("Stopping streaming audio output");
        self.playing.store(false, Ordering::SeqCst);
        self.previewing.store(false, Ordering::SeqCst);
        self.send(AudioCommand::Stop);
        let worker = match self.worker.lock() {
            Ok(mut worker) => worker.take(),
            Err(error) => error.into_inner().take(),
        };
        if let Some(worker) = worker
            && worker.join().is_err()
        {
            let error = "streaming audio worker panicked during shutdown".to_string();
            tracing::error!(%error);
            match self.failure.lock() {
                Ok(mut failure) => *failure = Some(error),
                Err(poisoned) => *poisoned.into_inner() = Some(error),
            }
        }
    }

    pub fn take_failure(&self) -> Option<String> {
        match self.failure.lock() {
            Ok(mut failure) => failure.take(),
            Err(error) => error.into_inner().take(),
        }
    }

    fn send(&self, command: AudioCommand) {
        if let Err(error) = self.command_tx.send(command) {
            tracing::warn!("Could not send streaming audio command: {error}");
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
