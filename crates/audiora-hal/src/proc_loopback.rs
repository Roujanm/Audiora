//! Per-process loopback capture (Windows 10 2004+ / Windows 11).
//!
//! Captures the audio one application (and its child processes) renders, before it reaches any
//! device — no driver needed. The stream is always 48 kHz 32-bit float stereo; Windows converts.

use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use windows::core::{implement, Interface, Ref, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
use windows::Win32::System::Com::{CoInitializeEx, BLOB, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::VT_BLOB;

use crate::{HalError, Result};

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;

/// The client handed back by `ActivateAudioInterfaceAsync` is agile: it may be used from any thread.
struct AgileClient(windows::core::Result<IAudioClient>);
// SAFETY: the completion handler is registered as `IAgileObject` and the activated interface
// is documented to be free-threaded.
unsafe impl Send for AgileClient {}

type Done = Arc<(Mutex<Option<AgileClient>>, Condvar)>;

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Handler {
    done: Done,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Handler_Impl {
    fn ActivateCompleted(&self, op: Ref<'_, IActivateAudioInterfaceAsyncOperation>) -> windows::core::Result<()> {
        let result = (|| {
            let op = op.ok()?;
            let mut hr = windows::core::HRESULT(0);
            let mut unk = None;
            unsafe { op.GetActivateResult(&mut hr, &mut unk)? };
            hr.ok()?;
            unk.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))?.cast::<IAudioClient>()
        })();
        let (slot, cv) = &*self.done;
        *slot.lock().unwrap() = Some(AgileClient(result));
        cv.notify_all();
        Ok(())
    }
}

/// Loopback capture of one process tree. Create and run on the same thread.
pub struct ProcessCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: HANDLE,
}

impl Drop for ProcessCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

impl ProcessCapture {
    /// Captures `pid` and every process it started.
    pub fn open(pid: u32) -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
                ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
                Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                    ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                        TargetProcessId: pid,
                        ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                    },
                },
            };
            let prop = PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_BLOB,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0 {
                            blob: BLOB {
                                cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                                pBlobData: &mut params as *mut _ as *mut u8,
                            },
                        },
                    }),
                },
            };
            let done: Done = Arc::new((Mutex::new(None), Condvar::new()));
            let handler: IActivateAudioInterfaceCompletionHandler = Handler { done: done.clone() }.into();
            let _op = ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                Some(&prop),
                &handler,
            )?;
            // The blob points at `params` on this stack; the prop must not free it.
            std::mem::forget(prop);

            let (slot, cv) = &*done;
            let mut guard = slot.lock().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while guard.is_none() {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    return Err(HalError("process loopback activation timed out".into()));
                }
                guard = cv.wait_timeout(guard, left).unwrap().0;
            }
            let client = guard.take().unwrap().0?;
            drop(guard);

            let fmt = WAVEFORMATEX {
                wFormatTag: 3, // IEEE float
                nChannels: CHANNELS as u16,
                nSamplesPerSec: SAMPLE_RATE,
                nAvgBytesPerSec: SAMPLE_RATE * CHANNELS as u32 * 4,
                nBlockAlign: CHANNELS as u16 * 4,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                200_000, // 20 ms
                0,
                &fmt,
                None,
            )?;
            let event = CreateEventW(None, false, false, PCWSTR::null())?;
            client.SetEventHandle(event)?;
            let capture: IAudioCaptureClient = client.GetService()?;
            Ok(Self { client, capture, event })
        }
    }

    /// Calls `on_audio(interleaved_stereo, frames)` for every packet until `stop` is set.
    /// Silent packets arrive as zeros. Returns an error if the process's stream goes away.
    pub fn run_until(&self, stop: &AtomicBool, mut on_audio: impl FnMut(&[f32], usize)) -> Result<()> {
        let mut zeros = vec![0f32; 4096 * CHANNELS];
        unsafe {
            self.client.Start()?;
            let result = (|| -> Result<()> {
                while !stop.load(Ordering::Relaxed) {
                    match WaitForSingleObject(self.event, 100) {
                        r if r == WAIT_OBJECT_0 => {}
                        r if r == WAIT_TIMEOUT => continue,
                        _ => return Err(HalError("wait on loopback event failed".into())),
                    }
                    while self.capture.GetNextPacketSize()? != 0 {
                        let mut data = std::ptr::null_mut();
                        let mut frames = 0u32;
                        let mut flags = 0u32;
                        self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                        let n = frames as usize * CHANNELS;
                        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                            if zeros.len() < n {
                                zeros.resize(n, 0.0);
                            }
                            on_audio(&zeros[..n], frames as usize);
                        } else {
                            on_audio(slice::from_raw_parts(data as *const f32, n), frames as usize);
                        }
                        self.capture.ReleaseBuffer(frames)?;
                    }
                }
                Ok(())
            })();
            let _ = self.client.Stop();
            result
        }
    }
}
