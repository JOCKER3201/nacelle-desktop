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
        let writer = crate::threads::spawn(crate::threads::AUDIO, move || {
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

    /// Starts the farewell and waits for the device to have rendered all
    /// of it. True means the sound ended the wait, false that the fuse
    /// did.
    ///
    /// Split out of [`Audio::play_blocking`] — which is this plus "ask
    /// the theme which clip" — because this is the part that HAS a
    /// duration, and a duration that cannot be measured is a duration
    /// nobody can be held to. `play_blocking` needs a card; these three
    /// lines need a mixer, so the measuring test drives exactly the code
    /// the exit runs, against a stand-in device kept at the cadence this
    /// file opens the real one with.
    fn play_and_wait(mixer: &SharedMixer, clip: Clip, rate: u32) -> bool {
        let cap = Audio::farewell_cap(clip.len(), rate);
        mixer.lock().play(clip, 1.0);
        mixer.wait_drained(cap)
    }

    /// Plays a clip and waits for it to actually finish, for the
    /// shutdown sound the process would otherwise cut off on its way out.
    ///
    /// The wait ends on an EVENT: the writer thread renders the last
    /// sample of the clip, the shared mixer raises its drained edge, and
    /// this returns — before the last samples are audible, because those
    /// are still inside the device and are then flushed by the ALSA
    /// drain in `Drop`, which was always going to happen anyway.
    /// [`Audio::farewell_cap`] is only the fuse on a card that has
    /// stopped taking buffers.
    pub fn play_blocking(&mut self, e: Event) {
        let Some(clip) = self.theme.clip(e) else { return };
        Audio::play_and_wait(&self.mixer, clip, self.rate);
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
    use std::path::PathBuf;
    use std::time::Instant;

    /// The rate the master theme's files are authored at, and the one
    /// the card is opened at on this machine. It is a property of the
    /// FIXTURE — the wav these tests write — not a length the program
    /// keeps: nothing in `audio.rs` may assume it.
    const FIXTURE_RATE: u32 = 48_000;

    /// The master theme's `shutdown.wav` is 86 400 mono frames at
    /// 48 kHz. The fixture is written to that length so the measurement
    /// below is the measurement of the real goodbye, but the length is
    /// carried by a FILE the test writes and the program then reads,
    /// exactly as a theme's file would be — see [`theme_dir`].
    const SHUTDOWN_FRAMES: usize = 86_400;

    /// A 16-bit mono RIFF/WAVE, which is what a sound theme ships.
    fn wav16_mono(frames: usize, rate: u32) -> Vec<u8> {
        let data: Vec<u8> = (0..frames)
            // A quiet ramp rather than silence: a decoder that returned
            // an empty clip would otherwise look like a working one.
            .flat_map(|i| ((i % 1024) as i16 * 8).to_le_bytes())
            .collect();
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&data);
        v
    }

    /// A sound theme on disk: a `meta` file and the wav it names, which
    /// is the only way the program ever learns how long a goodbye is.
    ///
    /// The bytes are written here rather than borrowed from
    /// nacelle-themes because a test that needed another repository
    /// checked out beside this one would be testing installation. What
    /// it must not do is state the length in Rust and then check its own
    /// statement: `frames` goes into a FILE, and everything downstream —
    /// the decoder, the clip, the cap, the wait — learns it from there.
    fn theme_dir(tag: &str, frames: Option<usize>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nacelle-farewell-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("theme dir");
        match frames {
            Some(frames) => {
                std::fs::write(dir.join("shutdown.wav"), wav16_mono(frames, FIXTURE_RATE))
                    .expect("wav");
                std::fs::write(dir.join("meta"), "Shutdown=shutdown.wav\n").expect("meta");
            }
            // A theme that simply has no goodbye. The file it does name
            // keeps this from being "an unreadable theme" instead.
            None => {
                std::fs::write(dir.join("click.wav"), wav16_mono(64, FIXTURE_RATE)).expect("wav");
                std::fs::write(dir.join("meta"), "Click=click.wav\n").expect("meta");
            }
        }
        dir
    }

    /// A stand-in for the writer thread in [`Audio::new`], kept at the
    /// cadence this file opens the card with: one `fill` of
    /// `PERIOD_FRAMES` per period at the device rate.
    ///
    /// The pacing is against an ABSOLUTE clock, and that is not a
    /// refinement — it is what a blocking `snd_pcm_writei` does. It
    /// returns the moment the card has room for another period, so a
    /// late wakeup is caught up on the next write instead of being added
    /// to the total. A device paced by `sleep(period)` after each fill
    /// would drift by every scheduling delay it ever suffered and would
    /// make this measurement say more about the test runner's load than
    /// about the program.
    ///
    /// Answers how many periods it rendered — the count is the
    /// instrument: elapsed time is a machine's opinion, but the number
    /// of buffers it took to render a clip is arithmetic.
    fn paced_device(
        mixer: Arc<SharedMixer>,
        stop: Arc<AtomicBool>,
        rate: u32,
    ) -> std::thread::JoinHandle<u64> {
        let period = Duration::from_nanos(PERIOD_FRAMES * 1_000_000_000 / rate as u64);
        std::thread::spawn(move || {
            let ch = CHANNELS as usize;
            let mut buf = vec![0.0f32; PERIOD_FRAMES as usize * ch];
            let start = Instant::now();
            let mut periods = 0u64;
            while !stop.load(Ordering::Relaxed) {
                mixer.fill(&mut buf, ch);
                periods += 1;
                let due = start + period * periods as u32;
                let now = Instant::now();
                if due > now {
                    std::thread::sleep(due - now);
                }
            }
            periods
        })
    }

    /// What one farewell cost, measured at the production cadence.
    struct Farewell {
        waited: Duration,
        cap: Duration,
        drained: bool,
        periods: u64,
    }

    /// Runs the exit's own wait — [`Audio::play_and_wait`] — against a
    /// device at the production cadence, and reports what it cost.
    fn measure(clip: Clip) -> Farewell {
        let cap = Audio::farewell_cap(clip.len(), FIXTURE_RATE);
        let mixer = Arc::new(SharedMixer::new());
        let stop = Arc::new(AtomicBool::new(false));
        let dev = paced_device(mixer.clone(), stop.clone(), FIXTURE_RATE);

        let t0 = Instant::now();
        let drained = Audio::play_and_wait(&mixer, clip, FIXTURE_RATE);
        let waited = t0.elapsed();

        stop.store(true, Ordering::Relaxed);
        let periods = dev.join().expect("device thread");
        Farewell {
            waited,
            cap,
            drained,
            periods,
        }
    }

    /// THE THEME'S CLIP MUST NOT BE CUT OFF BY A NUMBER IN RUST — and
    /// this is the measurement that says so, not an assertion about
    /// arithmetic.
    ///
    /// A theme is written to disk with a 1800 ms goodbye, loaded the way
    /// the desktop loads one, and played through the code the exit runs,
    /// against a device kept at the cadence the card is opened with. The
    /// predecessor slept `min(1800, 1400) + 60` = 1460 ms flat: it threw
    /// away 400 ms of the theme's own sound, and strace found that
    /// constant on every single exit. So the wait now has to come out
    /// LONGER than 1.46 s, and it has to be the sound that ends it.
    ///
    /// The period count is the part that does not depend on this
    /// machine's mood: 86 400 frames at 256 to a period is 338 buffers,
    /// and no arrangement of scheduling luck renders the clip in fewer.
    #[test]
    fn the_exit_waits_for_the_whole_goodbye_the_theme_ships() {
        let dir = theme_dir("full", Some(SHUTDOWN_FRAMES));
        let mut theme = SoundTheme::load(&dir, FIXTURE_RATE);
        let clip = theme
            .clip(Event::Shutdown)
            .expect("the theme's file is the only source of the goodbye");
        let frames = clip.len();
        assert_eq!(frames, SHUTDOWN_FRAMES, "the decoder lost part of the file");

        let m = measure(clip);
        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "MEASURED farewell: {} frames ({} ms of file) -> waited {:?}, \
             fuse {:?}, ended by {}, {} device periods",
            frames,
            frames as u64 * 1000 / FIXTURE_RATE as u64,
            m.waited,
            m.cap,
            if m.drained { "the sound" } else { "the fuse" },
            m.periods
        );

        assert!(
            m.drained,
            "the fuse ended the wait at {:?}, so the goodbye was cut short",
            m.waited
        );
        assert!(
            m.waited > Duration::from_millis(1460),
            "waited {:?}, which is no more than the constant this replaced",
            m.waited
        );
        let needed = frames.div_ceil(PERIOD_FRAMES as usize) as u64;
        assert_eq!(needed, 338, "the fixture's cadence changed under the test");
        assert!(
            m.periods >= needed,
            "{} periods cannot hold {frames} frames — part of the file was never rendered",
            m.periods
        );
        // And it did not run on past the sound: the wait is over within
        // a couple of periods of the last one being handed to the card.
        assert!(
            m.periods <= needed + 4,
            "{} periods for {needed} periods of sound",
            m.periods
        );
    }

    /// The same measurement with a short goodbye. Nothing in the wait is
    /// a constant of the program's own, so a theme with a 200 ms file
    /// pays 200 ms — where the predecessor's `+ 60` and its `min` made
    /// every exit cost roughly the same whatever the theme said.
    #[test]
    fn a_short_goodbye_costs_what_it_says_it_costs() {
        let frames = 9_600; // 200 ms
        let dir = theme_dir("short", Some(frames));
        let mut theme = SoundTheme::load(&dir, FIXTURE_RATE);
        let clip = theme.clip(Event::Shutdown).expect("the theme's clip");
        let m = measure(clip);
        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "MEASURED farewell: {} frames (200 ms of file) -> waited {:?}, \
             fuse {:?}, ended by {}, {} device periods",
            frames,
            m.waited,
            m.cap,
            if m.drained { "the sound" } else { "the fuse" },
            m.periods
        );

        assert!(m.drained, "the fuse ended a 200 ms wait at {:?}", m.waited);
        assert!(
            m.waited < Duration::from_millis(1460),
            "a 200 ms goodbye waited {:?}",
            m.waited
        );
        let needed = frames.div_ceil(PERIOD_FRAMES as usize) as u64;
        assert!(
            m.periods >= needed,
            "{} periods cannot hold {frames} frames",
            m.periods
        );
        assert!(
            m.periods <= needed + 4,
            "{} device periods for {needed} periods of sound — the wait \
             outlasted the clip, which is what a fixed sleep does",
            m.periods
        );
    }

    /// A theme with no goodbye at all costs nothing: no clip means no
    /// wait, and the exit is immediate.
    ///
    /// This one regresses NOTHING — the predecessor left just as fast,
    /// its `let Some(clip) = ... else { return }` came before the sleep.
    /// It is here because the earlier report claimed an "empty clip"
    /// went from 60 ms to 0, and that case does not exist: a wav with no
    /// samples fails to decode, the event stays silent, and there is no
    /// zero-length clip for either version to wait on. Both halves of
    /// that are checked below so the claim cannot come back.
    #[test]
    fn a_theme_without_a_goodbye_pays_nothing() {
        let named = theme_dir("silent", None);
        let mut theme = SoundTheme::load(&named, FIXTURE_RATE);
        assert!(theme.event_count() > 0, "the fixture theme loaded as empty");
        assert!(
            theme.clip(Event::Shutdown).is_none(),
            "a theme that names no shutdown file must offer no clip"
        );
        let _ = std::fs::remove_dir_all(&named);

        // A named file with nothing in it is the same answer: silence,
        // not a clip of length zero.
        let hollow = theme_dir("hollow", Some(0));
        let mut theme = SoundTheme::load(&hollow, FIXTURE_RATE);
        assert!(
            theme.clip(Event::Shutdown).is_none(),
            "a wav with no samples must leave the event silent"
        );
        let _ = std::fs::remove_dir_all(&hollow);
    }

    /// The fuse follows the clip: a short sound gets a short fuse, a long
    /// one a long fuse, and no length in it belongs to the program.
    ///
    /// This one IS about the arithmetic — the measurements above are what
    /// say the arithmetic is the right arithmetic.
    #[test]
    fn the_farewell_cap_follows_the_clip() {
        let short = Audio::farewell_cap(4_800, 48_000); // 100 ms
        let long = Audio::farewell_cap(240_000, 48_000); // 5 s
        assert!(short >= Duration::from_millis(100));
        assert!(short < Duration::from_millis(200));
        assert!(long >= Duration::from_secs(5));
        // The master theme's own goodbye must not be cut: a fuse below
        // the clip is a fuse that silences the sound, which is precisely
        // what `min(clip_ms, 1400)` did to it.
        assert!(Audio::farewell_cap(SHUTDOWN_FRAMES, 48_000) >= Duration::from_millis(1800));
        // A theme with no shutdown sound at all waits for nothing.
        assert_eq!(Audio::farewell_cap(0, 48_000), Duration::from_millis(21));
        // A device that reports a nonsense rate must not divide by zero.
        let _ = Audio::farewell_cap(4_800, 0);
    }
}
