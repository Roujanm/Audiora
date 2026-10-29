//! Audiora desktop shell. Hosts the Svelte UI and makes sure the audio engine
//! (`audiora-svc`) is running. All audio work happens in the engine process;
//! this shell never touches audio.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{Manager, RunEvent};

const WS_PORT: u16 = 7879;
const TCP_PORT: u16 = 7878;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The engine process, only when this app started it. An engine that was
/// already running (autostart, another app instance) is left alone on exit.
#[derive(Default)]
struct OwnedEngine(Mutex<Option<Child>>);

fn reachable(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

/// Looks for `audiora-svc.exe` next to the app, then in the repository build
/// output (development).
fn engine_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let beside = exe.parent()?.join("audiora-svc.exe");
    if beside.exists() {
        return Some(beside);
    }
    // Development: the freshest engine build in the repository. Picking by
    // modification time avoids launching a stale release build next to a new debug one.
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("target");
    ["release", "debug"]
        .iter()
        .map(|p| target.join(p).join("audiora-svc.exe"))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, p)| p)
}

/// Async so the wait for the engine runs off the window's thread and never freezes the UI.
#[tauri::command]
async fn ensure_engine(state: tauri::State<'_, OwnedEngine>) -> Result<String, String> {
    if reachable(WS_PORT) {
        return Ok("running".into());
    }
    // The engine this app started may still be busy: starting up, or restarting itself onto a new
    // output. Wait for it instead of starting a second one next to it.
    let alive = state.0.lock().unwrap().as_mut().is_some_and(|c| matches!(c.try_wait(), Ok(None)));
    if alive {
        return wait_for_engine("running");
    }
    let path = engine_path().ok_or("audiora-svc.exe was not found next to the app")?;
    let mut cmd = Command::new(&path);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| format!("could not start {}: {e}", path.display()))?;
    *state.0.lock().unwrap() = Some(child);
    wait_for_engine("started")
}

/// Waits until the engine answers. A new engine first waits for an old one that is still shutting
/// down (up to its 5 s limit), so this allows more than that.
fn wait_for_engine(done: &str) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(14);
    while Instant::now() < deadline {
        if reachable(WS_PORT) {
            return Ok(done.into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("the audio engine did not start in time".into())
}

/// Opens Explorer with a recording selected. Only existing `.wav` files are accepted,
/// so the engine's reply (or anything else) cannot make the shell open arbitrary paths.
#[tauri::command]
fn reveal_in_folder(path: String) -> Result<(), String> {
    let file = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let is_wav = file.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    if !file.is_file() || !is_wav {
        return Err("only recordings can be shown".into());
    }
    // canonicalize returns the \\?\ form, which Explorer does not understand
    let shown = file.display().to_string();
    let shown = shown.strip_prefix(r"\\?\").unwrap_or(&shown).to_string();
    Command::new("explorer.exe")
        .arg(format!("/select,{shown}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Asks the engine to stop itself (so it finalizes recordings and saves the
/// session), then falls back to killing it.
fn stop_engine(mut child: Child) {
    if let Ok(mut s) = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], TCP_PORT)), Duration::from_millis(500)) {
        let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = s.write_all(b"{\"cmd\":\"shutdown\"}\n");
        let mut buf = [0u8; 64];
        let _ = s.read(&mut buf);
    }
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
}

/// Stops the engine (and forgets it) before an update replaces its files.
#[tauri::command]
fn stop_engine_now(state: tauri::State<'_, OwnedEngine>) {
    if let Some(child) = state.0.lock().unwrap().take() {
        stop_engine(child);
    } else if let Ok(mut s) = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], TCP_PORT)), Duration::from_millis(500)) {
        let _ = s.write_all(b"{\"cmd\":\"shutdown\"}
");
        std::thread::sleep(Duration::from_millis(800));
    }
}

fn main() {
    tauri::Builder::default()
        // single source of truth for "start with Windows": the HKCU Run entry the plugin manages;
        // it launches the app with --autostart so it opens minimized
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if std::env::args().any(|a| a == "--autostart") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            Ok(())
        })
        .manage(OwnedEngine::default())
        .invoke_handler(tauri::generate_handler![ensure_engine, reveal_in_folder, stop_engine_now])
        .build(tauri::generate_context!())
        .expect("error while building the Audiora app")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(child) = app.state::<OwnedEngine>().0.lock().unwrap().take() {
                    stop_engine(child);
                }
            }
        });
}
