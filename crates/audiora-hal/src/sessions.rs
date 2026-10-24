//! Per-application audio sessions on playback devices: what the Windows
//! volume mixer shows. Lets Audiora list who is playing, with a live peak, and
//! change an application's volume or mute — without any driver.
//!
//! Note this controls an application's level on its current device; it cannot
//! move the application to another device (that needs the virtual driver).

use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, AudioSessionStateExpired, IAudioSessionControl2,
    IAudioSessionManager2, ISimpleAudioVolume, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::wasapi::{device_id, device_name, enumerator};
use crate::{HalError, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct AppSession {
    /// Stable identifier of this session on this device; pass it to [`set_session`].
    pub instance_id: String,
    pub device_id: String,
    pub device_name: String,
    pub pid: u32,
    /// Executable name without extension ("chrome", "Spotify"), or "System sounds".
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    /// Linear peak of the application's own output, 0.0..1.0.
    pub peak: f32,
    pub active: bool,
    pub system_sounds: bool,
}

fn process_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned())
    }
}

fn read_session(
    ctl: &windows::Win32::Media::Audio::IAudioSessionControl,
    dev_id: &str,
    dev_name: &str,
) -> Option<AppSession> {
    unsafe {
        let ctl2: IAudioSessionControl2 = ctl.cast().ok()?;
        let state = ctl.GetState().ok()?;
        if state == AudioSessionStateExpired {
            return None;
        }
        let pid = ctl2.GetProcessId().unwrap_or(0);
        let system_sounds = ctl2.IsSystemSoundsSession().0 == 0;
        let instance = {
            let p = ctl2.GetSessionInstanceIdentifier().ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            s?
        };
        let vol: ISimpleAudioVolume = ctl.cast().ok()?;
        let peak = ctl
            .cast::<IAudioMeterInformation>()
            .ok()
            .and_then(|m| m.GetPeakValue().ok())
            .unwrap_or(0.0);
        Some(AppSession {
            instance_id: instance,
            device_id: dev_id.to_string(),
            device_name: dev_name.to_string(),
            pid,
            name: if system_sounds {
                "System sounds".into()
            } else {
                process_name(pid).unwrap_or_else(|| format!("process {pid}"))
            },
            volume: vol.GetMasterVolume().unwrap_or(1.0),
            muted: vol.GetMute().map(|b| b.as_bool()).unwrap_or(false),
            peak,
            active: state == AudioSessionStateActive,
            system_sounds,
        })
    }
}

/// Every non-expired session on every active playback device.
pub fn list_sessions() -> Result<Vec<AppSession>> {
    let e = enumerator()?;
    let mut out = Vec::new();
    let coll = unsafe { e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    for i in 0..unsafe { coll.GetCount()? } {
        let dev = unsafe { coll.Item(i)? };
        let (id, name) = (device_id(&dev)?, device_name(&dev));
        let Ok(mgr) = (unsafe { dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) }) else {
            continue;
        };
        let Ok(sessions) = (unsafe { mgr.GetSessionEnumerator() }) else { continue };
        for k in 0..unsafe { sessions.GetCount().unwrap_or(0) } {
            if let Some(s) = unsafe { sessions.GetSession(k) }.ok().and_then(|c| read_session(&c, &id, &name)) {
                out.push(s);
            }
        }
    }
    Ok(out)
}

/// Changes one session's volume (0.0..=1.0) and/or mute.
pub fn set_session(
    device: &str,
    instance_id: &str,
    volume: Option<f32>,
    mute: Option<bool>,
) -> Result<()> {
    let e = enumerator()?;
    let coll = unsafe { e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    for i in 0..unsafe { coll.GetCount()? } {
        let dev = unsafe { coll.Item(i)? };
        if device_id(&dev)? != device {
            continue;
        }
        let mgr: IAudioSessionManager2 = unsafe { dev.Activate(CLSCTX_ALL, None)? };
        let sessions = unsafe { mgr.GetSessionEnumerator()? };
        for k in 0..unsafe { sessions.GetCount()? } {
            let ctl = unsafe { sessions.GetSession(k)? };
            let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
            let id = unsafe {
                let p = ctl2.GetSessionInstanceIdentifier()?;
                let s = p.to_string().ok();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            };
            if id.as_deref() != Some(instance_id) {
                continue;
            }
            let vol: ISimpleAudioVolume = ctl.cast()?;
            unsafe {
                if let Some(v) = volume {
                    vol.SetMasterVolume(v.clamp(0.0, 1.0), std::ptr::null())?;
                }
                if let Some(m) = mute {
                    vol.SetMute(m, std::ptr::null())?;
                }
            }
            return Ok(());
        }
        return Err(HalError("that application is no longer playing on this device".into()));
    }
    Err(HalError("unknown playback device".into()))
}
