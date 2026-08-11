//! The pixel guard — an environment hook that fingerprints one frame's
//! draw list, so two builds can be proved to draw the same thing without
//! a GPU, a screenshot or a human eye.
//!
//! `NACELLE_HASH_FRAME` arms it:
//!
//! * `1` — print one line: the frame index, the vertex and run counts and
//!   a 64-bit hash of the whole list.
//! * `dump` — print the list itself, one line per run and one per vertex,
//!   in a fixed decimal format. The hash says *whether* two builds differ;
//!   the dump says *where*.
//!
//! Two more knobs, both optional: `NACELLE_HASH_FRAME_AT` picks the frame
//! (default the first one the main window draws) and `NACELLE_HASH_FRAME_OUT`
//! sends the report to a file instead of stdout. The process leaves as soon
//! as the report is written — an armed run is a measurement, not a session.
//!
//! While armed, the interface clock is VIRTUAL: frame *n* is told the time
//! is *n* × the frame period, whatever the machine actually took to get
//! there. Without that the boot log types itself at the speed of the host
//! and no two runs would ever agree; with it, an armed run is a pure
//! function of the code and the theme. Both clocks a frame carries — the
//! one the draw context reads and the one the widgets read off `Host` —
//! come through [`clock`], because a widget animating off the second
//! would otherwise put the machine's speed back into the list.
//!
//! What the guard does NOT cover, said plainly rather than discovered
//! later: anything driven by a clock of its own (the board ride's
//! `Instant`), and anything the machine's state decides (which processes
//! the collector found). An armed run therefore measures the BOOT frames,
//! before either has anything to say.

use nacelle::draw::DrawList;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// The frame period the main loop keeps (`FRAME` in `main`), as seconds.
const FRAME_SECS: f64 = 1.0 / 60.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Off,
    Hash,
    Dump,
}

/// What a value of `NACELLE_HASH_FRAME` means. Pure, so the parsing is
/// testable — the reader below can only be exercised once per process.
fn mode_of(v: Option<&str>) -> Mode {
    match v {
        Some(v) if v.eq_ignore_ascii_case("dump") => Mode::Dump,
        Some(v) if !v.is_empty() && v != "0" => Mode::Hash,
        _ => Mode::Off,
    }
}

fn mode() -> Mode {
    static MODE: OnceLock<Mode> = OnceLock::new();
    *MODE.get_or_init(|| mode_of(std::env::var("NACELLE_HASH_FRAME").ok().as_deref()))
}

/// Which frame `NACELLE_HASH_FRAME_AT` asks for. Anything unreadable is
/// the first frame: a mistyped number must not turn the guard off
/// silently, and the first frame is the one it would have measured.
fn frame_of(v: Option<&str>) -> u64 {
    v.and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

fn target_frame() -> u64 {
    static AT: OnceLock<u64> = OnceLock::new();
    *AT.get_or_init(|| frame_of(std::env::var("NACELLE_HASH_FRAME_AT").ok().as_deref()))
}

/// Frames the main window has begun. Also the virtual clock's tick count.
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// The time the interface is told it is. Real elapsed seconds normally;
/// the frame counter times the frame period while the guard is armed.
pub fn clock(elapsed: f64) -> f64 {
    if mode() == Mode::Off {
        return elapsed;
    }
    FRAMES.load(Ordering::Relaxed) as f64 * FRAME_SECS
}

/// FNV-1a over the bytes of a float rounded to a thousandth of a pixel —
/// the same value on every machine, and blind to the last bit of a
/// multiply that reassociated.
fn mix(h: &mut u64, v: f32) {
    mix_int(h, (v * 1000.0).round() as i64 as u64);
}

/// The same walk over an integer's bytes, EXACTLY — a vertex index or an
/// image handle is a number, not a measurement, and rounding one through
/// `f32` would let two different lists agree above sixteen million
/// vertices.
fn mix_int(h: &mut u64, v: u64) {
    for b in v.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The whole list as one number.
///
/// The two lengths go in FIRST so the digest is self-delimiting: without
/// them a list of one long run and a list of two short ones could feed
/// the same byte stream and agree, which is exactly the failure a guard
/// may not have.
pub fn digest(dl: &DrawList) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    mix_int(&mut h, dl.verts.len() as u64);
    mix_int(&mut h, dl.runs.len() as u64);
    for v in &dl.verts {
        mix(&mut h, v.pos[0]);
        mix(&mut h, v.pos[1]);
        mix(&mut h, v.uv[0]);
        mix(&mut h, v.uv[1]);
        for c in v.color {
            mix(&mut h, c);
        }
    }
    for r in &dl.runs {
        // Presence first, then the value: a sentinel folded into the
        // number itself would collide with a real clip that happens to
        // hold it.
        match r.image {
            Some(i) => {
                mix_int(&mut h, 1);
                mix_int(&mut h, i.0 as u64);
            }
            None => mix_int(&mut h, 0),
        }
        mix_int(&mut h, r.end as u64);
        match r.clip {
            Some(c) => {
                mix_int(&mut h, 1);
                for v in c {
                    mix(&mut h, v);
                }
            }
            None => mix_int(&mut h, 0),
        }
    }
    h
}

/// Called once per main-window frame, with the list about to be drawn.
/// Off costs one relaxed load; armed, it reports the chosen frame and
/// leaves.
pub fn observe(dl: &DrawList) {
    let m = mode();
    if m == Mode::Off {
        return;
    }
    let n = FRAMES.fetch_add(1, Ordering::Relaxed);
    if n != target_frame() {
        return;
    }

    let mut out: Box<dyn Write> = match std::env::var("NACELLE_HASH_FRAME_OUT") {
        Ok(p) => match std::fs::File::create(&p) {
            Ok(f) => Box::new(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("nacelle-desktop: hash frame: {p}: {e}");
                Box::new(std::io::stdout())
            }
        },
        Err(_) => Box::new(std::io::stdout()),
    };

    let _ = writeln!(
        out,
        "frame {} verts {} runs {} hash {:016x}",
        n,
        dl.verts.len(),
        dl.runs.len(),
        digest(dl)
    );
    if m == Mode::Dump {
        for (i, r) in dl.runs.iter().enumerate() {
            let clip = match r.clip {
                Some([x, y, w, hh]) => format!("{x:.3},{y:.3},{w:.3},{hh:.3}"),
                None => "none".to_string(),
            };
            let _ = writeln!(
                out,
                "run {i} image {} end {} clip {clip}",
                r.image.map(|i| i.0 as i64).unwrap_or(-1),
                r.end
            );
        }
        for (i, v) in dl.verts.iter().enumerate() {
            let _ = writeln!(
                out,
                "v {i} {:.3} {:.3} {:.6} {:.6} {:.4} {:.4} {:.4} {:.4}",
                v.pos[0],
                v.pos[1],
                v.uv[0],
                v.uv[1],
                v.color[0],
                v.color[1],
                v.color[2],
                v.color[3]
            );
        }
    }
    let _ = out.flush();
    drop(out);
    std::process::exit(0);
}

// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nacelle::theme::Color;

    fn ink() -> Color {
        Color { r: 0.5, g: 0.25, b: 0.125, a: 1.0 }
    }

    fn list() -> DrawList {
        let mut dl = DrawList::new();
        dl.rect(10.0, 20.0, 30.0, 40.0, ink());
        dl.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        dl
    }

    #[test]
    fn the_word_decides_the_mode_and_anything_else_is_off() {
        assert_eq!(mode_of(None), Mode::Off);
        assert_eq!(mode_of(Some("")), Mode::Off);
        assert_eq!(mode_of(Some("0")), Mode::Off);
        assert_eq!(mode_of(Some("1")), Mode::Hash);
        assert_eq!(mode_of(Some("yes")), Mode::Hash);
        assert_eq!(mode_of(Some("dump")), Mode::Dump);
        assert_eq!(mode_of(Some("DUMP")), Mode::Dump);
    }

    #[test]
    fn an_unreadable_frame_number_is_the_first_frame() {
        assert_eq!(frame_of(None), 0);
        assert_eq!(frame_of(Some("")), 0);
        assert_eq!(frame_of(Some("banana")), 0);
        assert_eq!(frame_of(Some("-3")), 0);
        assert_eq!(frame_of(Some(" 42 ")), 42);
    }

    /// The claim the whole guard rests on: the same drawing hashes the
    /// same, twice, in one process and across them — nothing in the walk
    /// reads an address, an allocation or a clock.
    #[test]
    fn the_same_drawing_hashes_the_same() {
        assert_eq!(digest(&list()), digest(&list()));
        assert_eq!(digest(&DrawList::new()), digest(&DrawList::new()));
    }

    /// And the other half: a hash that never moves proves nothing.
    #[test]
    fn a_thousandth_of_a_pixel_is_the_grain_the_guard_sees() {
        let mut moved = DrawList::new();
        moved.rect(10.001, 20.0, 30.0, 40.0, ink());
        moved.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        assert_ne!(digest(&list()), digest(&moved), "a moved rectangle is a different frame");

        // Half a thousandth is under the grain and deliberately invisible
        // — that tolerance is what makes the guard survive a compiler
        // reassociating a multiply.
        let mut jitter = DrawList::new();
        jitter.rect(10.0004, 20.0, 30.0, 40.0, ink());
        jitter.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        assert_eq!(digest(&list()), digest(&jitter));
    }

    /// A colour change moves no vertex, and would be invisible to a hash
    /// of positions alone.
    #[test]
    fn a_recoloured_frame_is_a_different_frame() {
        let mut other = DrawList::new();
        other.rect(10.0, 20.0, 30.0, 40.0, Color { r: 0.5, g: 0.25, b: 0.125, a: 0.99 });
        other.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        assert_ne!(digest(&list()), digest(&other));
    }

    /// The lengths are hashed first for this: two lists that hold the
    /// same vertices in a different number of runs are different lists,
    /// and a stream of bytes with no lengths in it could not tell.
    #[test]
    fn the_run_structure_is_part_of_the_fingerprint() {
        let mut split = list();
        // A run with no vertices of its own: same geometry, one more run.
        split.rect(10.0, 20.0, 0.0, 0.0, ink());
        assert_ne!(digest(&list()), digest(&split));
    }
}
