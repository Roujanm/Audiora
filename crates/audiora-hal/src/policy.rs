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
const ROLES: [i32; 2] = [1, 0]; // multimedia, console
const E_RENDER: i32 = 0;

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

/// The id form the policy store wants for an MMDevice render endpoint id.
fn policy_id(device_id: &str) -> String {
    format!("\\\\?\\SWD#MMDEVAPI#{device_id}{RENDER_SUFFIX}")
}

fn from_policy_id(s: &str) -> Option<String> {
    let rest = s.strip_prefix("\\\\?\\SWD#MMDEVAPI#")?;
    Some(rest.strip_suffix(RENDER_SUFFIX).unwrap_or(rest).to_string())
}

/// Sends `pid`'s audio to `device_id` (an MMDevice render endpoint id), or back to the system
/// default when `None`. The choice is stored per executable and survives restarts of that app.
pub fn set_app_output(pid: u32, device_id: Option<&str>) -> Result<()> {
    let cfg = Config::open()?;
    let hs = device_id.map(|d| HSTRING::from(policy_id(d))).unwrap_or_default();
    for role in ROLES {
        let hr = unsafe { (cfg.set)(cfg.this(), pid, E_RENDER, role, std::mem::transmute_copy(&hs)) };
        hr.ok().map_err(|e| HalError(format!("set app output (role {role}): {e}")))?;
    }
    Ok(())
}

/// The endpoint `pid` is explicitly routed to, or `None` for "system default".
pub fn get_app_output(pid: u32) -> Result<Option<String>> {
    let cfg = Config::open()?;
    let mut raw: *mut c_void = std::ptr::null_mut();
    unsafe { (cfg.get)(cfg.this(), pid, E_RENDER, 1, &mut raw) }
        .ok()
        .map_err(|e| HalError(format!("get app output: {e}")))?;
    // The callee hands over an owned HSTRING (null = empty).
    let hs: HSTRING = unsafe { std::mem::transmute::<*mut c_void, HSTRING>(raw) };
    let text = hs.to_string();
    Ok(if text.is_empty() { None } else { from_policy_id(&text) })
}
