//! mpv's OpenGL render API inside GTK3's `GtkGLArea`.
//!
//! GTK owns the `GdkGLContext`. The mpv render context is created on the GTK
//! main thread only after `GtkGLArea` is realized and current, woken via
//! `glib::idle_add_full`, drawn strictly inside `GtkGLArea::connect_render`,
//! and freed while `GtkGLArea`'s context is still current before the widget is
//! removed and before the `Mpv` core drops.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use gdk::prelude::DisplayExtManual;
use glib::translate::ToGlibPtr;
use gtk::prelude::*;

use super::ffi;
use super::mpv::Mpv;
use crate::error::{AppError, AppResult};
use crate::state::lock;

const GL_COLOR_BUFFER_BIT: c_uint = 0x0000_4000;
const GL_DRAW_FRAMEBUFFER_BINDING: c_uint = 0x8CA6;
#[cfg(test)]
const GL_READ_FRAMEBUFFER: c_uint = 0x8CA8;
#[cfg(test)]
const GL_READ_FRAMEBUFFER_BINDING: c_uint = 0x8CAA;
#[cfg(test)]
const GL_RGBA: c_uint = 0x1908;
#[cfg(test)]
const GL_UNSIGNED_BYTE: c_uint = 0x1401;

type GetCurrentCtxFn = unsafe extern "C" fn() -> *mut c_void;
type GetProcAddrFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type GetNativeDisplayFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

struct GlProcs {
    backend_name: &'static str,
    get_proc: GetProcAddrFn,
    fallback_libs: Vec<usize>,
    get_integerv: unsafe extern "C" fn(c_uint, *mut c_int),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(c_uint),
    #[cfg(test)]
    bind_framebuffer: unsafe extern "C" fn(c_uint, c_uint),
    #[cfg(test)]
    read_pixels: unsafe extern "C" fn(c_int, c_int, c_int, c_int, c_uint, c_uint, *mut c_void),
}

unsafe fn resolve_gl_loader() -> Result<(&'static str, GetProcAddrFn, Vec<usize>), String> {
    let flags = ffi::RTLD_LAZY | ffi::RTLD_LOCAL;
    let egl = ffi::dlopen(c"libEGL.so.1".as_ptr(), flags);
    if !egl.is_null() {
        let cur = ffi::dlsym(egl, c"eglGetCurrentContext".as_ptr());
        let proc_fn = ffi::dlsym(egl, c"eglGetProcAddress".as_ptr());
        if !cur.is_null() && !proc_fn.is_null() {
            let cur_fn: GetCurrentCtxFn = std::mem::transmute(cur);
            if !cur_fn().is_null() {
                let mut libs = vec![egl as usize];
                for lib_name in [c"libGLESv2.so.2", c"libGL.so.1"] {
                    let lib = ffi::dlopen(lib_name.as_ptr(), flags);
                    if !lib.is_null() {
                        libs.push(lib as usize);
                    }
                }
                return Ok((
                    "EGL",
                    std::mem::transmute::<*mut c_void, GetProcAddrFn>(proc_fn),
                    libs,
                ));
            }
        }
    }

    let gl = ffi::dlopen(c"libGL.so.1".as_ptr(), flags);
    if !gl.is_null() {
        let cur = ffi::dlsym(gl, c"glXGetCurrentContext".as_ptr());
        let proc_fn = ffi::dlsym(gl, c"glXGetProcAddressARB".as_ptr());
        if !cur.is_null() && !proc_fn.is_null() {
            let cur_fn: GetCurrentCtxFn = std::mem::transmute(cur);
            if !cur_fn().is_null() {
                return Ok((
                    "GLX",
                    std::mem::transmute::<*mut c_void, GetProcAddrFn>(proc_fn),
                    vec![gl as usize],
                ));
            }
        }
    }

    Err("neither EGL nor GLX has a current OpenGL context".into())
}

impl GlProcs {
    /// # Safety
    /// A GTK OpenGL context must be current on this thread.
    unsafe fn load() -> Result<Self, String> {
        let (backend_name, get_proc, fallback_libs) = resolve_gl_loader()?;
        let lookup = |name: &CStr| -> Result<*mut c_void, String> {
            let found = get_proc(name.as_ptr());
            if !found.is_null() {
                return Ok(found);
            }
            for &lib in &fallback_libs {
                let sym = ffi::dlsym(lib as *mut c_void, name.as_ptr());
                if !sym.is_null() {
                    return Ok(sym);
                }
            }
            let sym = ffi::dlsym(ffi::RTLD_DEFAULT, name.as_ptr());
            if !sym.is_null() {
                return Ok(sym);
            }
            Err(format!("missing OpenGL symbol {}", name.to_string_lossy()))
        };

        let get_integerv = std::mem::transmute_copy(&lookup(c"glGetIntegerv")?);
        let clear_color = std::mem::transmute_copy(&lookup(c"glClearColor")?);
        let clear = std::mem::transmute_copy(&lookup(c"glClear")?);
        #[cfg(test)]
        let bind_framebuffer = std::mem::transmute_copy(&lookup(c"glBindFramebuffer")?);
        #[cfg(test)]
        let read_pixels = std::mem::transmute_copy(&lookup(c"glReadPixels")?);

        Ok(Self {
            backend_name,
            get_proc,
            fallback_libs,
            get_integerv,
            clear_color,
            clear,
            #[cfg(test)]
            bind_framebuffer,
            #[cfg(test)]
            read_pixels,
        })
    }

    /// # Safety
    /// `name` must be null or a valid NUL-terminated OpenGL symbol name.
    unsafe fn get_proc_address(&self, name: *const c_char) -> *mut c_void {
        if name.is_null() {
            return ptr::null_mut();
        }
        let found = (self.get_proc)(name);
        if !found.is_null() {
            return found;
        }
        for &lib in &self.fallback_libs {
            let sym = ffi::dlsym(lib as *mut c_void, name);
            if !sym.is_null() {
                return sym;
            }
        }
        ffi::dlsym(ffi::RTLD_DEFAULT, name)
    }
}

unsafe extern "C" fn mpv_get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    match ctx.cast::<GlProcs>().as_ref() {
        Some(procs) => procs.get_proc_address(name),
        None => ptr::null_mut(),
    }
}

struct UpdateBridge {
    queued: AtomicBool,
    stopped: AtomicBool,
    area: glib::SendWeakRef<gtk::GLArea>,
}

struct UpdateCtx {
    bridge: Arc<UpdateBridge>,
    renderer_weak: Weak<LinuxRenderer>,
}

unsafe extern "C" fn on_mpv_render_update(ctx: *mut c_void) {
    // SAFETY: `ctx` points to the `UpdateCtx` owned by `LinuxRenderer`, which
    // unregisters this callback and calls `mpv_render_context_free` before
    // dropping `UpdateCtx`. mpv forbids calling back into mpv from here, so we
    // only flip an atomic and schedule an async idle callback on GLib's main
    // loop.
    let Some(ctx) = ctx.cast::<UpdateCtx>().as_ref() else {
        return;
    };
    let bridge = &ctx.bridge;
    if bridge.stopped.load(Ordering::Acquire) {
        return;
    }
    if bridge.queued.swap(true, Ordering::AcqRel) {
        return;
    }
    let bridge_clone = Arc::clone(bridge);
    let renderer_weak = ctx.renderer_weak.clone();
    let mut once = Some(move || {
        bridge_clone.queued.store(false, Ordering::Release);
        if bridge_clone.stopped.load(Ordering::Acquire) {
            return;
        }
        if let Some(area) = bridge_clone.area.upgrade() {
            if area.is_mapped() {
                area.queue_render();
            } else if area.is_realized() {
                if let Some(renderer) = renderer_weak.upgrade() {
                    renderer.skip_frame_on_main(&area);
                }
            }
        }
    });
    // Use HIGH_IDLE (100, above GDK_PRIORITY_REDRAW at 120) and never invoke
    // synchronously from the callback.
    glib::idle_add_full(glib::Priority::HIGH_IDLE, move || {
        if let Some(run) = once.take() {
            run();
        }
        glib::ControlFlow::Break
    });
}

fn native_display_param(area: &gtk::GLArea) -> Option<(c_int, *mut c_void)> {
    let display = area.display();
    let backend = display.backend();
    let raw_display: *mut gdk::ffi::GdkDisplay = display.to_glib_none().0;
    let (param_kind, symbol_name) = if backend.is_x11() {
        (
            ffi::RENDER_PARAM_X11_DISPLAY,
            c"gdk_x11_display_get_xdisplay",
        )
    } else if backend.is_wayland() {
        (
            ffi::RENDER_PARAM_WL_DISPLAY,
            c"gdk_wayland_display_get_wl_display",
        )
    } else {
        return None;
    };
    // SAFETY: `symbol_name` is a GDK3 getter taking `*mut GdkDisplay` and
    // returning the native display handle, which stays valid for the display's
    // lifetime.
    unsafe {
        let sym = ffi::dlsym(ffi::RTLD_DEFAULT, symbol_name.as_ptr());
        if sym.is_null() {
            return None;
        }
        let get_display: GetNativeDisplayFn = std::mem::transmute(sym);
        let native = get_display(raw_display.cast());
        (!native.is_null()).then_some((param_kind, native))
    }
}

fn param(kind: c_int, data: *mut c_void) -> ffi::RenderParam {
    ffi::RenderParam { kind, data }
}

pub struct LinuxRenderer {
    mpv: Arc<Mpv>,
    procs: Box<GlProcs>,
    /// Null once shut down. Only dereferenced on the GTK main thread.
    render: AtomicPtr<ffi::RenderHandle>,
    bridge: Arc<UpdateBridge>,
    update_ctx: Mutex<Option<Box<UpdateCtx>>>,
    draws: AtomicU64,
    errors: AtomicU64,
    #[cfg(test)]
    sample_next: AtomicBool,
    #[cfg(test)]
    last_lit_pixels: AtomicU64,
    #[cfg(test)]
    last_total_pixels: AtomicU64,
}

impl LinuxRenderer {
    /// Creates the mpv OpenGL render context on a realized, current `GtkGLArea`.
    /// Must run on the GTK main thread.
    pub fn new(mpv: Arc<Mpv>, area: &gtk::GLArea) -> AppResult<Arc<Self>> {
        if !glib::MainContext::default().is_owner() {
            return Err(AppError::Player(
                "video renderer must be created on the main thread".into(),
            ));
        }
        area.make_current();
        if let Some(error) = area.error() {
            return Err(AppError::Player(format!(
                "could not initialize OpenGL surface ({error})"
            )));
        }
        // SAFETY: `area.make_current()` succeeded with no error on the GTK main thread.
        let procs = Box::new(unsafe { GlProcs::load() }.map_err(AppError::Player)?);
        log::info!(
            "initialized GtkGLArea OpenGL backend ({})",
            procs.backend_name
        );

        let bridge = Arc::new(UpdateBridge {
            queued: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            area: glib::SendWeakRef::from(area.downgrade()),
        });

        let renderer = Arc::new(Self {
            mpv,
            procs,
            render: AtomicPtr::new(ptr::null_mut()),
            bridge,
            update_ctx: Mutex::new(None),
            draws: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            #[cfg(test)]
            sample_next: AtomicBool::new(false),
            #[cfg(test)]
            last_lit_pixels: AtomicU64::new(0),
            #[cfg(test)]
            last_total_pixels: AtomicU64::new(0),
        });

        let mut init = ffi::OpenGlInitParams {
            get_proc_address: mpv_get_proc_address,
            get_proc_address_ctx: ptr::from_ref(renderer.procs.as_ref()).cast_mut().cast(),
        };
        let mut params = Vec::with_capacity(4);
        params.push(param(
            ffi::RENDER_PARAM_API_TYPE,
            ffi::RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
        ));
        params.push(param(
            ffi::RENDER_PARAM_OPENGL_INIT_PARAMS,
            ptr::from_mut(&mut init).cast(),
        ));
        if let Some((kind, disp_ptr)) = native_display_param(area) {
            params.push(param(kind, disp_ptr));
        }
        params.push(param(ffi::RENDER_PARAM_INVALID, ptr::null_mut()));

        let mut render_ctx = ptr::null_mut();
        let api = renderer.mpv.api();
        // SAFETY: `area` has a current GL context, `mpv` is live, and `params`
        // is terminated by `RENDER_PARAM_INVALID` and outlives the call.
        let code = unsafe {
            (api.mpv_render_context_create)(
                &mut render_ctx,
                renderer.mpv.raw(),
                params.as_mut_ptr(),
            )
        };
        renderer.mpv.check(code)?;
        renderer.render.store(render_ctx, Ordering::Release);

        let update_ctx = Box::new(UpdateCtx {
            bridge: Arc::clone(&renderer.bridge),
            renderer_weak: Arc::downgrade(&renderer),
        });
        let ctx_ptr = ptr::from_ref(update_ctx.as_ref()).cast_mut().cast();
        *lock(&renderer.update_ctx) = Some(update_ctx);

        // SAFETY: `update_ctx` stays owned by `renderer` until after
        // `mpv_render_context_free` runs in `shutdown_on_main`.
        unsafe {
            (api.mpv_render_context_set_update_callback)(
                render_ctx,
                Some(on_mpv_render_update),
                ctx_ptr,
            );
        }
        Ok(renderer)
    }

    #[cfg(any(test, debug_assertions))]
    pub fn draws(&self) -> u64 {
        self.draws.load(Ordering::Relaxed)
    }

    #[cfg(any(test, debug_assertions))]
    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    #[cfg(any(test, debug_assertions))]
    pub fn backend_name(&self) -> &'static str {
        self.procs.backend_name
    }

    #[cfg(test)]
    pub fn request_sample(&self) {
        self.sample_next.store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub fn last_lit_pixels(&self) -> (u64, u64) {
        (
            self.last_lit_pixels.load(Ordering::Relaxed),
            self.last_total_pixels.load(Ordering::Relaxed),
        )
    }

    /// Renders one frame into `GtkGLArea`'s current bound framebuffer.
    /// Must only be called from `GtkGLArea::connect_render` on the main thread.
    pub fn render(&self, area: &gtk::GLArea) {
        if self.bridge.stopped.load(Ordering::Acquire) {
            return;
        }
        let render = self.render.load(Ordering::Acquire);
        if render.is_null() {
            return;
        }
        let scale = area.scale_factor();
        let w = (area.allocated_width() * scale).max(1);
        let h = (area.allocated_height() * scale).max(1);

        let mut fbo: c_int = 0;
        // SAFETY: GTK has made `area`'s context current and bound its draw FBO.
        unsafe { (self.procs.get_integerv)(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo) };
        let mut target = ffi::OpenGlFbo {
            fbo,
            w,
            h,
            internal_format: 0,
        };
        let mut flip: c_int = 1;
        let mut block: c_int = 0;
        let mut params = [
            param(
                ffi::RENDER_PARAM_OPENGL_FBO,
                ptr::from_mut(&mut target).cast(),
            ),
            param(ffi::RENDER_PARAM_FLIP_Y, ptr::from_mut(&mut flip).cast()),
            param(
                ffi::RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                ptr::from_mut(&mut block).cast(),
            ),
            param(ffi::RENDER_PARAM_INVALID, ptr::null_mut()),
        ];

        // SAFETY: `render` is live on the main thread, the GL context is
        // current, and every parameter outlives the call.
        let code =
            unsafe { (self.mpv.api().mpv_render_context_render)(render, params.as_mut_ptr()) };
        if code >= 0 {
            self.draws.fetch_add(1, Ordering::Relaxed);
            #[cfg(test)]
            if self.sample_next.swap(false, Ordering::AcqRel) && w >= 64 && h >= 64 {
                self.sample_framebuffer(fbo, w, h);
            }
        } else {
            self.errors.fetch_add(1, Ordering::Relaxed);
            // SAFETY: GL context is current inside `connect_render`.
            unsafe {
                (self.procs.clear_color)(0.0, 0.0, 0.0, 1.0);
                (self.procs.clear)(GL_COLOR_BUFFER_BIT);
            }
        }
    }

    #[cfg(test)]
    fn sample_framebuffer(&self, fbo: c_int, w: c_int, h: c_int) {
        let mut prev_read: c_int = 0;
        let mut buf = vec![0_u8; (w * h * 4) as usize];
        // SAFETY: called inside `connect_render` with `fbo` complete at `w`x`h`.
        unsafe {
            (self.procs.get_integerv)(GL_READ_FRAMEBUFFER_BINDING, &mut prev_read);
            (self.procs.bind_framebuffer)(GL_READ_FRAMEBUFFER, fbo as c_uint);
            (self.procs.read_pixels)(
                0,
                0,
                w,
                h,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                buf.as_mut_ptr().cast(),
            );
            (self.procs.bind_framebuffer)(GL_READ_FRAMEBUFFER, prev_read as c_uint);
        }
        let lit = buf
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 24 || p[1] > 24 || p[2] > 24)
            .count() as u64;
        self.last_lit_pixels.store(lit, Ordering::Relaxed);
        self.last_total_pixels
            .store((w * h) as u64, Ordering::Relaxed);
    }

    fn skip_frame_on_main(&self, area: &gtk::GLArea) {
        if self.bridge.stopped.load(Ordering::Acquire) {
            return;
        }
        let render = self.render.load(Ordering::Acquire);
        if render.is_null() || !area.is_realized() {
            return;
        }
        area.make_current();
        if area.error().is_some() {
            return;
        }
        let mut skip: c_int = 1;
        let mut params = [
            param(
                ffi::RENDER_PARAM_SKIP_RENDERING,
                ptr::from_mut(&mut skip).cast(),
            ),
            param(ffi::RENDER_PARAM_INVALID, ptr::null_mut()),
        ];
        // SAFETY: GL context is current on the main thread and `render` is live.
        unsafe {
            (self.mpv.api().mpv_render_context_render)(render, params.as_mut_ptr());
        }
    }

    /// Frees the mpv render context while `GtkGLArea`'s context is still valid.
    /// Must run on the GTK main thread. Idempotent.
    pub fn shutdown_on_main(&self, area: Option<&gtk::GLArea>) {
        self.bridge.stopped.store(true, Ordering::Release);
        let render = self.render.swap(ptr::null_mut(), Ordering::AcqRel);
        if render.is_null() {
            return;
        }
        if let Some(area) = area.filter(|a| a.is_realized()) {
            area.make_current();
        }
        let api = self.mpv.api();
        // SAFETY: runs on the GTK main thread (no concurrent render is active),
        // clears the update callback first, and frees `render` once.
        unsafe {
            (api.mpv_render_context_set_update_callback)(render, None, ptr::null_mut());
            (api.mpv_render_context_free)(render);
        }
        *lock(&self.update_ctx) = None;
    }
}

impl Drop for LinuxRenderer {
    fn drop(&mut self) {
        if !self.render.load(Ordering::Acquire).is_null() {
            log::warn!("LinuxRenderer dropped before main-thread shutdown");
            self.shutdown_on_main(None);
        }
    }
}
