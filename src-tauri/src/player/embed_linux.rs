//! Where the video appears on Linux.
//!
//! * embedded: a `GtkGLArea` inside a `GtkOverlay` underneath Popcorn's
//!   transparent `WebKitWebView` in the main GTK3 window;
//! * fallback: a separate GTK3 window hosting a `GtkGLArea`, forwarding keys
//!   to mpv's bindings.
//!
//! All GTK/WebKitGTK and OpenGL calls run on the GTK main thread.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[cfg(debug_assertions)]
use gdk::prelude::DisplayExtManual;
use glib::prelude::*;
use gtk::prelude::*;
use tauri::{AppHandle, Manager};
use webkit2gtk::{WebView, WebViewExt};

use super::gl_linux::LinuxRenderer;
use super::mpv::Mpv;
use crate::error::{AppError, AppResult};

const MAIN_THREAD_TIMEOUT: Duration = Duration::from_secs(5);
const VIDEO_SLOT_NAME: &str = "popcorn-video-slot";
const FALLBACK_CSS: &str = "window { background-color: #000000; }";

/// Receives key presses in the fallback window: GDK keysym, characters,
/// GDK modifier mask. Called on the main thread, so it must not block.
pub type KeyHandler = Arc<dyn Fn(u16, &str, usize) + Send + Sync>;

fn is_main_thread() -> bool {
    glib::MainContext::default().is_owner()
}

fn on_main<T: Send + 'static>(
    app: &AppHandle,
    run: impl FnOnce() -> T + Send + 'static,
) -> AppResult<T> {
    if is_main_thread() {
        return Ok(run());
    }
    let (sender, receiver) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(run());
    })?;
    receiver
        .recv_timeout(MAIN_THREAD_TIMEOUT)
        .map_err(|_| AppError::Player("the window did not respond".into()))
}

/// Ensures the main window's `WebKitWebView` is stacked inside a `GtkOverlay`
/// over a persistent `video_slot` box. Safe to call repeatedly on the main thread.
fn ensure_overlay_host(webview: &WebView) -> AppResult<gtk::Box> {
    if let Some(parent) = webview.parent() {
        if let Ok(overlay) = parent.clone().downcast::<gtk::Overlay>() {
            if let Some(slot) = overlay
                .child()
                .and_then(|widget| widget.downcast::<gtk::Box>().ok())
                .filter(|b| b.widget_name() == VIDEO_SLOT_NAME)
            {
                return Ok(slot);
            }
        }
        let container = parent
            .downcast::<gtk::Container>()
            .map_err(|_| AppError::Player("the webview parent is not a GTK container".into()))?;

        let video_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        video_slot.set_widget_name(VIDEO_SLOT_NAME);
        video_slot.set_hexpand(true);
        video_slot.set_vexpand(true);

        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);

        container.remove(webview);
        overlay.add(&video_slot);
        webview.set_hexpand(true);
        webview.set_vexpand(true);
        overlay.add_overlay(webview);

        if let Ok(parent_box) = container.clone().downcast::<gtk::Box>() {
            parent_box.pack_start(&overlay, true, true, 0);
        } else {
            container.add(&overlay);
        }
        overlay.show_all();
        webview.grab_focus();
        return Ok(video_slot);
    }
    Err(AppError::Player("the webview is not in a window".into()))
}

/// Prepares the main window's `GtkOverlay` stack early during app startup so
/// playback does not have to reparent the `WebKitWebView` mid-flight.
pub fn setup_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.with_webview(|platform| {
        if let Err(err) = ensure_overlay_host(&platform.inner()) {
            log::warn!("could not prepare video overlay at startup: {err}");
        }
    }) {
        log::warn!("could not schedule video overlay setup: {error}");
    }
}

fn attach_gl_signals(gl_area: &gtk::GLArea, renderer: &Arc<LinuxRenderer>) {
    let weak_render = Arc::downgrade(renderer);
    gl_area.connect_render(move |area, _ctx| {
        if let Some(renderer) = weak_render.upgrade() {
            renderer.render(area);
        }
        glib::Propagation::Stop
    });

    gl_area.connect_resize(|area, _, _| {
        area.queue_render();
    });

    let weak_unrealize = Arc::downgrade(renderer);
    gl_area.connect_unrealize(move |area| {
        if let Some(renderer) = weak_unrealize.upgrade() {
            renderer.shutdown_on_main(Some(area));
        }
    });
}

#[derive(Clone)]
enum Kind {
    Embedded {
        video_slot: glib::SendWeakRef<gtk::Box>,
        webview: glib::SendWeakRef<WebView>,
        prev_bg: [f64; 4],
    },
    Window {
        window: glib::SendWeakRef<gtk::Window>,
        #[cfg(debug_assertions)]
        keys: KeyHandler,
    },
}

/// Handle to a Linux video surface. Clones refer to the same GTK widgets; the
/// session closes its handle once on teardown.
#[derive(Clone)]
pub struct VideoSurface {
    renderer: Arc<LinuxRenderer>,
    gl_area: glib::SendWeakRef<gtk::GLArea>,
    kind: Kind,
    windowed: Arc<AtomicBool>,
}

impl VideoSurface {
    pub fn is_embedded(&self) -> bool {
        matches!(self.kind, Kind::Embedded { .. })
    }

    /// Puts a `GtkGLArea` into the main window underneath the transparent `WebKitWebView`.
    pub fn embed(app: &AppHandle, mpv: &Arc<Mpv>) -> AppResult<Self> {
        #[cfg(debug_assertions)]
        if std::env::var_os("POPCORN_FORCE_FALLBACK").is_some() {
            return Err(AppError::Player("POPCORN_FORCE_FALLBACK is set".into()));
        }
        let webview_window = app
            .get_webview_window("main")
            .ok_or_else(|| AppError::Player("the main window is gone".into()))?;
        let (sender, receiver) = mpsc::channel();
        let mpv = Arc::clone(mpv);
        webview_window.with_webview(move |platform| {
            let webview = platform.inner();
            let result = Self::attach_embedded(&mpv, &webview);
            if sender.send(result).is_err() {
                log::warn!("embedded surface receiver dropped before reply");
            }
        })?;
        receiver
            .recv_timeout(MAIN_THREAD_TIMEOUT)
            .map_err(|_| AppError::Player("the window did not respond".into()))?
    }

    fn attach_embedded(mpv: &Arc<Mpv>, webview: &WebView) -> AppResult<Self> {
        let video_slot = ensure_overlay_host(webview)?;
        for child in video_slot.children() {
            video_slot.remove(&child);
        }

        let prev = webview.background_color();
        let prev_bg = [prev.red(), prev.green(), prev.blue(), prev.alpha()];
        webview.set_background_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));

        let gl_area = gtk::GLArea::new();
        gl_area.set_hexpand(true);
        gl_area.set_vexpand(true);
        gl_area.set_auto_render(false);
        gl_area.set_can_focus(false);
        video_slot.pack_start(&gl_area, true, true, 0);
        gl_area.show();
        if !gl_area.is_realized() {
            gl_area.realize();
        }

        let renderer = match LinuxRenderer::new(Arc::clone(mpv), &gl_area) {
            Ok(renderer) => renderer,
            Err(error) => {
                video_slot.remove(&gl_area);
                webview.set_background_color(&gdk::RGBA::new(
                    prev_bg[0], prev_bg[1], prev_bg[2], prev_bg[3],
                ));
                return Err(error);
            }
        };

        attach_gl_signals(&gl_area, &renderer);
        gl_area.queue_render();
        webview.grab_focus();

        Ok(Self {
            renderer,
            gl_area: glib::SendWeakRef::from(gl_area.downgrade()),
            kind: Kind::Embedded {
                video_slot: glib::SendWeakRef::from(video_slot.downgrade()),
                webview: glib::SendWeakRef::from(webview.downgrade()),
                prev_bg,
            },
            windowed: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Opens the fallback: a separate GTK3 window hosting its own `GtkGLArea`.
    pub fn open_window(app: &AppHandle, mpv: &Arc<Mpv>, keys: KeyHandler) -> AppResult<Self> {
        let mpv = Arc::clone(mpv);
        on_main(app, move || Self::create_fallback_window(&mpv, keys))?
    }

    fn create_fallback_window(mpv: &Arc<Mpv>, keys: KeyHandler) -> AppResult<Self> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("Popcorn");
        window.set_default_size(1280, 720);

        let provider = gtk::CssProvider::new();
        if provider.load_from_data(FALLBACK_CSS.as_bytes()).is_ok() {
            window
                .style_context()
                .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        let gl_area = gtk::GLArea::new();
        gl_area.set_hexpand(true);
        gl_area.set_vexpand(true);
        gl_area.set_auto_render(false);
        gl_area.set_can_focus(true);
        window.add(&gl_area);
        window.show_all();
        if !gl_area.is_realized() {
            gl_area.realize();
        }

        let renderer = match LinuxRenderer::new(Arc::clone(mpv), &gl_area) {
            Ok(renderer) => renderer,
            Err(error) => {
                window.close();
                return Err(error);
            }
        };

        attach_gl_signals(&gl_area, &renderer);

        let windowed = Arc::new(AtomicBool::new(false));
        let fs_flag = Arc::clone(&windowed);
        window.connect_window_state_event(move |_, event| {
            let fs = event
                .new_window_state()
                .contains(gdk::WindowState::FULLSCREEN);
            fs_flag.store(!fs, Ordering::SeqCst);
            glib::Propagation::Proceed
        });

        let keys_press = Arc::clone(&keys);
        window.connect_key_press_event(move |_, event| {
            let keyval = event.keyval();
            let code = u16::try_from(*keyval).unwrap_or(0);
            let chars = keyval
                .to_unicode()
                .map(|c| c.to_string())
                .unwrap_or_default();
            let modifiers = event.state().bits() as usize;
            keys_press(code, &chars, modifiers);
            glib::Propagation::Stop
        });

        let keys_close = Arc::clone(&keys);
        window.connect_delete_event(move |_, _| {
            keys_close(0xff1b, "\x1b", 0);
            glib::Propagation::Stop
        });

        window.fullscreen();
        window.present();
        gl_area.grab_focus();
        gl_area.queue_render();

        Ok(Self {
            renderer,
            gl_area: glib::SendWeakRef::from(gl_area.downgrade()),
            kind: Kind::Window {
                window: glib::SendWeakRef::from(window.downgrade()),
                #[cfg(debug_assertions)]
                keys,
            },
            windowed,
        })
    }

    pub fn set_scale(&self, app: &AppHandle, _scale: f64) {
        let area_weak = self.gl_area.clone();
        if let Err(error) = on_main(app, move || {
            if let Some(area) = area_weak.upgrade() {
                area.queue_render();
            }
        }) {
            log::warn!("could not rescale the video: {error}");
        }
    }

    pub fn is_window_fullscreen(&self) -> bool {
        !self.windowed.load(Ordering::SeqCst)
    }

    pub fn toggle_window_fullscreen(&self, app: &AppHandle) -> AppResult<bool> {
        let windowed = !self.windowed.load(Ordering::SeqCst);
        self.set_window_fullscreen(app, !windowed)?;
        Ok(!windowed)
    }

    pub fn set_window_fullscreen(&self, app: &AppHandle, fullscreen: bool) -> AppResult<()> {
        let Kind::Window { window, .. } = &self.kind else {
            return Ok(());
        };
        self.windowed.store(!fullscreen, Ordering::SeqCst);
        let window_weak = window.clone();
        let state = Arc::clone(&self.windowed);
        on_main(app, move || {
            if let Some(window) = window_weak.upgrade() {
                state.store(!fullscreen, Ordering::SeqCst);
                if fullscreen {
                    window.fullscreen();
                } else {
                    window.unfullscreen();
                }
            }
        })
    }

    /// Destroys the mpv render context while `GtkGLArea` is current, then removes
    /// the surface and restores the webview background (or closes the fallback window).
    pub fn close(self, app: &AppHandle) {
        let renderer = self.renderer;
        let gl_area = self.gl_area;
        let kind = self.kind;
        if let Err(error) = on_main(app, move || {
            let area = gl_area.upgrade();
            renderer.shutdown_on_main(area.as_ref());
            match kind {
                Kind::Embedded {
                    video_slot,
                    webview,
                    prev_bg,
                } => {
                    if let (Some(slot), Some(area)) = (video_slot.upgrade(), area) {
                        slot.remove(&area);
                    }
                    if let Some(wv) = webview.upgrade() {
                        wv.set_background_color(&gdk::RGBA::new(
                            prev_bg[0], prev_bg[1], prev_bg[2], prev_bg[3],
                        ));
                    }
                }
                Kind::Window { window, .. } => {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                }
            }
        }) {
            log::warn!("could not remove the Linux video surface: {error}");
        }
    }

    #[cfg(debug_assertions)]
    pub fn press_key(&self, app: &AppHandle, key_code: u16, characters: &'static std::ffi::CStr) {
        let Kind::Window { keys, .. } = &self.kind else {
            return;
        };
        let keys = Arc::clone(keys);
        let text = characters.to_string_lossy().into_owned();
        if let Err(error) = on_main(app, move || keys(key_code, &text, 0)) {
            log::warn!("could not send a key: {error}");
        }
    }

    #[cfg(debug_assertions)]
    pub fn describe(&self, app: &AppHandle) -> String {
        let stats = format!(
            "draws={} render_errors={} backend={}",
            self.renderer.draws(),
            self.renderer.errors(),
            self.renderer.backend_name()
        );
        let area_weak = self.gl_area.clone();
        let embedded = self.is_embedded();
        let shape = on_main(app, move || {
            area_weak.upgrade().map_or_else(
                || "gl_area=gone".to_owned(),
                |area| {
                    let display = area.display().backend();
                    let display_name = if display.is_wayland() {
                        "wayland"
                    } else if display.is_x11() {
                        "x11"
                    } else {
                        "other"
                    };
                    format!(
                        "embedded={embedded} display={display_name} realized={} mapped={} alloc={}x{} scale={}",
                        area.is_realized(),
                        area.is_mapped(),
                        area.allocated_width(),
                        area.allocated_height(),
                        area.scale_factor()
                    )
                },
            )
        });
        format!("{stats} {}", shape.unwrap_or_default())
    }
}

// ---- Keeping the screen awake ---------------------------------------------

static ACTIVE_WAKE_LOCKS: AtomicUsize = AtomicUsize::new(0);

/// Stops the display from sleeping while alive using the session D-Bus
/// `org.freedesktop.ScreenSaver` interface (safe off the main thread).
pub struct WakeLock {
    cookie: Option<u32>,
}

impl WakeLock {
    pub fn new() -> Self {
        let cookie = Self::inhibit();
        if cookie.is_some() {
            ACTIVE_WAKE_LOCKS.fetch_add(1, Ordering::Release);
        }
        Self { cookie }
    }

    #[cfg(debug_assertions)]
    pub fn is_active() -> bool {
        ACTIVE_WAKE_LOCKS.load(Ordering::Acquire) > 0
    }

    fn inhibit() -> Option<u32> {
        let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>).ok()?;
        let reply = bus
            .call_sync(
                Some("org.freedesktop.ScreenSaver"),
                "/org/freedesktop/ScreenSaver",
                "org.freedesktop.ScreenSaver",
                "Inhibit",
                Some(&("Popcorn", "Popcorn is playing a video").to_variant()),
                Some(glib::VariantTy::new("(u)").ok()?),
                gio::DBusCallFlags::NONE,
                1500,
                None::<&gio::Cancellable>,
            )
            .ok()?;
        (reply.n_children() > 0)
            .then(|| reply.child_value(0))
            .and_then(|v| v.get::<u32>())
    }
}

impl Drop for WakeLock {
    fn drop(&mut self) {
        let Some(cookie) = self.cookie.take() else {
            return;
        };
        ACTIVE_WAKE_LOCKS.fetch_sub(1, Ordering::Release);
        let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
            return;
        };
        let _ = bus.call_sync(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver",
            "UnInhibit",
            Some(&(cookie,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1500,
            None::<&gio::Cancellable>,
        );
    }
}
