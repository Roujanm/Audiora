//! Per-application default output device — what Windows shows under
//! Settings > Sound > Volume mixer > "Output device" for each app.
//!
//! There is no public API for this. It is the `AudioPolicyConfig` WinRT factory that the Settings
//! app and other per-app audio tools use; the interface id changed between Windows releases, so both known ids
//! are tried and the result is verified by reading the setting back.

use std::ffi::c_void;

use windows::core::{Interface, GUID, HRESULT, HSTRING};
use windows::Win32::System::WinRT::RoGetActivationFactory;
use windows_core::IInspectable;

use crate::{HalError, Result};

/// Windows 10 21H2 / Windows 11 and later.
const IID_NEW: GUID = GUID::from_u128(0xab3d4648_e242_459f_b02f_541c70306324);
/// Windows 10 before 21H2.
const IID_OLD: GUID = GUID::from_u128(0x2a59116d_6c4f_45e0_a74f_707e3fef9258);

const RENDER_SUFFIX: &str = "#{e6327cad-dcec-4949-ae8a-991e976a79d2}";
const CAPTURE_SUFFIX: &str = "#{2eef81be-33fa-4800-9670-1cd474972c3f}";
const ROLES: [i32; 3] = [1, 0, 2]; // multimedia, console, communications
const E_RENDER: i32 = 0;
const E_CAPTURE: i32 = 1;

type SetFn = unsafe extern "system" fn(*mut c_void, u32, i32, i32, *mut c_void) -> HRESULT;
type GetFn = unsafe extern "system" fn(*mut c_void, u32, i32, i32, *mut *mut c_void) -> HRESULT;

struct Config {
    obj: windows::core::IUnknown,
    set: SetFn,
    get: GetFn,
}

/// Index of `SetPersistedDefaultAudioEndpoint` after the three IUnknown and three IInspectable
/// slots: nineteen volume-group / chat methods come first in both known layouts.
const SLOT_SET: usize = 6 + 19;

impl Config {
    fn open() -> Result<Self> {
        crate::wasapi::init_com();
        let hs = HSTRING::from("Windows.Media.Internal.AudioPolicyConfig");
        let factory: IInspectable = unsafe { RoGetActivationFactory(&hs) }
            .map_err(|e| HalError(format!("audio policy factory unavailable: {e}")))?;
        for iid in [IID_NEW, IID_OLD] {
            let mut raw = std::ptr::null_mut();
            if unsafe { factory.query(&iid, &mut raw) }.is_ok() && !raw.is_null() {
                let obj = unsafe { windows::core::IUnknown::from_raw(raw) };
                let vtbl = unsafe { *(raw as *const *const usize) };
                let (set, get) = unsafe {
                    (
                        std::mem::transmute::<usize, SetFn>(*vtbl.add(SLOT_SET)),
                        std::mem::transmute::<usize, GetFn>(*vtbl.add(SLOT_SET + 1)),
                    )
                };
                return Ok(Self { obj, set, get });
            }
        }
        Err(HalError("this Windows version does not expose per-app output routing".into()))
    }

    fn this(&self) -> *mut c_void {
        self.obj.as_raw()
    }
}

/// The id form the policy store wants for an MMDevice endpoint id.
fn policy_id(device_id: &str, flow: i32) -> String {
    let suffix = if flow == E_CAPTURE { CAPTURE_SUFFIX } else { RENDER_SUFFIX };
    format!("\\\\?\\SWD#MMDEVAPI#{device_id}{suffix}")
}

fn from_policy_id(s: &str) -> Option<String> {
    let rest = s.strip_prefix("\\\\?\\SWD#MMDEVAPI#")?;
    Some(rest.strip_suffix(RENDER_SUFFIX).or_else(|| rest.strip_suffix(CAPTURE_SUFFIX)).unwrap_or(rest).to_string())
}

/// Sends `pid`'s audio to `device_id` (an MMDevice render endpoint id), or back to the system
/// default when `None`. The choice is stored per executable and survives restarts of that app.
pub fn set_app_output(pid: u32, device_id: Option<&str>) -> Result<()> {
    set_app_device(pid, E_RENDER, device_id)
}

/// The output `pid` is explicitly routed to, or `None` for "system default".
pub fn get_app_output(pid: u32) -> Result<Option<String>> {
    get_app_device(pid, E_RENDER)
}

/// Makes `pid` record from `device_id` (an MMDevice capture endpoint id), or from the system
/// default when `None`: Windows' per-app "Input device" in the volume mixer.
pub fn set_app_input(pid: u32, device_id: Option<&str>) -> Result<()> {
    set_app_device(pid, E_CAPTURE, device_id)
}

/// The input `pid` is explicitly routed to, or `None` for "system default".
pub fn get_app_input(pid: u32) -> Result<Option<String>> {
    get_app_device(pid, E_CAPTURE)
}

fn set_app_device(pid: u32, flow: i32, device_id: Option<&str>) -> Result<()> {
    let cfg = Config::open()?;
    let hs = device_id.map(|d| HSTRING::from(policy_id(d, flow))).unwrap_or_default();
    for role in ROLES {
        let hr = unsafe { (cfg.set)(cfg.this(), pid, flow, role, std::mem::transmute_copy(&hs)) };
        hr.ok().map_err(|e| HalError(format!("set app device (role {role}): {e}")))?;
    }
    Ok(())
}

fn get_app_device(pid: u32, flow: i32) -> Result<Option<String>> {
    let cfg = Config::open()?;
    let mut raw: *mut c_void = std::ptr::null_mut();
    unsafe { (cfg.get)(cfg.this(), pid, flow, 1, &mut raw) }
        .ok()
        .map_err(|e| HalError(format!("get app device: {e}")))?;
    // The callee hands over an owned HSTRING (null = empty).
    let hs: HSTRING = unsafe { std::mem::transmute::<*mut c_void, HSTRING>(raw) };
    let text = hs.to_string();
    Ok(if text.is_empty() { None } else { from_policy_id(&text) })
}

/// `CPolicyConfigClient` and its `IPolicyConfig` interface (Windows 7 and later), the undocumented
/// way the Sound settings change the default device.
const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
/// `SetDefaultEndpoint` after the three IUnknown slots and ten format / period / property methods.
const SLOT_SET_DEFAULT: usize = 3 + 10;

type SetDefaultFn = unsafe extern "system" fn(*mut c_void, windows::core::PCWSTR, i32) -> HRESULT;

/// Makes `device_id` (an MMDevice render or capture endpoint id) the Windows default for every
/// role: what Settings > System > Sound does when you pick a device there.
pub fn set_default_device(device_id: &str) -> Result<()> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    crate::wasapi::init_com();
    let unk: windows::core::IUnknown = unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL) }
        .map_err(|e| HalError(format!("policy config unavailable: {e}")))?;
    let mut raw = std::ptr::null_mut();
    unsafe { unk.query(&IID_POLICY_CONFIG, &mut raw) }.ok().map_err(|e| HalError(format!("policy config interface: {e}")))?;
    // owns the reference query() added, released when it goes out of scope
    let obj = unsafe { windows::core::IUnknown::from_raw(raw) };
    let set: SetDefaultFn = unsafe { std::mem::transmute::<usize, SetDefaultFn>(*(*(raw as *const *const usize)).add(SLOT_SET_DEFAULT)) };
    let id = HSTRING::from(device_id);
    for role in 0..3 {
        unsafe { set(obj.as_raw(), windows::core::PCWSTR(id.as_ptr()), role) }
            .ok()
            .map_err(|e| HalError(format!("set default device (role {role}): {e}")))?;
    }
    Ok(())
}
