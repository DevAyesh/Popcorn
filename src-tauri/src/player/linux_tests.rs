//! Live Linux player tests: `LinuxRenderer` + `GtkGLArea` + real `libmpv.so.2`.
//!
//! Run ignored live checks on a desktop session with:
//! ```sh
//! DISPLAY=:0.0 POPCORN_TEST_MEDIA=/tmp/popcorn-test-media/clip_h264.mkv \
//!   POPCORN_TEST_MEDIA_SW=/tmp/popcorn-test-media/clip_hevc.mkv \
//!   cargo test linux_tests -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use gtk::prelude::*;

use super::gl_linux::LinuxRenderer;
use super::mpv::{Event, EventReader, Mpv};
use super::{ffi, model, Player};

struct LiveSurface {
    window: gtk::Window,
    gl_area: gtk::GLArea,
    renderer: Arc<LinuxRenderer>,
}

impl LiveSurface {
    fn open(mpv: &Arc<Mpv>) -> Self {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("Popcorn Linux Test");
        window.set_default_size(640, 360);

        let gl_area = gtk::GLArea::new();
        gl_area.set_hexpand(true);
        gl_area.set_vexpand(true);
        gl_area.set_auto_render(false);
        window.add(&gl_area);
        window.show_all();
        if !gl_area.is_realized() {
            gl_area.realize();
        }

        let renderer = LinuxRenderer::new(Arc::clone(mpv), &gl_area).unwrap();
        let weak_render = Arc::downgrade(&renderer);
        gl_area.connect_render(move |area, _ctx| {
            if let Some(renderer) = weak_render.upgrade() {
                renderer.render(area);
            }
            glib::Propagation::Stop
        });
        let weak_unrealize = Arc::downgrade(&renderer);
        gl_area.connect_unrealize(move |area| {
            if let Some(renderer) = weak_unrealize.upgrade() {
                renderer.shutdown_on_main(Some(area));
            }
        });

        Self {
            window,
            gl_area,
            renderer,
        }
    }

    fn close(self) {
        self.renderer.shutdown_on_main(Some(&self.gl_area));
        self.window.remove(&self.gl_area);
        self.window.close();
        pump_ms(50);
    }
}

fn pump_ms(ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline {
        let mut count = 0;
        while gtk::events_pending() && count < 64 && Instant::now() < deadline {
            gtk::main_iteration_do(false);
            count += 1;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for(reader: &mut EventReader, what: &str, wanted: impl Fn(&Event) -> bool) -> Event {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        pump_ms(20);
        let event = reader.wait(0.05);
        if wanted(&event) {
            return event;
        }
        assert!(
            !matches!(event, Event::Failed(_)),
            "failed while waiting for {what}: {event:?}"
        );
    }
    panic!("timed out waiting for {what}");
}

fn wait_for_failure(reader: &mut EventReader) -> Event {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        pump_ms(20);
        let event = reader.wait(0.05);
        if matches!(event, Event::Failed(_)) {
            return event;
        }
    }
    panic!("no failure was reported");
}

fn eventually(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        pump_ms(40);
    }
}

fn tracks(mpv: &Mpv) -> model::Tracks {
    model::parse_tracks(&mpv.get_string("track-list").unwrap()).unwrap()
}

fn selected(list: &[model::Track]) -> Option<i64> {
    list.iter().find(|t| t.selected).map(|t| t.id)
}

fn verify_lit_frames(surface: &LiveSurface) {
    eventually("rendered video frames", || surface.renderer.draws() >= 15);
    surface.renderer.request_sample();
    surface.gl_area.queue_render();
    eventually("sampled frame", || surface.renderer.last_lit_pixels().1 > 0);
    let (lit, total) = surface.renderer.last_lit_pixels();
    println!(
        "rendered {} frames ({} errors) on {}, lit_pixels={lit}/{total}",
        surface.renderer.draws(),
        surface.renderer.errors(),
        surface.renderer.backend_name()
    );
    assert_eq!(surface.renderer.errors(), 0);
    assert!(total > 0 && lit * 4 > total, "expected >25% lit pixels");
}

fn run_h264_checks(media: &Path) {
    let path = media.to_str().unwrap();
    let mpv = Mpv::new(&[("ao", "null")]).unwrap();
    let surface = LiveSurface::open(&mpv);
    let mut reader = mpv.event_reader().unwrap();
    assert!(mpv.event_reader().is_none());

    mpv.load(path, Some(70.0)).unwrap();
    wait_for(&mut reader, "playback", |e| *e == Event::PlaybackRestart);
    let position = mpv.get_f64("time-pos").unwrap();
    println!("h264 started at {position:.2}s");
    assert!((69.0..74.0).contains(&position));
    assert!(!mpv.get_flag("pause"));

    let found = tracks(&mpv);
    assert_eq!(found.audio.len(), 2);
    assert_eq!(found.audio[0].lang.as_deref(), Some("eng"));
    assert_eq!(found.audio[1].lang.as_deref(), Some("jpn"));
    assert_eq!(found.subtitles.len(), 3, "sidecar subtitle not loaded");

    verify_lit_frames(&surface);
    let hwdec = mpv.get_string("hwdec-current").unwrap_or_default();
    println!("h264 hwdec-current: {hwdec:?}");
    assert!(
        hwdec.starts_with("vaapi"),
        "expected VAAPI hardware decoding on H.264, got {hwdec:?}"
    );

    mpv.toggle_pause().unwrap();
    eventually("pause", || mpv.get_flag("pause"));
    let before = mpv.get_f64("time-pos").unwrap();
    mpv.seek(5.0, true).unwrap();
    eventually("seek", || {
        mpv.get_f64("time-pos").is_some_and(|t| t > before + 3.0)
    });
    mpv.toggle_mute().unwrap();
    eventually("mute", || mpv.get_flag("mute"));
    mpv.toggle_mute().unwrap();
    mpv.toggle_pause().unwrap();
    eventually("resume", || !mpv.get_flag("pause"));

    mpv.set_volume(250.0).unwrap();
    assert!((mpv.get_f64("volume").unwrap() - 100.0).abs() < f64::EPSILON);

    mpv.set_audio(found.audio[1].id).unwrap();
    eventually("second audio track", || {
        selected(&tracks(&mpv).audio) == Some(found.audio[1].id)
    });
    mpv.set_subtitle(Some(found.subtitles[1].id)).unwrap();
    eventually("second subtitle", || {
        selected(&tracks(&mpv).subtitles) == Some(found.subtitles[1].id)
    });
    mpv.set_subtitle(None).unwrap();
    eventually("subtitles off", || {
        selected(&tracks(&mpv).subtitles).is_none()
    });
    let sidecar = media.with_extension("srt");
    mpv.add_subtitle(sidecar.to_str().unwrap()).unwrap();
    eventually("sidecar subtitle", || {
        selected(&tracks(&mpv).subtitles).is_some()
    });

    let duration = mpv.get_f64("duration").unwrap();
    mpv.seek(duration - 1.0, false).unwrap();
    eventually("eof-reached", || mpv.get_flag("eof-reached"));
    eventually("keep-open pause", || mpv.get_flag("pause"));

    surface.close();
    drop(reader);
    assert_eq!(Arc::strong_count(&mpv), 1);
}

fn run_software_fallback_check(media: &Path) {
    let mpv = Mpv::new(&[("ao", "null")]).unwrap();
    let surface = LiveSurface::open(&mpv);
    let mut reader = mpv.event_reader().unwrap();
    mpv.load(media.to_str().unwrap(), None).unwrap();
    wait_for(&mut reader, "hevc playback", |e| {
        *e == Event::PlaybackRestart
    });
    verify_lit_frames(&surface);
    let hwdec = mpv.get_string("hwdec-current").unwrap_or_default();
    println!("hevc software fallback hwdec-current: {hwdec:?}");
    assert!(hwdec.is_empty() || hwdec == "no");
    surface.close();
    drop(reader);
    assert_eq!(Arc::strong_count(&mpv), 1);
}

fn run_broken_and_churn_checks(media: &Path) {
    let broken = std::env::temp_dir().join("popcorn-linux-broken-test.mkv");
    std::fs::write(&broken, b"not a video at all").unwrap();
    let mpv = Mpv::new(&[("ao", "null")]).unwrap();
    let surface = LiveSurface::open(&mpv);
    let mut reader = mpv.event_reader().unwrap();
    mpv.load(broken.to_str().unwrap(), None).unwrap();
    let event = wait_for_failure(&mut reader);
    println!("broken media event: {event:?}");
    std::fs::remove_file(&broken).unwrap();
    surface.close();
    drop(reader);
    assert_eq!(Arc::strong_count(&mpv), 1);
    drop(mpv);

    for round in 0..10 {
        let mpv = Mpv::new(&[("ao", "null")]).unwrap();
        let surface = LiveSurface::open(&mpv);
        let reader = mpv.event_reader().unwrap();
        mpv.load(media.to_str().unwrap(), None).unwrap();
        pump_ms(round * 15);
        drop(reader);
        surface.close();
        assert_eq!(Arc::strong_count(&mpv), 1);
    }
}

#[test]
fn linux_ffi_loads_and_initializes_mpv() {
    if ffi::api().is_err() {
        return;
    }
    let mpv = Mpv::new(&[("ao", "null")]).unwrap();
    assert_eq!(mpv.get_string("vo").as_deref(), Some("libmpv"));
    assert!(mpv.get_flag("keep-open"));
    assert!(mpv.event_reader().is_some());
    assert!(mpv.event_reader().is_none());
}

#[test]
#[ignore = "needs DISPLAY and POPCORN_TEST_MEDIA"]
fn linux_engine_plays_real_media_and_tears_down_cleanly() {
    gtk::init().expect("failed to initialize GTK3");
    let media = PathBuf::from(std::env::var("POPCORN_TEST_MEDIA").expect("POPCORN_TEST_MEDIA"));
    run_h264_checks(&media);
    if let Ok(sw) = std::env::var("POPCORN_TEST_MEDIA_SW") {
        run_software_fallback_check(Path::new(&sw));
    }
    run_broken_and_churn_checks(&media);
}
