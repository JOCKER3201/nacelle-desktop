//! Audio output — the platform half of the sound system.
//!
//! Everything about WHAT to play (events, themes, WAV decoding, mixing)
//! is platform-independent and lives in nacelle::sound. This file is
//! only the device.
//!
//! ALSA is reached through dlopen rather than by linking to it, so
//! building nacelle-desktop needs no audio development package at all — only
//! libasound.so.2 at run time, which every desktop Linux has (on a
//! PipeWire system it is PipeWire's own compatibility layer). This
//! mirrors how the terminal already talks to the system in pty.rs.
//!
//! Sound is strictly optional. No audio library, no device, a busy
//! card — every one of those leaves the program fully usable and merely
//! silent, so nothing here may fail loudly.

use nacelle::sound::{Clip, Event, SharedMixer, SoundTheme};
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ALSA constants (alsa/pcm.h).
const STREAM_PLAYBACK: c_int = 0;
const ACCESS_RW_INTERLEAVED: c_int = 3;
const FORMAT_S16_LE: c_int = 2;
const FORMAT_FLOAT_LE: c_int = 14;

/// How much audio is in flight. 256 frames at 48 kHz is ~5 ms per
/// write, short enough that a keystroke is heard as immediate.
const PERIOD_FRAMES: u64 = 256;
const BUFFER_FRAMES: u64 = 1024;
const CHANNELS: u32 = 2;
const WANTED_RATE: u32 = 48_000;

type Ulong = std::ffi::c_ulong;
type Slong = std::ffi::c_long;

/// The handful of libasound entry points needed to play a stream.
struct Alsa {
    /// Kept so the library stays loaded for the lifetime of the struct;
    /// never called through.
    #[allow(dead_code)]
    handle: *mut c_void,
    open: unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
    prepare: unsafe extern "C" fn(*mut c_void) -> c_int,
    drain: unsafe extern "C" fn(*mut c_void) -> c_int,
    writei: unsafe extern "C" fn(*mut c_void, *const c_void, Ulong) -> Slong,
    recover: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
    hw_malloc: unsafe extern "C" fn(*mut *mut c_void) -> c_int,
    hw_free: unsafe extern "C" fn(*mut c_void),
    hw_any: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    hw_set_access: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int,
    hw_set_format: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int,
    hw_set_channels: unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint) -> c_int,
    hw_set_rate_near:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_uint, *mut c_int) -> c_int,
    hw_set_period_near:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Ulong, *mut c_int) -> c_int,
    hw_set_buffer_near: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Ulong) -> c_int,
    hw_apply: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
}

/// Every pointer here is used only by the single writer thread that owns
/// them; nothing else in the program can reach them.
unsafe impl Send for Alsa {}
unsafe impl Sync for Alsa {}

/// Resolves one entry point. The target type is named at the call site
/// by the struct field it initialises; transmuting a dlsym pointer is
/// how a C function is reached, and the signatures above are the
/// contract with libasound.
macro_rules! sym {
    ($lib:expr, $name:literal, $ty:ty) => {{
        let cname = CString::new($name).ok()?;
        let p = libc::dlsym($lib, cname.as_ptr());
        if p.is_null() {
            libc::dlclose($lib);
            return None;
        }
        std::mem::transmute::<*mut c_void, $ty>(p)
    }};
}

impl Alsa {
    /// Loads libasound.so.2 and resolves the entry points. None means
    /// there is no usable ALSA on this machine.
    unsafe fn load() -> Option<Alsa> {
        let name = CString::new("libasound.so.2").ok()?;
        let lib = libc::dlopen(name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if lib.is_null() {
            return None;
        }
        Some(Alsa {
            open: sym!(lib, "snd_pcm_open", unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int),
            close: sym!(lib, "snd_pcm_close", unsafe extern "C" fn(*mut c_void) -> c_int),
            prepare: sym!(lib, "snd_pcm_prepare", unsafe extern "C" fn(*mut c_void) -> c_int),
            drain: sym!(lib, "snd_pcm_drain", unsafe extern "C" fn(*mut c_void) -> c_int),
            writei: sym!(lib, "snd_pcm_writei", unsafe extern "C" fn(*mut c_void, *const c_void, Ulong) -> Slong),
            recover: sym!(lib, "snd_pcm_recover", unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int),
            hw_malloc: sym!(lib, "snd_pcm_hw_params_malloc", unsafe extern "C" fn(*mut *mut c_void) -> c_int),
            hw_free: sym!(lib, "snd_pcm_hw_params_free", unsafe extern "C" fn(*mut c_void)),
            hw_any: sym!(lib, "snd_pcm_hw_params_any", unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int),
            hw_set_access: sym!(lib, "snd_pcm_hw_params_set_access", unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int),
            hw_set_format: sym!(lib, "snd_pcm_hw_params_set_format", unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int),
            hw_set_channels: sym!(lib, "snd_pcm_hw_params_set_channels", unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint) -> c_int),
            hw_set_rate_near: sym!(lib, "snd_pcm_hw_params_set_rate_near", unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_uint, *mut c_int) -> c_int),
            hw_set_period_near: sym!(lib, "snd_pcm_hw_params_set_period_size_near", unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Ulong, *mut c_int) -> c_int),
            hw_set_buffer_near: sym!(lib, "snd_pcm_hw_params_set_buffer_size_near", unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Ulong) -> c_int),
            hw_apply: sym!(lib, "snd_pcm_hw_params", unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int),
            handle: lib,
        })
    }

    /// Opens the default playback device. Returns the PCM handle, the
    /// rate actually granted and whether the device took float samples.
    unsafe fn open_default(&self) -> Option<(*mut c_void, u32, bool)> {
        let mut pcm: *mut c_void = std::ptr::null_mut();
        let dev = CString::new("default").ok()?;
        if (self.open)(&mut pcm, dev.as_ptr(), STREAM_PLAYBACK, 0) < 0 || pcm.is_null() {
            return None;
        }

        let mut params: *mut c_void = std::ptr::null_mut();
        if (self.hw_malloc)(&mut params) < 0 || params.is_null() {
            (self.close)(pcm);
            return None;
        }
        // From here on every failure must free the params block and
        // close the device before returning.
        let fail = |s: &Alsa, params: *mut c_void, pcm: *mut c_void| {
            (s.hw_free)(params);
            (s.close)(pcm);
            None::<(*mut c_void, u32, bool)>
        };

        if (self.hw_any)(pcm, params) < 0
            || (self.hw_set_access)(pcm, params, ACCESS_RW_INTERLEAVED) < 0
        {
            return fail(self, params, pcm);
        }
        // Float is what the mixer produces; S16 is the fallback for a
        // device that will not take it.
        let float = (self.hw_set_format)(pcm, params, FORMAT_FLOAT_LE) >= 0;
        if !float && (self.hw_set_format)(pcm, params, FORMAT_S16_LE) < 0 {
            return fail(self, params, pcm);
        }
        if (self.hw_set_channels)(pcm, params, CHANNELS) < 0 {
            return fail(self, params, pcm);
        }
        let mut rate: c_uint = WANTED_RATE;
        let mut dir: c_int = 0;
        if (self.hw_set_rate_near)(pcm, params, &mut rate, &mut dir) < 0 || rate == 0 {
            return fail(self, params, pcm);
        }
        // Period and buffer are advisory: a device that refuses them
        // still works, just with whatever latency it prefers.
        let mut period: Ulong = PERIOD_FRAMES as Ulong;
        let mut buffer: Ulong = BUFFER_FRAMES as Ulong;
        let mut pdir: c_int = 0;
        let _ = (self.hw_set_period_near)(pcm, params, &mut period, &mut pdir);
        let _ = (self.hw_set_buffer_near)(pcm, params, &mut buffer);

        if (self.hw_apply)(pcm, params) < 0 {
            return fail(self, params, pcm);
        }
        (self.hw_free)(params);
        if (self.prepare)(pcm) < 0 {
            (self.close)(pcm);
            return None;
        }
        Some((pcm, rate as u32, float))
    }
}

/// The PCM handle, moved to the writer thread that exclusively owns it.
struct Pcm(*mut c_void);
unsafe impl Send for Pcm {}

pub struct Audio {
    mixer: Arc<SharedMixer>,
    rate: u32,
    theme: SoundTheme,
    stop: Arc<AtomicBool>,
    writer: Option<std::thread::JoinHandle<()>>,
    volume: f32,
    typing: bool,
    ambient_on: bool,
}

impl Audio {
    /// Opens the default output. None means "no sound available"; the
    /// caller simply carries on without it.
    pub fn new() -> Option<Audio> {
        let (alsa, pcm, rate, float) = unsafe {
            let alsa = Alsa::load()?;
            let (pcm, rate, float) = alsa.open_default()?;
            (Arc::new(alsa), Pcm(pcm), rate, float)
        };

        let mixer = Arc::new(SharedMixer::new());
        let stop = Arc::new(AtomicBool::new(false));
        let m = mixer.clone();
        let s = stop.clone();
        let a = alsa.clone();

        // A dedicated writer thread. Blocking writes pace themselves
        // against the card's clock, so no timer of our own is needed.
        let writer = std::thread::Builder::new()
            .name("nacelle-desktop-audio".into())
            .spawn(move || {
                let pcm = pcm;
                let frames = PERIOD_FRAMES as usize;
                let ch = CHANNELS as usize;
                let mut buf = vec![0.0f32; frames * ch];
                let mut pcm16 = vec![0i16; frames * ch];
                while !s.load(Ordering::Relaxed) {
                    // Filling THROUGH the shared mixer is what raises
                    // the "the last clip just ended" edge that
                    // `play_blocking` waits on; a bare `lock().fill()`
                    // here would render the same samples and tell
                    // nobody, which is how the exit came to be timed by
                    // a constant instead of by the sound.
                    m.fill(&mut buf, ch);
                    if !float {
                        for (o, i) in pcm16.iter_mut().zip(buf.iter()) {
                            *o = (i.clamp(-1.0, 1.0) * 32767.0) as i16;
                        }
                    }
                    let base: *const c_void = if float {
                        buf.as_ptr() as *const c_void
                    } else {
                        pcm16.as_ptr() as *const c_void
                    };
                    let bytes_per_frame = if float { 4 * ch } else { 2 * ch };

                    let mut done = 0usize;
                    while done < frames {
                        let ptr = unsafe { (base as *const u8).add(done * bytes_per_frame) };
                        let n = unsafe {
                            (a.writei)(pcm.0, ptr as *const c_void, (frames - done) as Ulong)
                        };
                        if n < 0 {
                            // Underrun or a suspended device: recover
                            // once, and give up on this period if the
                            // card will not come back.
                            if unsafe { (a.recover)(pcm.0, n as c_int, 1) } < 0 {
                                break;
                            }
                        } else if n == 0 {
                            break;
                        } else {
                            done += n as usize;
                        }
                    }
                }
                unsafe {
                    (a.drain)(pcm.0);
                    (a.close)(pcm.0);
                }
            })
            .ok()?;

        Some(Audio {
            mixer,
            rate,
            theme: SoundTheme::empty(),
            stop,
            writer: Some(writer),
            volume: 1.0,
            typing: true,
            ambient_on: true,
        })
    }

    /// Loads a sound theme directory, replacing the current one. The
    /// meta file inside is what maps events to files.
    pub fn load_theme(&mut self, dir: &Path) {
        self.theme = SoundTheme::load(dir, self.rate);
        self.restart_ambient();
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// How many events the loaded theme provides sounds for.
    pub fn event_count(&self) -> usize {
        self.theme.event_count()
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        self.mixer.lock().set_volume(self.volume);
    }

    pub fn set_typing_enabled(&mut self, on: bool) {
        self.typing = on;
    }

    pub fn set_ambient_enabled(&mut self, on: bool) {
        self.ambient_on = on;
        self.restart_ambient();
    }

    fn restart_ambient(&mut self) {
        let clip: Option<Clip> = if self.ambient_on {
            self.theme.clip(Event::Ambient)
        } else {
            None
        };
        self.mixer.lock().set_ambient(clip);
    }

    /// Plays whatever the theme assigns to this event. An event the
    /// theme says nothing about is silent — by design, not by accident.
    pub fn play(&mut self, e: Event) {
        if e == Event::Ambient {
            return; // the bed is driven by restart_ambient(), not by events
        }
        if e.is_typing() && !self.typing {
            return;
        }
        let Some(clip) = self.theme.clip(e) else { return };
        self.mixer.lock().play(clip, 1.0);
    }

    /// The longest the exit may wait for a farewell clip of `frames`:
    /// the clip's own length plus the one device buffer that is still in
    /// flight at the moment the mixer says it is finished.
    ///
    /// This is a WATCHDOG, not a duration. What normally ends the wait is
    /// [`SharedMixer::wait_drained`]'s event; this number only bounds the
    /// case where the card has stopped consuming buffers altogether, and
    /// it is derived rather than chosen so that no length lives in Rust:
    /// the length of the goodbye is the length of the file the sound
    /// theme ships, which is the theme's decision and nobody else's.
    ///
    /// The predecessor was `min(clip_ms, 1400) + 60`, and both halves
    /// were wrong. The `min` CUT THE THEME OFF — the master theme's
    /// `shutdown.wav` is 1800 ms, so the program silenced its own sound
    /// 400 ms early — and the sleep spent the whole nominal length
    /// whatever the card was doing, which is where strace found a
    /// `clock_nanosleep` of exactly 1.46 s on every single exit.
    fn farewell_cap(frames: usize, rate: u32) -> Duration {
        let rate = rate.max(1) as u64;
        Duration::from_millis((frames as u64 + BUFFER_FRAMES) * 1000 / rate)
    }

    /// Plays a clip and waits for it to actually finish, for the
    /// shutdown sound the process would otherwise cut off on its way out.
    ///
    /// The wait ends on an EVENT: the writer thread renders the last
    /// sample of the clip, the shared mixer raises its drained edge, and
    /// this returns — typically before the nominal length, because the
    /// samples still inside the device are then flushed by the ALSA
    /// drain in `Drop`, which was always going to happen anyway.
    /// [`Audio::farewell_cap`] is only the fuse on a card that has
    /// stopped taking buffers.
    pub fn play_blocking(&mut self, e: Event) {
        let Some(clip) = self.theme.clip(e) else { return };
        let cap = Audio::farewell_cap(clip.len(), self.rate);
        self.mixer.lock().play(clip, 1.0);
        self.mixer.wait_drained(cap);
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        // Stopping the writer is what drains and closes the device; the
        // library handle is deliberately left loaded, since unloading it
        // while ALSA may still hold threads of its own buys nothing.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The master sound theme's own `shutdown.wav`: 86 400 mono frames
    /// at 48 kHz, which is 1800 ms. Stated as the numbers rather than
    /// read from the file, because the file lives in another repository
    /// and this test is about arithmetic, not about installation.
    const SHUTDOWN_FRAMES: usize = 86_400;
    const SHUTDOWN_RATE: u32 = 48_000;

    /// THE THEME'S CLIP MUST NOT BE CUT OFF BY A NUMBER IN RUST.
    ///
    /// The predecessor computed `min(1800, 1400) + 60` = 1460 ms and
    /// slept exactly that on every exit — which is both the 1.46 s
    /// strace measured and 400 ms of the theme's goodbye thrown away.
    /// A cap below the clip is a cap that silences the sound.
    #[test]
    fn the_farewell_cap_never_truncates_the_theme() {
        let cap = Audio::farewell_cap(SHUTDOWN_FRAMES, SHUTDOWN_RATE);
        assert!(
            cap >= Duration::from_millis(1800),
            "the master theme's 1800 ms shutdown would be cut at {cap:?}"
        );
        // And it is a fuse, not a pause: nothing beyond the clip plus
        // the single device buffer that is still in flight.
        assert!(
            cap <= Duration::from_millis(1800) + Duration::from_millis(50),
            "{cap:?} is longer than the sound could honestly take"
        );
    }

    /// A theme with a short sound must pay a short fuse. The cap follows
    /// the clip; nothing in it is a constant of the program's own.
    #[test]
    fn the_farewell_cap_follows_the_clip() {
        let short = Audio::farewell_cap(4_800, 48_000); // 100 ms
        let long = Audio::farewell_cap(240_000, 48_000); // 5 s
        assert!(short >= Duration::from_millis(100));
        assert!(short < Duration::from_millis(200));
        assert!(long >= Duration::from_secs(5));
        // A theme with no shutdown sound at all waits for nothing.
        assert_eq!(Audio::farewell_cap(0, 48_000), Duration::from_millis(21));
        // A device that reports a nonsense rate must not divide by zero.
        let _ = Audio::farewell_cap(4_800, 0);
    }
}
