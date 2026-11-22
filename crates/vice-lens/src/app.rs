//! Vice Lens as a desktop app: the research console in a native window
//! (the system's web view: WebKitGTK on Linux, WebView2 on Windows,
//! WKWebView on macOS), served by Lens inside the same process on a free
//! local port. Closing the window ends the app.

use std::net::TcpListener;
use std::path::PathBuf;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// Run the app until its window closes.
pub fn run(game: PathBuf) {
    #[cfg(target_os = "linux")]
    let gpu_run = choose_renderer();
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("cannot open a local port for Vice Lens: {e}");
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    std::thread::spawn(move || crate::lens::serve_on(listener, game));
    let base = format!("http://127.0.0.1:{port}/");
    // VICE_LENS_PROBE makes the page measure its frame rate and log it.
    let start = if std::env::var_os("VICE_LENS_PROBE").is_some() { format!("{base}#probe") } else { base.clone() };
    // The page asks for native things (a folder dialog) over IPC; answers
    // come back as scripts to run in it.
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window =
        WindowBuilder::new().with_title("Vice Lens").with_inner_size(LogicalSize::new(1440.0, 900.0)).with_min_inner_size(LogicalSize::new(900.0, 560.0)).build(&event_loop).expect("a window");
    let local = base.clone();
    let server = port;
    let builder = WebViewBuilder::new()
        .with_url(&start)
        .with_devtools(cfg!(debug_assertions))
        // Links that leave the app open in the system browser.
        .with_navigation_handler(move |url| {
            if url.starts_with(&local) || url.starts_with("about:") {
                return true;
            }
            open_external(&url);
            false
        })
        .with_ipc_handler(move |req| {
            let msg = req.body().clone();
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let answer = |id: &str, path: Option<PathBuf>| {
                    let path = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    let _ = proxy.send_event(format!("window.lensPicked && window.lensPicked({}, {})", crate::lens::quote(id), crate::lens::quote(&path)));
                };
                // The frame-rate probe closes the window when it is done.
                if msg == "quit" && std::env::var_os("VICE_LENS_PROBE").is_some() {
                    let _ = proxy.send_event("quit".into());
                    return;
                }
                // The page in the system browser, served from this process
                // while the window stays open.
                if msg == "open-browser" {
                    open_external(&format!("http://127.0.0.1:{server}/"));
                    return;
                }
                if let Some(id) = msg.strip_prefix("pick-folder:") {
                    answer(id, rfd::FileDialog::new().set_title("Choose the game folder (the one with levels.rpf)").pick_folder());
                } else if let Some(id) = msg.strip_prefix("pick-file:") {
                    answer(id, rfd::FileDialog::new().set_title("Choose a file").pick_file());
                } else if let Some((name, path)) = msg.strip_prefix("save:").and_then(|r| r.split_once('|')) {
                    // Ask where, then fetch the file from Lens and write it.
                    let Some(to) = rfd::FileDialog::new().set_file_name(name).save_file() else { return };
                    let note = match fetch(server, path).and_then(|b| std::fs::write(&to, b).map_err(|e| e.to_string())) {
                        Ok(()) => format!("Saved {}", to.display()),
                        Err(e) => format!("Could not save: {e}"),
                    };
                    let _ = proxy.send_event(format!("window.lensToast && window.lensToast({})", crate::lens::quote(&note)));
                }
            });
        });
    #[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().expect("a GTK box"))
    };
    #[cfg(not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd")))]
    let webview = builder.build(&window);
    let webview = match webview {
        Ok(w) => w,
        Err(e) => {
            log::error!("no web view: {e} (on Linux, install WebKitGTK 4.1)");
            return;
        }
    };
    #[cfg(target_os = "linux")]
    unthrottle(&webview);
    log::info!("Vice Lens is open (server on {base})");
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(js) if js != "quit" => {
                let _ = webview.evaluate_script(&js);
            }
            Event::UserEvent(_) | Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                #[cfg(target_os = "linux")]
                if let Some(m) = &gpu_run {
                    let _ = std::fs::remove_file(m);
                }
                *flow = ControlFlow::Exit
            }
            _ => {}
        }
    });
}

/// How WebKitGTK draws the page: through shared memory (the default),
/// which works everywhere, or on the GPU, chosen in Setup (the `renderer`
/// setting). The GPU path paces frames more evenly, but on NVIDIA under
/// Wayland it breaks the connection unless the driver's explicit sync is
/// off (the window dies with "Error 71 (Protocol error)"), so choosing it
/// turns that off too. A run on the GPU leaves a marker until its window
/// closes or it has lasted a minute; a marker found at start means the
/// last such run died early, and the setting goes back to shared memory.
/// `WEBKIT_DISABLE_DMABUF_RENDERER` set by hand decides instead.
/// Returns the marker when this run draws on the GPU.
#[cfg(target_os = "linux")]
fn choose_renderer() -> Option<PathBuf> {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some() {
        return None;
    }
    let setting = crate::dirs_config().join("renderer");
    let marker = crate::dirs_cache().join("gpu-renderer.running");
    let mut gpu = std::fs::read_to_string(&setting).is_ok_and(|s| s.trim() == "gpu");
    if gpu && marker.exists() {
        log::warn!("the last run drawing on the GPU ended early; back to drawing through shared memory (Setup can switch again)");
        let _ = std::fs::write(&setting, "compatible");
        gpu = false;
    }
    let _ = std::fs::remove_file(&marker);
    // SAFETY (both): no other thread runs yet.
    if !gpu {
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        return None;
    }
    if std::path::Path::new("/proc/driver/nvidia/version").exists() && std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none() {
        unsafe { std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1") };
    }
    let _ = std::fs::create_dir_all(marker.parent()?);
    let _ = std::fs::write(&marker, std::process::id().to_string());
    let m = marker.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let _ = std::fs::remove_file(m);
    });
    Some(marker)
}

/// Let WebKitGTK draw the page as often as the display refreshes. Its
/// `PreferPageRenderingUpdatesNear60FPS` feature, on by default,
/// holds rendering updates near 60 a second on any display: on a 144 or
/// 240 Hz screen, dragging the map or turning a model then lags the
/// pointer. The GTK binding has no call for features (WebKitGTK 2.42+),
/// so the C functions are called directly; on an older WebKitGTK the
/// feature list holds no such feature and nothing changes.
#[cfg(target_os = "linux")]
fn unthrottle(webview: &wry::WebView) {
    use std::ffi::{CStr, c_char, c_int, c_void};
    use webkit2gtk::glib::object::ObjectType;
    use wry::WebViewExtUnix;
    unsafe extern "C" {
        fn webkit_web_view_get_settings(view: *mut c_void) -> *mut c_void;
        fn webkit_settings_get_all_features() -> *mut c_void;
        fn webkit_feature_list_get_length(list: *mut c_void) -> usize;
        fn webkit_feature_list_get(list: *mut c_void, index: usize) -> *mut c_void;
        fn webkit_feature_get_identifier(feature: *mut c_void) -> *const c_char;
        fn webkit_settings_set_feature_enabled(settings: *mut c_void, feature: *mut c_void, enabled: c_int);
        fn webkit_feature_list_unref(list: *mut c_void);
    }
    let view = webview.webview();
    // SAFETY: the view is alive for the call; the feature list is
    // WebKit's own, held until unref; identifiers are static strings.
    let done = unsafe {
        let settings = webkit_web_view_get_settings(view.as_ptr() as *mut c_void);
        let list = webkit_settings_get_all_features();
        let mut done = false;
        if !settings.is_null() && !list.is_null() {
            for i in 0..webkit_feature_list_get_length(list) {
                let f = webkit_feature_list_get(list, i);
                let id = webkit_feature_get_identifier(f);
                if !id.is_null() && CStr::from_ptr(id).to_bytes() == b"PreferPageRenderingUpdatesNear60FPS" {
                    webkit_settings_set_feature_enabled(settings, f, 0);
                    done = true;
                }
            }
            webkit_feature_list_unref(list);
        }
        done
    };
    log::debug!("web view frame rate {}", if done { "follows the display" } else { "as WebKit sets it" });
}

/// Open a link in the system's browser.
fn open_external(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(url).spawn();
}

/// GET a path from the Lens server in this process.
fn fetch(port: u16, path: &str) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    s.read_to_end(&mut all).map_err(|e| e.to_string())?;
    let at = all.windows(4).position(|w| w == b"\r\n\r\n").ok_or("no reply")?;
    if !all.starts_with(b"HTTP/1.1 200") {
        return Err(String::from_utf8_lossy(&all[at + 4..]).into_owned());
    }
    Ok(all[at + 4..].to_vec())
}
