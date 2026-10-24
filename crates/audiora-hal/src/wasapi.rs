//! WASAPI shared-mode backend (event driven).

use std::slice;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::{DeviceInfo, Flow, HalError, Result, StreamFormat};

const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

fn init_com() {
    // Ignore the result: S_FALSE / RPC_E_CHANGED_MODE just mean COM is already up.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn enumerator() -> Result<IMMDeviceEnumerator> {
    init_com();
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? })
}

fn device_id(dev: &IMMDevice) -> Result<String> {
    unsafe {
        let p = dev.GetId()?;
        let s = p.to_string().map_err(|e| HalError(e.to_string()));
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn device_name(dev: &IMMDevice) -> String {
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

/// Shared-mode render stream on one device, driven by the device's event.
/// Must be used on the thread that created it.
pub struct RenderStream {
    client: IAudioClient,
    render: IAudioRenderClient,
    event: HANDLE,
    format: StreamFormat,
}

impl RenderStream {
    /// `None` opens the default render device.
    pub fn open(device_id_opt: Option<&str>) -> Result<Self> {
        let e = enumerator()?;
        let dev = unsafe {
            match device_id_opt {
                Some(id) => {
                    let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
                    e.GetDevice(PCWSTR(wide.as_ptr()))?
                }
                None => e.GetDefaultAudioEndpoint(eRender, eConsole)?,
            }
        };
        let client: IAudioClient = unsafe { dev.Activate(CLSCTX_ALL, None)? };
        let (fmt, is_float) = mix_format(&client)?;
        if !is_float || fmt.wBitsPerSample != 32 {
            return Err(HalError(format!(
                "mix format is not 32-bit float (tag {}, {} bits)",
                { fmt.wFormatTag },
                { fmt.wBitsPerSample }
            )));
        }
        unsafe {
            let ptr = client.GetMixFormat()?;
            let init = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                0,
                0,
                ptr,
                None,
            );
            CoTaskMemFree(Some(ptr as *const _));
            init?;
            let event = CreateEventW(None, false, false, None)?;
            client.SetEventHandle(event)?;
            let buffer_frames = client.GetBufferSize()?;
            let render: IAudioRenderClient = client.GetService()?;
            Ok(Self {
                client,
                render,
                event,
                format: StreamFormat {
                    sample_rate: fmt.nSamplesPerSec,
                    channels: fmt.nChannels,
                    buffer_frames,
                },
            })
        }
    }

    pub fn format(&self) -> StreamFormat {
        self.format
    }

    /// Calls `fill(interleaved_samples, frames)` whenever the device wants
    /// audio, for `seconds` of wall-clock time. The buffer is already zeroed.
    pub fn run(&self, seconds: f32, mut fill: impl FnMut(&mut [f32], usize)) -> Result<()> {
        let ch = self.format.channels as usize;
        unsafe {
            // Prime the device with one buffer of silence before starting.
            self.render.GetBuffer(self.format.buffer_frames)?;
            self.render
                .ReleaseBuffer(self.format.buffer_frames, windows::Win32::Media::Audio::AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)?;
            self.client.Start()?;

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f32(seconds);
            let result = (|| -> Result<()> {
                while std::time::Instant::now() < deadline {
                    if WaitForSingleObject(self.event, 2000) != WAIT_OBJECT_0 {
                        return Err(HalError("timed out waiting for device buffer event".into()));
                    }
                    let padding = self.client.GetCurrentPadding()?;
                    let frames = self.format.buffer_frames - padding;
                    if frames == 0 {
                        continue;
                    }
                    let data = self.render.GetBuffer(frames)?;
                    let samples = slice::from_raw_parts_mut(data as *mut f32, frames as usize * ch);
                    samples.fill(0.0);
                    fill(samples, frames as usize);
                    self.render.ReleaseBuffer(frames, 0)?;
                }
                Ok(())
            })();
            let _ = self.client.Stop();
            result
        }
    }
}

impl Drop for RenderStream {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

#[allow(dead_code)]
fn _assert_interface<T: Interface>() {}
