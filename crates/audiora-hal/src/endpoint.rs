//! Windows' own volume and mute of an endpoint: the same switch as Settings > Sound. Muting the
//! microphone here silences it for every program, not only inside Audiora.

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::System::Com::CLSCTX_ALL;

use crate::wasapi::open_device_by_id;
use crate::Result;

fn volume(device_id: &str) -> Result<IAudioEndpointVolume> {
    let dev = open_device_by_id(device_id)?;
    Ok(unsafe { dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)? })
}

/// Whether Windows has the endpoint muted.
pub fn endpoint_mute(device_id: &str) -> Result<bool> {
    Ok(unsafe { volume(device_id)?.GetMute()?.as_bool() })
}

/// Mutes or unmutes the endpoint in Windows (every application hears the change).
pub fn set_endpoint_mute(device_id: &str, mute: bool) -> Result<()> {
    unsafe { volume(device_id)?.SetMute(mute, std::ptr::null())? };
    Ok(())
}

/// The endpoint's Windows level as 0..1 (the Sound settings slider).
pub fn endpoint_level(device_id: &str) -> Result<f32> {
    Ok(unsafe { volume(device_id)?.GetMasterVolumeLevelScalar()? })
}

/// Sets the endpoint's Windows level (0..1). For a microphone this is the analogue input gain.
pub fn set_endpoint_level(device_id: &str, level: f32) -> Result<()> {
    unsafe { volume(device_id)?.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null())? };
    Ok(())
}
