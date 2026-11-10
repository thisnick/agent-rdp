//! Audio streaming for IPC clients: the remote computer's sound (RDPSND, MS-RDPEA) and a
//! microphone fed by the client (AUDIO_INPUT, MS-RDPEAI).
//!
//! Nothing plays on or records from the local machine: a client sends `audio_attach` on an IPC
//! connection, and that connection then carries binary frames both ways:
//!
//! ```text
//! frame = kind: u8, length: u32 (little endian), payload
//!
//! daemon -> client
//!   1 PCM    rate: u32, channels: u16, bits: u16, then interleaved signed 16-bit samples (LE)
//!   2 EVENT  UTF-8 JSON, one of
//!            {"event":"mic_open","rate":..,"channels":..}   an application on the remote side records
//!            {"event":"mic_close"}
//!            {"event":"playing","id":..}                   a microphone chunk starts
//!            {"event":"played","id":..,"stopped":false}    ... and is through (or was dropped)
//!            {"event":"sound_close"}                       the remote side closed its sound output
//!
//! client -> daemon
//!   1 MIC    id: u32, rate: u32, channels: u16, then interleaved signed 16-bit samples (LE)
//!   2 CMD    UTF-8 JSON: {"cmd":"stop"} drops what is queued for the microphone
//! ```
//!
//! The microphone runs on the daemon's clock: queued chunks are resampled to the format the
//! server opened and sent in real time, silence when nothing is queued. While no application on
//! the remote side records, queued chunks still run through at real time (and are dropped), so
//! `played` always follows `playing` after the chunk's duration.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ironrdp_rdpeai::client::{AudioPacketSink, RdpeaiCaptureHandler};
use ironrdp_rdpsnd::client::RdpsndClientHandler;
use ironrdp_rdpsnd::pdu::{AudioFormat, PitchPdu, VolumePdu, WaveFormat};
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub const FRAME_PCM: u8 = 1;
pub const FRAME_EVENT: u8 = 2;
/// Client frames use the same numbers: 1 = microphone samples, 2 = command.
pub const FRAME_MIC: u8 = 1;
pub const FRAME_CMD: u8 = 2;

/// Largest frame a client may send (about 10 s of 48 kHz stereo).
const MAX_CLIENT_FRAME: usize = 4 * 1024 * 1024;
/// Microphone audio queued beyond this is refused (oldest chunks are dropped).
const MAX_QUEUED_SECONDS: f64 = 120.0;
/// Pace of the microphone clock.
const TICK: Duration = Duration::from_millis(10);
/// The rate the microphone clock runs at while no application records.
const IDLE_RATE: u32 = 16_000;

/// Something for the attached clients.
#[derive(Debug, Clone)]
pub enum AudioOut {
    Pcm { rate: u32, channels: u16, data: Arc<Vec<u8>> },
    Event(String),
}

/// One chunk of microphone audio from a client.
struct MicChunk {
    id: u32,
    rate: u32,
    channels: u16,
    samples: Vec<i16>,
    /// Position in source frames (fractional, for resampling).
    pos: f64,
    started: bool,
}

impl MicChunk {
    fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }

    /// Mono sample at a fractional source frame position (linear interpolation, channels mixed).
    fn sample_at(&self, pos: f64) -> f32 {
        let ch = usize::from(self.channels.max(1));
        let frames = self.frames();
        let i = pos.floor() as usize;
        let frac = (pos - pos.floor()) as f32;
        let at = |f: usize| -> f32 {
            let f = f.min(frames.saturating_sub(1));
            let mut sum = 0.0f32;
            for c in 0..ch {
                sum += f32::from(self.samples[f * ch + c]);
            }
            sum / ch as f32
        };
        at(i) * (1.0 - frac) + at(i + 1) * frac
    }
}

/// The format the server opened the microphone with, and where its packets go.
struct MicOpen {
    rate: u32,
    channels: u16,
    /// Bytes of one Data PDU (whole frames of `channels` 16-bit samples).
    packet_bytes: usize,
    sink: AudioPacketSink,
    packet: Vec<u8>,
}

struct MicState {
    queue: VecDeque<MicChunk>,
    open: Option<MicOpen>,
    /// Frames produced since `clock_start` at `clock_rate`.
    produced: u64,
    clock_start: Instant,
    clock_rate: u32,
}

impl MicState {
    fn restart_clock(&mut self, rate: u32) {
        self.produced = 0;
        self.clock_start = Instant::now();
        self.clock_rate = rate;
    }

    fn queued_seconds(&self) -> f64 {
        self.queue
            .iter()
            .map(|c| (c.frames() as f64 - c.pos).max(0.0) / f64::from(c.rate.max(1)))
            .sum()
    }
}

/// Shared audio state of a daemon: one RDP session, any number of attached clients.
pub struct AudioHub {
    out: broadcast::Sender<AudioOut>,
    mic: Mutex<MicState>,
}

pub type SharedAudioHub = Arc<AudioHub>;

static HUB: std::sync::OnceLock<SharedAudioHub> = std::sync::OnceLock::new();

/// The audio hub of this daemon (it owns one session). Call from inside the tokio runtime.
pub fn hub() -> SharedAudioHub {
    Arc::clone(HUB.get_or_init(AudioHub::new))
}

impl AudioHub {
    pub fn new() -> SharedAudioHub {
        let (out, _) = broadcast::channel(512);
        let hub = Arc::new(Self {
            out,
            mic: Mutex::new(MicState {
                queue: VecDeque::new(),
                open: None,
                produced: 0,
                clock_start: Instant::now(),
                clock_rate: IDLE_RATE,
            }),
        });
        let weak = Arc::downgrade(&hub);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                let Some(hub) = weak.upgrade() else { break };
                hub.pump();
            }
        });
        hub
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AudioOut> {
        self.out.subscribe()
    }

    fn event(&self, json: serde_json::Value) {
        let _ = self.out.send(AudioOut::Event(json.to_string()));
    }

    fn pcm(&self, rate: u32, channels: u16, data: &[u8]) {
        if self.out.receiver_count() == 0 {
            return;
        }
        let _ = self.out.send(AudioOut::Pcm { rate, channels, data: Arc::new(data.to_vec()) });
    }

    /// Queues microphone samples (interleaved 16-bit at `rate`).
    pub fn push_mic(&self, id: u32, rate: u32, channels: u16, samples: Vec<i16>) {
        if rate == 0 || channels == 0 || samples.is_empty() {
            self.event(serde_json::json!({ "event": "played", "id": id, "stopped": true }));
            return;
        }
        let mut dropped = Vec::new();
        {
            let mut mic = self.mic.lock();
            mic.queue.push_back(MicChunk { id, rate, channels, samples, pos: 0.0, started: false });
            while mic.queue.len() > 1 && mic.queued_seconds() > MAX_QUEUED_SECONDS {
                if let Some(c) = mic.queue.pop_front() {
                    dropped.push(c.id);
                }
            }
        }
        for id in dropped {
            self.event(serde_json::json!({ "event": "played", "id": id, "stopped": true }));
        }
    }

    /// Drops everything queued for the microphone.
    pub fn stop_mic(&self) {
        let ids: Vec<u32> = self.mic.lock().queue.drain(..).map(|c| c.id).collect();
        for id in ids {
            self.event(serde_json::json!({ "event": "played", "id": id, "stopped": true }));
        }
    }

    fn mic_open(&self, rate: u32, channels: u16, packet_bytes: usize, sink: AudioPacketSink) {
        {
            let mut mic = self.mic.lock();
            mic.open = Some(MicOpen { rate, channels, packet_bytes, sink, packet: Vec::with_capacity(packet_bytes) });
            mic.restart_clock(rate);
        }
        info!(rate, channels, packet_bytes, "Microphone opened by the remote side");
        self.event(serde_json::json!({ "event": "mic_open", "rate": rate, "channels": channels }));
    }

    fn mic_close(&self) {
        let was_open = {
            let mut mic = self.mic.lock();
            let was = mic.open.take().is_some();
            mic.restart_clock(IDLE_RATE);
            was
        };
        if was_open {
            info!("Microphone closed by the remote side");
            self.event(serde_json::json!({ "event": "mic_close" }));
        }
    }

    /// Produces the microphone frames that are due by the clock.
    fn pump(&self) {
        let mut events = Vec::new();
        {
            let mut guard = self.mic.lock();
            let mic = &mut *guard;
            let rate = mic.clock_rate;
            let due = (mic.clock_start.elapsed().as_secs_f64() * f64::from(rate)) as u64;
            // A daemon that was stalled for long does not burst the backlog out.
            if due > mic.produced + u64::from(rate) {
                mic.produced = due - u64::from(rate) / 10;
            }
            while mic.produced < due {
                // One output frame (mono value), from the head chunk or silence.
                let value = loop {
                    let Some(head) = mic.queue.front_mut() else { break 0.0f32 };
                    if !head.started {
                        head.started = true;
                        events.push(serde_json::json!({ "event": "playing", "id": head.id }));
                    }
                    if head.pos >= head.frames() as f64 {
                        let id = head.id;
                        mic.queue.pop_front();
                        events.push(serde_json::json!({ "event": "played", "id": id, "stopped": false }));
                        continue;
                    }
                    let v = head.sample_at(head.pos);
                    head.pos += f64::from(head.rate) / f64::from(rate);
                    break v;
                };
                mic.produced += 1;
                if let Some(open) = mic.open.as_mut() {
                    let s = value.round().clamp(-32768.0, 32767.0) as i16;
                    for _ in 0..open.channels.max(1) {
                        open.packet.extend_from_slice(&s.to_le_bytes());
                    }
                    if open.packet.len() >= open.packet_bytes {
                        let packet = std::mem::replace(&mut open.packet, Vec::with_capacity(open.packet_bytes));
                        (open.sink)(packet);
                    }
                }
            }
            // A chunk that ended exactly on the last frame is reported now, not on the next tick.
            while mic.queue.front().is_some_and(|c| c.started && c.pos >= c.frames() as f64) {
                if let Some(c) = mic.queue.pop_front() {
                    events.push(serde_json::json!({ "event": "played", "id": c.id, "stopped": false }));
                }
            }
        }
        for e in events {
            self.event(e);
        }
    }
}

/// 16-bit PCM formats, preferred first.
fn pcm_formats() -> Vec<AudioFormat> {
    [(48_000, 2), (44_100, 2), (48_000, 1), (44_100, 1), (22_050, 2), (22_050, 1), (16_000, 1), (16_000, 2)]
        .into_iter()
        .map(|(rate, channels)| pcm_format(rate, channels))
        .collect()
}

fn pcm_format(rate: u32, channels: u16) -> AudioFormat {
    AudioFormat {
        format: WaveFormat::PCM,
        n_channels: channels,
        n_samples_per_sec: rate,
        n_avg_bytes_per_sec: rate * u32::from(channels) * 2,
        n_block_align: channels * 2,
        bits_per_sample: 16,
        data: None,
    }
}

fn is_pcm16(f: &AudioFormat) -> bool {
    f.format == WaveFormat::PCM && f.bits_per_sample == 16 && f.n_channels >= 1 && f.n_channels <= 2
}

/// The server's 16-bit PCM formats in our order of preference, then any others it lists.
fn choose_pcm(server_formats: &[AudioFormat]) -> Vec<AudioFormat> {
    let mut chosen: Vec<AudioFormat> = pcm_formats()
        .into_iter()
        .filter_map(|want| {
            server_formats
                .iter()
                .find(|s| is_pcm16(s) && s.n_samples_per_sec == want.n_samples_per_sec && s.n_channels == want.n_channels)
                .cloned()
        })
        .collect();
    for s in server_formats.iter().filter(|s| is_pcm16(s)) {
        if !chosen.contains(s) {
            chosen.push(s.clone());
        }
    }
    chosen
}

/// RDPSND backend: hands every wave block to the attached clients.
#[derive(Clone)]
pub struct AudioPlayback {
    hub: SharedAudioHub,
    formats: Vec<AudioFormat>,
}

impl core::fmt::Debug for AudioPlayback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AudioPlayback").finish_non_exhaustive()
    }
}

impl AudioPlayback {
    pub fn new(hub: SharedAudioHub) -> Self {
        Self { hub, formats: pcm_formats() }
    }
}

impl RdpsndClientHandler for AudioPlayback {
    fn get_formats(&self) -> &[AudioFormat] {
        &self.formats
    }

    fn choose_formats(&self, server_formats: &[AudioFormat]) -> Vec<AudioFormat> {
        let chosen = choose_pcm(server_formats);
        debug!(count = chosen.len(), "RDPSND formats chosen");
        chosen
    }

    fn wave(&mut self, format: &AudioFormat, _ts: u32, data: Cow<'_, [u8]>) {
        if !is_pcm16(format) {
            warn!(?format, "Wave block in a format other than 16-bit PCM, dropped");
            return;
        }
        self.hub.pcm(format.n_samples_per_sec, format.n_channels, &data);
    }

    fn set_volume(&mut self, _volume: VolumePdu) {}

    fn set_pitch(&mut self, _pitch: PitchPdu) {}

    fn close(&mut self) {
        self.hub.event(serde_json::json!({ "event": "sound_close" }));
    }
}

/// AUDIO_INPUT backend: the microphone is the hub's queue.
pub struct AudioCapture {
    hub: SharedAudioHub,
    formats: Vec<AudioFormat>,
}

impl AudioCapture {
    pub fn new(hub: SharedAudioHub) -> Self {
        Self { hub, formats: pcm_formats() }
    }
}

impl RdpeaiCaptureHandler for AudioCapture {
    fn supported_formats(&self) -> &[AudioFormat] {
        &self.formats
    }

    fn choose_formats(&self, server_formats: &[AudioFormat]) -> Vec<AudioFormat> {
        choose_pcm(server_formats)
    }

    fn open(&mut self, capture_format: &AudioFormat, encode_format: &AudioFormat, packet_size: usize, sink: AudioPacketSink) -> i32 {
        if !is_pcm16(encode_format) {
            warn!(?encode_format, "AUDIO_INPUT asked for a format other than 16-bit PCM");
            return 0x8000_4005_u32 as i32; // E_FAIL
        }
        // `packet_size` counts frames of the capture format; the packets carry the encode format.
        let frames = packet_size / (usize::from(capture_format.n_channels.max(1)) * 2);
        let packet_bytes = frames.max(1) * usize::from(encode_format.n_channels) * 2;
        self.hub.mic_open(encode_format.n_samples_per_sec, encode_format.n_channels, packet_bytes, sink);
        0
    }

    fn set_format(&mut self, encode_format: &AudioFormat, packet_size: usize) -> bool {
        if !is_pcm16(encode_format) {
            return false;
        }
        let mut mic = self.hub.mic.lock();
        let Some(open) = mic.open.as_mut() else { return false };
        let frames = open.packet_bytes / (usize::from(open.channels.max(1)) * 2);
        let _ = packet_size;
        open.rate = encode_format.n_samples_per_sec;
        open.channels = encode_format.n_channels;
        open.packet_bytes = frames.max(1) * usize::from(open.channels) * 2;
        open.packet.clear();
        let rate = open.rate;
        mic.restart_clock(rate);
        true
    }

    fn close(&mut self) {
        self.hub.mic_close();
    }
}

async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, kind: u8, parts: &[&[u8]]) -> std::io::Result<()> {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    let mut head = [0u8; 5];
    head[0] = kind;
    head[1..].copy_from_slice(&(len as u32).to_le_bytes());
    w.write_all(&head).await?;
    for p in parts {
        w.write_all(p).await?;
    }
    w.flush().await
}

/// Serves one attached IPC connection until either side closes it.
pub async fn serve<R, W>(hub: SharedAudioHub, mut reader: R, mut writer: W) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut rx = hub.subscribe();
    let send = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AudioOut::Pcm { rate, channels, data }) => {
                    let mut head = [0u8; 8];
                    head[..4].copy_from_slice(&rate.to_le_bytes());
                    head[4..6].copy_from_slice(&channels.to_le_bytes());
                    head[6..].copy_from_slice(&16u16.to_le_bytes());
                    if write_frame(&mut writer, FRAME_PCM, &[&head, &data]).await.is_err() {
                        break;
                    }
                }
                Ok(AudioOut::Event(json)) => {
                    if write_frame(&mut writer, FRAME_EVENT, &[json.as_bytes()]).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => warn!(n, "Audio client lagged, frames dropped"),
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let result: anyhow::Result<()> = async {
        loop {
            let mut head = [0u8; 5];
            match reader.read_exact(&mut head).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e.into()),
            }
            let len = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
            if len > MAX_CLIENT_FRAME {
                anyhow::bail!("audio frame of {len} bytes is too large");
            }
            let mut payload = vec![0u8; len];
            reader.read_exact(&mut payload).await?;
            match head[0] {
                FRAME_MIC if len >= 10 => {
                    let id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    let rate = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
                    let channels = u16::from_le_bytes([payload[8], payload[9]]);
                    let samples = payload[10..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
                    hub.push_mic(id, rate, channels, samples);
                }
                FRAME_CMD => {
                    let cmd: serde_json::Value = serde_json::from_slice(&payload).unwrap_or_default();
                    if cmd.get("cmd").and_then(|c| c.as_str()) == Some("stop") {
                        hub.stop_mic();
                    }
                }
                other => warn!(kind = other, len, "Unknown audio frame from client"),
            }
        }
    }
    .await;
    send.abort();
    result
}
