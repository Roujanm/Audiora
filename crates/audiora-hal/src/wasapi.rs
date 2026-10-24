//! WASAPI shared-mode backend (event driven).

use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IAudioClient3, IAudioRenderClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_EXCLUSIVE,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_STATE_ACTIVE, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0,
};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::KernelStreaming::KSDATAFORMAT_SUBTYPE_PCM;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject,
};

use crate::{DeviceInfo, Flow, HalError, Mode, Result, SampleFormat, StreamFormat};

const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

fn init_com() {
    // Ignore the result: S_FALSE / RPC_E_CHANGED_MODE just mean COM is already up.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

pub(crate) fn enumerator() -> Result<IMMDeviceEnumerator> {
    init_com();
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? })
}

pub(crate) fn device_id(dev: &IMMDevice) -> Result<String> {
    unsafe {
        let p = dev.GetId()?;
        let s = p.to_string().map_err(|e| HalError(e.to_string()));
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

pub(crate) fn device_name(dev: &IMMDevice) -> String {
    unsafe {
        dev.OpenPropertyStore(STGM_READ)
            .and_then(|s| s.GetValue(&PKEY_Device_FriendlyName))
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "(unknown)".into())
    }
}

/// Copies the mix format out and frees the COM allocation.
fn mix_format(client: &IAudioClient) -> Result<(WAVEFORMATEX, bool)> {
    unsafe {
        let ptr = client.GetMixFormat()?;
        let fmt = std::ptr::read_unaligned(ptr);
        let is_float = if fmt.wFormatTag == WAVE_FORMAT_IEEE_FLOAT {
            true
        } else if fmt.wFormatTag as u32 == WAVE_FORMAT_EXTENSIBLE {
            let ext = std::ptr::read_unaligned(ptr as *const WAVEFORMATEXTENSIBLE);
            let sub = ext.SubFormat;
            sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
        } else {
            false
        };
        CoTaskMemFree(Some(ptr as *const _));
        Ok((fmt, is_float))
    }
}

pub fn default_device_id(flow: Flow) -> Result<String> {
    let e = enumerator()?;
    let dev = unsafe {
        e.GetDefaultAudioEndpoint(if flow == Flow::Render { eRender } else { eCapture }, eConsole)?
    };
    device_id(&dev)
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let e = enumerator()?;
    let mut out = Vec::new();
    for flow in [Flow::Render, Flow::Capture] {
        let df = if flow == Flow::Render { eRender } else { eCapture };
        let default = default_device_id(flow).ok();
        let coll = unsafe { e.EnumAudioEndpoints(df, DEVICE_STATE_ACTIVE)? };
        for i in 0..unsafe { coll.GetCount()? } {
            let dev = unsafe { coll.Item(i)? };
            let id = device_id(&dev)?;
            let (rate, ch) = unsafe { dev.Activate::<IAudioClient>(CLSCTX_ALL, None) }
                .map_err(HalError::from)
                .and_then(|c| mix_format(&c))
                .map(|(f, _)| (f.nSamplesPerSec, f.nChannels))
                .unwrap_or((0, 0));
            out.push(DeviceInfo {
                is_default: default.as_deref() == Some(id.as_str()),
                name: device_name(&dev),
                id,
                flow,
                sample_rate: rate,
                channels: ch,
            });
        }
    }
    Ok(out)
}

struct Shared {
    client: IAudioClient,
    event: HANDLE,
    format: StreamFormat,
    /// Wire format; F32 streams are read and written in place, others are converted.
    sample: SampleFormat,
}

/// Registers the calling thread with the Multimedia Class Scheduler (Pro Audio)
/// so Windows keeps its audio callbacks on time under load.
struct Mmcss(Option<HANDLE>);

impl Mmcss {
    fn enter() -> Self {
        let mut task = 0u32;
        Self(unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task) }.ok())
    }
}

impl Drop for Mmcss {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            unsafe {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

fn open_device(flow: Flow, id: Option<&str>) -> Result<IMMDevice> {
    let e = enumerator()?;
    Ok(unsafe {
        match id {
            Some(id) => {
                let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
                e.GetDevice(PCWSTR(wide.as_ptr()))?
            }
            None => e.GetDefaultAudioEndpoint(
                if flow == Flow::Render { eRender } else { eCapture },
                eConsole,
            )?,
        }
    })
}

fn open_stream(flow: Flow, id: Option<&str>, mode: Mode) -> Result<Shared> {
    let dev = open_device(flow, id)?;
    if mode == Mode::Exclusive {
        return open_exclusive(&dev);
    }
    let low_latency = mode == Mode::LowLatency;
    let client: IAudioClient = unsafe { dev.Activate(CLSCTX_ALL, None) }
        .map_err(|e| HalError(format!("activate: {e}")))?;
    let (fmt, is_float) = mix_format(&client).map_err(|e| HalError(format!("mix format: {e}")))?;
    if !is_float || fmt.wBitsPerSample != 32 {
        return Err(HalError(format!(
            "mix format is not 32-bit float (tag {}, {} bits)",
            { fmt.wFormatTag },
            { fmt.wBitsPerSample }
        )));
    }
    unsafe {
        let ptr = client.GetMixFormat()?;
        let mut period_frames = 0u32;
        let mut achieved_low = false;
        let mut init: windows::core::Result<()> = Err(windows::core::Error::empty());

        if low_latency {
            // IAudioClient3: ask the engine for its smallest shared-mode period.
            if let Ok(c3) = client.cast::<IAudioClient3>() {
                let (mut def, mut fund, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
                if c3.GetSharedModeEnginePeriod(ptr, &mut def, &mut fund, &mut min, &mut max).is_ok()
                    && c3
                        .InitializeSharedAudioStream(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, min, ptr, None)
                        .is_ok()
                {
                    init = Ok(());
                    period_frames = min;
                    achieved_low = true;
                }
            }
        }
        if !achieved_low {
            init = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                0,
                0,
                ptr,
                None,
            );
            if init.is_ok() {
                let mut default_hns = 0i64;
                let mut min_hns = 0i64;
                if client.GetDevicePeriod(Some(&mut default_hns), Some(&mut min_hns)).is_ok() {
                    period_frames =
                        (default_hns as f64 * fmt.nSamplesPerSec as f64 / 10_000_000.0).round() as u32;
                }
            }
        }
        CoTaskMemFree(Some(ptr as *const _));
        init.map_err(|e| HalError(format!("initialize: {e}")))?;
        let event = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let buffer_frames = client.GetBufferSize()?;
        Ok(Shared {
            client,
            event,
            sample: SampleFormat::F32,
            format: StreamFormat {
                sample_rate: fmt.nSamplesPerSec,
                channels: fmt.nChannels,
                buffer_frames,
                period_frames: if period_frames == 0 { buffer_frames / 2 } else { period_frames },
                low_latency: achieved_low,
                exclusive: false,
                bit_depth: 32,
            },
        })
    }
}

fn wave_format(rate: u32, channels: u16, s: SampleFormat) -> WAVEFORMATEXTENSIBLE {
    let container_bits = (s.container_bytes() * 8) as u16;
    let block = channels * (container_bits / 8);
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: channels,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * block as u32,
            nBlockAlign: block,
            wBitsPerSample: container_bits,
            cbSize: 22,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: s.valid_bits() },
        dwChannelMask: match channels {
            1 => 0x4,
            2 => 0x3,
            _ => 0,
        },
        SubFormat: if s.is_float() { KSDATAFORMAT_SUBTYPE_IEEE_FLOAT } else { KSDATAFORMAT_SUBTYPE_PCM },
    }
}

const AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED: u32 = 0x8889_0019;

/// Exclusive, event-driven stream in the first native format the device accepts
/// (stereo preferred; the device own rate first, then common rates; float
/// before integer so conversion is avoided when possible), at its smallest period.
fn open_exclusive(dev: &IMMDevice) -> Result<Shared> {
    let probe: IAudioClient = unsafe { dev.Activate(CLSCTX_ALL, None) }
        .map_err(|e| HalError(format!("activate: {e}")))?;
    let (mix, _) = mix_format(&probe).map_err(|e| HalError(format!("mix format: {e}")))?;

    let mut rates = vec![mix.nSamplesPerSec];
    for r in [48_000, 44_100, 96_000, 88_200, 192_000, 176_400] {
        if !rates.contains(&r) {
            rates.push(r);
        }
    }
    let mut channel_counts = vec![2u16];
    if mix.nChannels != 2 {
        channel_counts.push(mix.nChannels);
    }

    let mut chosen = None;
    'search: for &rate in &rates {
        for &ch in &channel_counts {
            for &sf in &SampleFormat::ALL {
                let wf = wave_format(rate, ch, sf);
                let hr = unsafe {
                    probe.IsFormatSupported(
                        AUDCLNT_SHAREMODE_EXCLUSIVE,
                        &wf as *const WAVEFORMATEXTENSIBLE as *const WAVEFORMATEX,
                        None,
                    )
                };
                if hr.0 == 0 {
                    chosen = Some((rate, ch, sf, wf));
                    break 'search;
                }
            }
        }
    }
    let (rate, ch, sf, wf) = chosen.ok_or_else(|| {
        HalError("the device accepts none of the exclusive formats tried (in use by another exclusive app, or exclusive mode is disabled for it)".into())
    })?;

    let mut period = 0i64;
    unsafe { probe.GetDevicePeriod(None, Some(&mut period))? };
    let wfp = &wf as *const WAVEFORMATEXTENSIBLE as *const WAVEFORMATEX;
    let mut client = probe;
    for attempt in 0..2 {
        let init = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_EXCLUSIVE,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                period,
                period,
                wfp,
                None,
            )
        };
        match init {
            Ok(()) => break,
            // The device wants a period that is a whole number of its internal blocks:
            // read the size it rounded to, then retry on a fresh client.
            Err(e) if e.code().0 as u32 == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED && attempt == 0 => {
                let frames = unsafe { client.GetBufferSize()? };
                period = (10_000_000f64 * frames as f64 / rate as f64 + 0.5) as i64;
                client = unsafe { dev.Activate(CLSCTX_ALL, None)? };
            }
            Err(e) => return Err(HalError(format!("exclusive initialize: {e}"))),
        }
    }
    unsafe {
        let event = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let buffer_frames = client.GetBufferSize()?;
        Ok(Shared {
            client,
            event,
            sample: sf,
            format: StreamFormat {
                sample_rate: rate,
                channels: ch,
                buffer_frames,
                period_frames: buffer_frames,
                low_latency: true,
                exclusive: true,
                bit_depth: sf.valid_bits(),
            },
        })
    }
}

/// Waits up to 100 ms (keeps stop requests responsive); a timeout is not an error.
fn wait_event(event: HANDLE) -> Result<bool> {
    match unsafe { WaitForSingleObject(event, 100) } {
        r if r == WAIT_OBJECT_0 => Ok(true),
        r if r == WAIT_TIMEOUT => Ok(false),
        _ => Err(HalError("wait on device event failed".into())),
    }
}

/// Shared-mode render stream on one device, driven by the device's event.
/// Must be used on the thread that created it.
pub struct RenderStream {
    s: Shared,
    render: IAudioRenderClient,
}

impl RenderStream {
    /// `None` opens the default render device.
    pub fn open(device_id: Option<&str>, mode: Mode) -> Result<Self> {
        let s = open_stream(Flow::Render, device_id, mode)?;
        let render: IAudioRenderClient = unsafe { s.client.GetService()? };
        Ok(Self { s, render })
    }

    pub fn format(&self) -> StreamFormat {
        self.s.format
    }

    /// Runs for `seconds` of wall-clock time.
    pub fn run(&self, seconds: f32, fill: impl FnMut(&mut [f32], usize)) -> Result<()> {
        let stop = AtomicBool::new(false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f32(seconds);
        self.run_until(&stop, Some(deadline), fill)
    }

    /// Calls `fill(interleaved_samples, frames)` whenever the device wants
    /// audio until `stop` is set or `deadline` passes. The buffer is pre-zeroed.
    pub fn run_until(
        &self,
        stop: &AtomicBool,
        deadline: Option<std::time::Instant>,
        mut fill: impl FnMut(&mut [f32], usize),
    ) -> Result<()> {
        let f = self.s.format;
        let ch = f.channels as usize;
        let sample = self.s.sample;
        let mut scratch = if sample.is_float() { Vec::new() } else { vec![0f32; f.buffer_frames as usize * ch] };
        let _mmcss = Mmcss::enter();
        unsafe {
            // Prime the device with one buffer of silence before starting.
            self.render.GetBuffer(f.buffer_frames)?;
            self.render.ReleaseBuffer(f.buffer_frames, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)?;
            self.s.client.Start()?;
            let result = (|| -> Result<()> {
                while !stop.load(Ordering::Relaxed)
                    && deadline.map_or(true, |d| std::time::Instant::now() < d)
                {
                    if !wait_event(self.s.event)? {
                        continue;
                    }
                    // Exclusive event-driven streams hand over one whole buffer per event;
                    // shared streams report how much of the buffer is still queued.
                    let frames = if f.exclusive {
                        f.buffer_frames
                    } else {
                        f.buffer_frames - self.s.client.GetCurrentPadding()?
                    };
                    if frames == 0 {
                        continue;
                    }
                    let data = self.render.GetBuffer(frames)?;
                    let n = frames as usize * ch;
                    if sample.is_float() {
                        let samples = slice::from_raw_parts_mut(data as *mut f32, n);
                        samples.fill(0.0);
                        fill(samples, frames as usize);
                    } else {
                        scratch[..n].fill(0.0);
                        fill(&mut scratch[..n], frames as usize);
                        sample.encode(&scratch[..n], slice::from_raw_parts_mut(data, n * sample.container_bytes()));
                    }
                    self.render.ReleaseBuffer(frames, 0)?;
                }
                Ok(())
            })();
            let _ = self.s.client.Stop();
            result
        }
    }
}

/// Shared-mode capture stream. Must be used on the thread that created it.
pub struct CaptureStream {
    s: Shared,
    capture: IAudioCaptureClient,
}

impl CaptureStream {
    /// `None` opens the default capture device.
    pub fn open(device_id: Option<&str>, mode: Mode) -> Result<Self> {
        let s = open_stream(Flow::Capture, device_id, mode)?;
        let capture: IAudioCaptureClient = unsafe { s.client.GetService()? };
        Ok(Self { s, capture })
    }

    pub fn format(&self) -> StreamFormat {
        self.s.format
    }

    /// Calls `on_audio(interleaved_samples, frames)` for every captured packet
    /// (silent packets are delivered as zeros) until `stop` is set.
    pub fn run_until(
        &self,
        stop: &AtomicBool,
        mut on_audio: impl FnMut(&[f32], usize),
    ) -> Result<()> {
        let ch = self.s.format.channels as usize;
        let sample = self.s.sample;
        let exclusive = self.s.format.exclusive;
        let mut zeros = vec![0f32; self.s.format.buffer_frames as usize * ch];
        let mut decoded = if sample.is_float() { Vec::new() } else { vec![0f32; zeros.len()] };
        let _mmcss = Mmcss::enter();
        unsafe {
            self.s.client.Start()?;
            let result = (|| -> Result<()> {
                while !stop.load(Ordering::Relaxed) {
                    if !wait_event(self.s.event)? {
                        continue;
                    }
                    loop {
                        // Exclusive event-driven capture hands over exactly one buffer per
                        // event, and GetNextPacketSize is not meaningful there: reading until
                        // it reports 0 re-reads stale data.
                        if !exclusive && self.capture.GetNextPacketSize()? == 0 {
                            break;
                        }
                        let mut data = std::ptr::null_mut();
                        let mut frames = 0u32;
                        let mut flags = 0u32;
                        self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                        if frames == 0 {
                            break;
                        }
                        let n = frames as usize * ch;
                        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                            if zeros.len() < n {
                                zeros.resize(n, 0.0);
                            }
                            on_audio(&zeros[..n], frames as usize);
                        } else if sample.is_float() {
                            on_audio(slice::from_raw_parts(data as *const f32, n), frames as usize);
                        } else {
                            if decoded.len() < n {
                                decoded.resize(n, 0.0);
                            }
                            sample.decode(slice::from_raw_parts(data, n * sample.container_bytes()), &mut decoded[..n]);
                            on_audio(&decoded[..n], frames as usize);
                        }
                        self.capture.ReleaseBuffer(frames)?;
                        if exclusive {
                            break;
                        }
                    }
                }
                Ok(())
            })();
            let _ = self.s.client.Stop();
            result
        }
    }
}
