//! The pixel guard — an environment hook that fingerprints one frame's
//! draw list, so two builds can be proved to draw the same thing without
//! a GPU, a screenshot or a human eye.
//!
//! # Which report answers which question
//!
//! The reports look at one frame through three different grains, and the
//! grain IS the question. A reader who reaches for the wrong one gets a
//! confident answer to something else — which is worse than no answer,
//! because it reads like a clean bill of health. Pick by the question:
//!
//! * `verts` — **is the geometry identical?** Every vertex in order, plus
//!   every run. Sensitive to everything, including a retessellation that
//!   paints exactly the same picture out of different triangles. Reach for
//!   it when the change is supposed to move no vertex at all: an identity
//!   transform in the vertex stage, a refactor of layout arithmetic, a
//!   field appended to `Vertex`.
//! * `cmds` — **are we drawing the same things, however they are built?**
//!   The command register, without the triangles it expands to. Blind to
//!   tessellation by construction. Reach for it when the change is *meant*
//!   to rebuild geometry — an SDF core replacing a ring of triangles —
//!   and the picture has to survive it.
//! * `bbox` — **is it in the same place, at the same size?** One rectangle
//!   per run around its own vertices, with the texture it samples and
//!   the scissor it is cut by. Coarse and cheap: it catches a thing that
//!   moved, grew or vanished, and says nothing at all about how it is
//!   built. Reach for it first when something differs, and keep it as the
//!   standing check across a change that rebuilds geometry on purpose.
//!
//! One honest caveat about the last one, because it decides what a clean
//! `bbox` diff is worth. Its unit is the RUN — everything between two
//! changes of texture or scissor — because the run is the only unit the
//! vertex list partitions itself into. Whole boards therefore share one
//! rectangle, and a shape that moves inside another's span does not move
//! it. Boxes that match mean the frame's coarse layout survived; they do
//! not mean nothing moved.
//!
//! The command register does not fix that and was never going to: it
//! records intent and deliberately keeps no vertex range, so there is
//! nothing to take a per-command rectangle *of*. What it does instead is
//! carry each command's own box in the command itself, which is why a
//! move inside a run shows up in `cmds` even when `bbox` shrugs. The two
//! coarse reports are complementary, not one refined into the other.
//!
//! How coarse that leaves `bbox` is worth stating as a measurement
//! rather than a worry. A settled desktop frame at 1920×1080 — 7740
//! vertices, 380 commands — is ONE run: every command samples the glyph
//! atlas and nothing pushes a scissor, so `bbox` prints a single
//! rectangle, the layout's content box. That makes it a genuine standing
//! check (it moved the moment the window did) and a very blunt one (a
//! panel recoloured, or a label slid across the board, does not touch
//! it). Read `bbox` as "the frame still occupies the same ground" and
//! `cmds` for everything finer.
//!
//! And the bare hash is the yes/no over `verts`: same number, same
//! geometry; different number, go read a dump to find out where.
//!
//! No report stands in for another. `verts` never excuses a triangle that
//! moved; `bbox` never complains about one that was rebuilt where it was.
//! A change that must not touch the picture is therefore argued with a
//! pair of them — `cmds` and `bbox` identical, `verts` different in
//! exactly the way the commit claims and no other.
//!
//! # Arming it
//!
//! `NACELLE_HASH_FRAME` picks the report. Surrounding blanks are ignored;
//! the word is case-blind:
//!
//! * unset, empty or `0` — off, and the whole hook costs one atomic load.
//! * `verts` (or `dump`, the older word) — the header, then one line per
//!   run and one per vertex in a fixed decimal format.
//! * `cmds` — the header, then the command register, one line per call.
//!   The register is the toolkit's, and it is off unless somebody arms
//!   it; this mode arms it in [`arm`], which `main` calls before the
//!   first draw list exists. A frame whose list is not recording after
//!   all reports that it cannot answer and leaves with status 2, rather
//!   than an empty body a diff would read as a match.
//! * `bbox` — the header, then one line per run: its texture, its scissor
//!   and its bounding rectangle, and the frame's own outline last.
//! * anything else — the header alone.
//!
//! Two more knobs, both optional: `NACELLE_HASH_FRAME_AT` picks the frame
//! (default the first one the main window draws) and `NACELLE_HASH_FRAME_OUT`
//! sends the report to a file instead of stdout. The process leaves as soon
//! as the report is written — an armed run is a measurement, not a session.
//!
//! Every report names its own mode on the header line, so a file found
//! later, or produced by a word the guard did not recognise, says which
//! question it answered.
//!
//! The exit status is part of the report: **0** when the guard answered,
//! **2** when it could not. A comparison script that only diffs two files
//! would otherwise read two identical apologies as two identical frames.
//!
//! # The clock
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
//!
//! # Taking a measurement
//!
//! A run needs a compositor, a fixed size and nothing else:
//!
//! ```text
//! gamescope --backend headless --expose-wayland -W 1920 -H 1080 \
//!     -w 1920 -h 1080 -r 60 -- sh -c '
//!         WAYLAND_DISPLAY=$GAMESCOPE_WAYLAND_DISPLAY \
//!         NACELLE_HASH_FRAME=cmds \
//!         NACELLE_HASH_FRAME_AT=200 \
//!         NACELLE_HASH_FRAME_OUT=/tmp/before-cmds.txt \
//!         nacelle-desktop'
//! ```
//!
//! The display name comes out of the client's OWN environment, which
//! gamescope fills in; guessing it, or reading it off some other
//! compositor on the machine, is how a measurement quietly becomes a
//! measurement of something else.
//!
//! Frame 200 rather than 0 on purpose. The boot animation is still
//! running at 0 (294 vertices there against 7740 at 200), and it has
//! settled long before 200 — frames 200 and 400 hash alike, so any
//! number past the settle measures the same standing picture.
//!
//! Then the same three files after the change, and `cmp` decides. What
//! each verdict means is the table at the top of this module; the short
//! version for the image phase is that a tessellation core must move
//! `verts` and leave `cmds` and `bbox` exactly where they were.

use nacelle::draw::{DrawList, DrawRun, Vertex};
use std::fmt::Write as _;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// The frame period the main loop keeps (`FRAME` in `main`), as seconds.
const FRAME_SECS: f64 = 1.0 / 60.0;

/// The status an armed run leaves with when it could not answer the
/// question it was asked. Distinct from 1, which is what a panicking or
/// failing program already means.
const EXIT_UNANSWERED: i32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Off,
    Hash,
    Verts,
    Cmds,
    Bbox,
}

impl Mode {
    /// The word that selects this mode, and the word the header prints.
    /// One vocabulary for the environment and for the report, so what a
    /// file says it is can be pasted straight back into the shell.
    fn word(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Hash => "hash",
            Mode::Verts => "verts",
            Mode::Cmds => "cmds",
            Mode::Bbox => "bbox",
        }
    }
}

/// What a value of `NACELLE_HASH_FRAME` means. Pure, so the parsing is
/// testable — the reader below can only be exercised once per process.
///
/// Blanks around the word are ignored: `NACELLE_HASH_FRAME="bbox "` out of
/// a shell script must not quietly become the bare hash, which answers a
/// different question and looks perfectly healthy doing it.
fn mode_of(v: Option<&str>) -> Mode {
    let v = v.map(str::trim).unwrap_or("");
    // `dump` is the older spelling of `verts` and keeps working: reports
    // and scripts written before the vocabulary existed are still valid.
    if v.eq_ignore_ascii_case("verts") || v.eq_ignore_ascii_case("dump") {
        Mode::Verts
    } else if v.eq_ignore_ascii_case("cmds") {
        Mode::Cmds
    } else if v.eq_ignore_ascii_case("bbox") {
        Mode::Bbox
    } else if v.is_empty() || v == "0" {
        Mode::Off
    } else {
        Mode::Hash
    }
}

fn mode() -> Mode {
    static MODE: OnceLock<Mode> = OnceLock::new();
    *MODE.get_or_init(|| mode_of(std::env::var("NACELLE_HASH_FRAME").ok().as_deref()))
}

/// Switches on whatever the chosen report needs to be answerable, before
/// the program builds anything. `main` calls this first.
///
/// Only `cmds` needs it: the toolkit's command register is off unless
/// armed, and a list made while it was off records nothing. Arming here
/// rather than at the first frame keeps one env var — `NACELLE_HASH_FRAME`
/// — in charge of the whole measurement, so nobody has to know that
/// `NACELLE_DRAW_CMDS` exists to get an answer out of the guard.
pub fn arm() {
    if mode() == Mode::Cmds {
        nacelle::draw::arm_cmds();
    }
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

/// A texture handle, then its value. Presence first for the same reason
/// everywhere here: a sentinel folded into the number itself would
/// collide with a real handle that happens to hold it.
fn mix_image(h: &mut u64, image: Option<nacelle::draw::ImageId>) {
    match image {
        Some(i) => {
            mix_int(h, 1);
            mix_int(h, i.0 as u64);
        }
        None => mix_int(h, 0),
    }
}

fn mix_rect(h: &mut u64, r: Option<[f32; 4]>) {
    match r {
        Some(r) => {
            mix_int(h, 1);
            for v in r {
                mix(h, v);
            }
        }
        None => mix_int(h, 0),
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
        mix_image(&mut h, r.image);
        mix_int(&mut h, r.end as u64);
        mix_rect(&mut h, r.clip);
    }
    h
}

/// The frame's outlines as one number: per command, what it samples,
/// what cuts it, and the rectangle it occupies.
///
/// Deliberately NOT the vertex digest with fewer digits. Nothing here
/// counts vertices or reads a vertex index, so a change that rebuilds the
/// same shape out of a different number of triangles leaves this number
/// alone — which is the whole reason to have a second one.
///
/// The command is the run until the toolkit's command register exists;
/// see the module header for what that costs in resolution.
pub fn bbox_digest(dl: &DrawList) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    mix_int(&mut h, dl.runs.len() as u64);
    for (r, (a, b)) in dl.runs.iter().zip(spans(&dl.runs, dl.verts.len())) {
        mix_image(&mut h, r.image);
        mix_rect(&mut h, r.clip);
        mix_rect(&mut h, bounds(&dl.verts[a..b]));
    }
    h
}

/// The frame's intent as one number: every command's canonical line, in
/// call order.
///
/// Hashing the TEXT rather than the fields is deliberate. `DrawCmd`'s
/// `Display` already quantises every number to a fixed grain so that two
/// runs of one scene print byte for byte alike; hashing anything else
/// would invent a second rounding rule, and the day the two disagreed
/// the dump and the digest would tell different stories about the same
/// frame.
pub fn cmds_digest(dl: &DrawList) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    mix_int(&mut h, dl.cmds().len() as u64);
    let mut line = String::new();
    for c in dl.cmds() {
        line.clear();
        let _ = write!(line, "{c}");
        // The length first, for the same reason the list's two lengths
        // go in first: without it two commands could be split
        // differently and feed one identical byte stream.
        mix_int(&mut h, line.len() as u64);
        for b in line.bytes() {
            mix_int(&mut h, b as u64);
        }
    }
    h
}

/// Where each run starts and ends in the vertex list. Runs partition the
/// list in emission order — `end` is one past the last vertex, and a run
/// begins where the previous one ended.
///
/// Clamped anyway. The guard is the thing that gets armed when something
/// is already suspected of being wrong, and a report that panics on a
/// malformed list tells nobody anything.
fn spans(runs: &[DrawRun], verts: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(runs.len());
    let mut start = 0usize;
    for r in runs {
        let end = (r.end as usize).clamp(start, verts);
        out.push((start, end));
        start = end;
    }
    out
}

/// The rectangle enclosing a slice of vertices, as `[x0, y0, x1, y1]`.
///
/// `None` for an empty slice rather than a rectangle at the origin: a
/// command that drew nothing has no place, and giving it `0,0,0,0` would
/// plant it in the top-left corner where a real one could be — turning
/// "it vanished" into "it moved", or hiding it entirely.
fn bounds(vs: &[Vertex]) -> Option<[f32; 4]> {
    let mut it = vs.iter();
    let first = it.next()?;
    let mut b = [first.pos[0], first.pos[1], first.pos[0], first.pos[1]];
    for v in it {
        b[0] = b[0].min(v.pos[0]);
        b[1] = b[1].min(v.pos[1]);
        b[2] = b[2].max(v.pos[0]);
        b[3] = b[3].max(v.pos[1]);
    }
    Some(b)
}

/// A coordinate at the guard's grain — a thousandth of a pixel, the same
/// one the hash rounds to — with negative zero folded onto zero.
///
/// `-0.0` and `0.0` are the same place, but they print differently, and
/// which one a subtraction lands on depends on the order the terms were
/// added. Without this, two frames drawn identically could differ in a
/// sign no pixel carries.
fn px(v: f32) -> f32 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn rect_text(r: Option<[f32; 4]>) -> String {
    match r {
        Some([a, b, c, d]) => {
            format!("{:.3} {:.3} {:.3} {:.3}", px(a), px(b), px(c), px(d))
        }
        None => "none".to_string(),
    }
}

/// A run's texture as a number, with `-1` for the glyph atlas — the
/// handle band starts at zero, so no real image can be mistaken for it.
fn image_text(image: Option<nacelle::draw::ImageId>) -> i64 {
    image.map(|i| i.0 as i64).unwrap_or(-1)
}

/// Writes one frame's report and answers with the status the process
/// should leave with. Separated from [`observe`] so the format can be
/// tested: `observe` may run once per process and ends it.
fn report(out: &mut dyn Write, n: u64, m: Mode, dl: &DrawList) -> i32 {
    // The one question this build can be asked and fail to answer. Said
    // on the header line, not in the body, so a reader who only ever
    // looks at the first line of a report cannot mistake the apology for
    // a measurement — and said before anything else is written, so the
    // file holds no half-answer either.
    if m == Mode::Cmds && !dl.is_recording() {
        let _ = writeln!(
            out,
            "frame {} mode {} unavailable: this frame's draw list carries no \
             command register",
            n,
            m.word()
        );
        eprintln!(
            "nacelle-desktop: hash frame: cmds asked for, but the frame's draw \
             list was built before the register was armed"
        );
        return EXIT_UNANSWERED;
    }

    match m {
        // The vertex reports carry the vertex counts and the vertex
        // digest. The coarse ones must not: a retessellation moves both,
        // and a header that moved would sink a diff whose whole point is
        // that the body did not.
        Mode::Off | Mode::Hash | Mode::Verts => {
            let _ = writeln!(
                out,
                "frame {} mode {} verts {} runs {} hash {:016x}",
                n,
                m.word(),
                dl.verts.len(),
                dl.runs.len(),
                digest(dl)
            );
        }
        Mode::Bbox => {
            let _ = writeln!(
                out,
                "frame {} mode {} runs {} hash {:016x}",
                n,
                m.word(),
                dl.runs.len(),
                bbox_digest(dl)
            );
        }
        Mode::Cmds => {
            // No run count here either. Runs are a batching decision —
            // a texture swap splits one — and a tessellation core is
            // free to move them without meaning to draw anything else.
            let _ = writeln!(
                out,
                "frame {} mode {} cmds {} hash {:016x}",
                n,
                m.word(),
                dl.cmds().len(),
                cmds_digest(dl)
            );
        }
    }

    match m {
        Mode::Off | Mode::Hash => 0,
        Mode::Verts => {
            for (i, r) in dl.runs.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "run {i} image {} end {} clip {}",
                    image_text(r.image),
                    r.end,
                    rect_text(r.clip)
                );
            }
            for (i, v) in dl.verts.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "v {i} {:.3} {:.3} {:.6} {:.6} {:.4} {:.4} {:.4} {:.4}",
                    px(v.pos[0]),
                    px(v.pos[1]),
                    v.uv[0],
                    v.uv[1],
                    v.color[0],
                    v.color[1],
                    v.color[2],
                    v.color[3]
                );
            }
            0
        }
        Mode::Bbox => {
            let mut all: Option<[f32; 4]> = None;
            for ((i, r), (a, b)) in
                dl.runs.iter().enumerate().zip(spans(&dl.runs, dl.verts.len()))
            {
                let box_ = bounds(&dl.verts[a..b]);
                if let Some(v) = box_ {
                    all = Some(match all {
                        Some(u) => [u[0].min(v[0]), u[1].min(v[1]), u[2].max(v[2]), u[3].max(v[3])],
                        None => v,
                    });
                }
                let _ = writeln!(
                    out,
                    "bbox {i} image {} clip {} box {}",
                    image_text(r.image),
                    rect_text(r.clip),
                    rect_text(box_)
                );
            }
            // The frame's own outline, last: the one line worth reading
            // when a whole board slid by a pixel.
            let _ = writeln!(out, "bbox all box {}", rect_text(all));
            0
        }
        Mode::Cmds => {
            // One command per line, numbered here rather than by the
            // toolkit: the register's canonical form is the line, and
            // where it sits in the frame is the guard's business.
            for (i, c) in dl.cmds().iter().enumerate() {
                let _ = writeln!(out, "cmd {i} {c}");
            }
            0
        }
    }
}

/// Where the report goes. A file when `NACELLE_HASH_FRAME_OUT` names one,
/// stdout otherwise — including when the file cannot be made, because a
/// measurement printed in the wrong place is still a measurement, and one
/// swallowed by a bad path is not.
fn destination(spec: Option<&str>) -> Box<dyn Write> {
    match spec {
        Some(p) => match std::fs::File::create(p) {
            Ok(f) => Box::new(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("nacelle-desktop: hash frame: {p}: {e}");
                Box::new(std::io::stdout())
            }
        },
        None => Box::new(std::io::stdout()),
    }
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

    let mut out = destination(std::env::var("NACELLE_HASH_FRAME_OUT").ok().as_deref());
    let code = report(&mut *out, n, m, dl);
    let _ = out.flush();
    drop(out);
    std::process::exit(code);
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

    /// The same drawing on a list that records its commands. `recording`
    /// rather than the environment: a test may not arm a process-wide,
    /// one-way switch that every other test in the binary then inherits.
    fn taped() -> DrawList {
        let mut dl = DrawList::recording();
        dl.rect(10.0, 20.0, 30.0, 40.0, ink());
        dl.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        dl
    }

    fn text(m: Mode, dl: &DrawList) -> String {
        let mut buf: Vec<u8> = Vec::new();
        report(&mut buf, 0, m, dl);
        String::from_utf8(buf).expect("the report is plain ASCII")
    }

    #[test]
    fn the_word_decides_the_mode_and_anything_else_is_off() {
        assert_eq!(mode_of(None), Mode::Off);
        assert_eq!(mode_of(Some("")), Mode::Off);
        assert_eq!(mode_of(Some("0")), Mode::Off);
        assert_eq!(mode_of(Some("1")), Mode::Hash);
        assert_eq!(mode_of(Some("yes")), Mode::Hash);
        assert_eq!(mode_of(Some("dump")), Mode::Verts);
        assert_eq!(mode_of(Some("DUMP")), Mode::Verts);
        assert_eq!(mode_of(Some("verts")), Mode::Verts);
        assert_eq!(mode_of(Some("cmds")), Mode::Cmds);
        assert_eq!(mode_of(Some("bbox")), Mode::Bbox);
        assert_eq!(mode_of(Some("BBox")), Mode::Bbox);
    }

    /// A word that arrived with a stray space is still that word. The
    /// failure this forbids is silent: `bbox ` would otherwise select the
    /// bare hash and produce a report that looks entirely reasonable.
    #[test]
    fn blanks_around_the_word_do_not_change_which_question_is_asked() {
        assert_eq!(mode_of(Some(" bbox ")), Mode::Bbox);
        assert_eq!(mode_of(Some("\tverts\n")), Mode::Verts);
        assert_eq!(mode_of(Some("  ")), Mode::Off);
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
        assert_eq!(bbox_digest(&list()), bbox_digest(&list()));
        assert_eq!(bbox_digest(&DrawList::new()), bbox_digest(&DrawList::new()));
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

    // -- the bounding boxes ------------------------------------------

    #[test]
    fn a_box_is_the_corners_of_what_was_drawn() {
        let mut dl = DrawList::new();
        dl.rect(10.0, 20.0, 30.0, 40.0, ink());
        assert_eq!(bounds(&dl.verts), Some([10.0, 20.0, 40.0, 60.0]));
        assert_eq!(bounds(&[]), None);
    }

    /// The measure the whole bbox report exists for: same picture, twice
    /// as many triangles, one unchanged answer. `verts` must disagree in
    /// the same breath, or the two reports are one report.
    #[test]
    fn retessellating_a_shape_leaves_its_box_alone() {
        let mut whole = DrawList::new();
        whole.rect(10.0, 20.0, 30.0, 40.0, ink());

        let mut halves = DrawList::new();
        halves.rect(10.0, 20.0, 15.0, 40.0, ink());
        halves.rect(25.0, 20.0, 15.0, 40.0, ink());

        assert_eq!(bbox_digest(&whole), bbox_digest(&halves));
        assert_eq!(text(Mode::Bbox, &whole), text(Mode::Bbox, &halves));
        assert_ne!(digest(&whole), digest(&halves), "the vertex report must still see it");
    }

    /// And the other half of the bargain: coarse is not blind.
    #[test]
    fn a_shape_that_moved_grew_or_vanished_changes_its_box() {
        let one = |x: f32, w: f32| {
            let mut dl = DrawList::new();
            dl.rect(x, 20.0, w, 40.0, ink());
            dl
        };
        let base = one(10.0, 30.0);
        assert_ne!(bbox_digest(&base), bbox_digest(&one(11.0, 30.0)), "moved");
        assert_ne!(bbox_digest(&base), bbox_digest(&one(10.0, 31.0)), "grew");
        assert_ne!(bbox_digest(&base), bbox_digest(&DrawList::new()), "vanished");
    }

    /// The reach of this report is exactly the run structure, and no
    /// further: a shape that moves inside the span of another one drawn
    /// with the same texture and the same scissor never leaves the run's
    /// box. Written down as a test rather than left to be discovered in
    /// the middle of a regression hunt — and it is the concrete reason
    /// the command register is the unit this mode wants.
    #[test]
    fn the_box_report_is_only_as_fine_as_the_runs_are() {
        let mut base = DrawList::new();
        base.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        base.rect(10.0, 20.0, 30.0, 40.0, ink());

        let mut moved = DrawList::new();
        moved.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        moved.rect(11.0, 20.0, 30.0, 40.0, ink());

        assert_eq!(base.runs.len(), 1, "one texture, one scissor, one run");
        assert_eq!(bbox_digest(&base), bbox_digest(&moved));
        assert_ne!(digest(&base), digest(&moved), "the vertex report still sees it");
    }

    /// A scissor decides where a command lands on screen as surely as its
    /// vertices do, so it belongs to the question the box mode answers.
    #[test]
    fn the_scissor_is_part_of_where_a_thing_is() {
        let mut clipped = DrawList::new();
        clipped.push_clip(0.0, 0.0, 20.0, 20.0);
        clipped.rect(10.0, 20.0, 30.0, 40.0, ink());
        clipped.pop_clip();

        let mut open = DrawList::new();
        open.rect(10.0, 20.0, 30.0, 40.0, ink());

        assert_ne!(bbox_digest(&clipped), bbox_digest(&open));
        assert!(text(Mode::Bbox, &clipped).contains("clip 0.000 0.000 20.000 20.000"));
        assert!(text(Mode::Bbox, &open).contains("clip none"));
    }

    /// A run that emitted nothing is reported as having no place, not as
    /// a point at the origin — where a real command could be.
    #[test]
    fn a_command_that_drew_nothing_has_no_box() {
        let mut dl = DrawList::new();
        dl.rect(10.0, 20.0, 0.0, 0.0, ink());
        dl.push_clip(0.0, 0.0, 5.0, 5.0);
        dl.pop_clip();
        let out = text(Mode::Bbox, &dl);
        assert!(out.contains("box none"), "{out}");
    }

    #[test]
    fn runs_partition_the_vertices_and_a_malformed_end_cannot_panic() {
        let dl = list();
        let s = spans(&dl.runs, dl.verts.len());
        assert_eq!(s.first().map(|r| r.0), Some(0));
        assert_eq!(s.last().map(|r| r.1), Some(dl.verts.len()));
        for w in s.windows(2) {
            assert_eq!(w[0].1, w[1].0);
        }

        let liar = [DrawRun { image: None, end: u32::MAX, clip: None }];
        assert_eq!(spans(&liar, 4), vec![(0, 4)]);
    }

    /// Negative zero is the same place as zero, and the guard prints it
    /// that way — otherwise a subtraction that came out on the other side
    /// would make two identical frames diff.
    #[test]
    fn negative_zero_is_zero() {
        assert_eq!(format!("{:.3}", px(-0.0)), "0.000");
        assert_eq!(format!("{:.3}", px(-0.0001)), "0.000");
        assert_eq!(format!("{:.3}", px(-1.5)), "-1.500");
    }

    // -- the reports themselves --------------------------------------

    /// Two runs of the same build on the same frame must produce the same
    /// text, byte for byte, in every mode — a report that is only nearly
    /// reproducible cannot be diffed, and a guard that cannot be diffed
    /// is a log.
    #[test]
    fn a_report_is_the_same_text_every_time() {
        for m in [Mode::Hash, Mode::Verts, Mode::Cmds, Mode::Bbox] {
            assert_eq!(text(m, &list()), text(m, &list()), "{m:?}");
            assert_eq!(text(m, &DrawList::new()), text(m, &DrawList::new()), "{m:?}");
        }
    }

    /// Every report says which question it answered, so a file found on
    /// disk a month later — or one produced by a word the guard did not
    /// recognise — cannot be read as the wrong measurement.
    #[test]
    fn every_report_names_its_own_mode() {
        for m in [Mode::Hash, Mode::Verts, Mode::Cmds, Mode::Bbox] {
            let out = text(m, &list());
            let head = out.lines().next().unwrap_or_default();
            assert!(head.starts_with("frame 0 mode "), "{head}");
            assert!(head.contains(&format!("mode {}", m.word())), "{head}");
        }
    }

    /// The coarse reports must not carry a vertex count or the vertex
    /// hash, or a deliberate retessellation would move their header and
    /// bury the body it was supposed to vindicate.
    #[test]
    fn the_coarse_reports_count_no_vertices() {
        for m in [Mode::Cmds, Mode::Bbox] {
            let head = text(m, &list()).lines().next().unwrap_or_default().to_string();
            assert!(!head.contains("verts"), "{head}");
            assert!(!head.contains(&format!("{:016x}", digest(&list()))), "{head}");
        }
    }

    /// A question the build cannot answer leaves a non-zero status. A
    /// script that only diffs two output files would otherwise read two
    /// identical apologies as proof the frame never changed.
    #[test]
    fn a_question_the_guard_cannot_answer_is_not_a_pass() {
        let mut buf: Vec<u8> = Vec::new();
        assert_eq!(report(&mut buf, 0, Mode::Cmds, &list()), EXIT_UNANSWERED);
        assert!(String::from_utf8_lossy(&buf).contains("unavailable"));

        let mut buf: Vec<u8> = Vec::new();
        assert_eq!(report(&mut buf, 0, Mode::Bbox, &list()), 0);

        // And with a register there, the same question is answerable.
        let mut buf: Vec<u8> = Vec::new();
        assert_eq!(report(&mut buf, 0, Mode::Cmds, &taped()), 0);
        assert!(!String::from_utf8_lossy(&buf).contains("unavailable"));
    }

    // -- the command register ----------------------------------------

    /// The register reports the intent, one numbered line per call, and
    /// the same drawing twice is the same text — the property every
    /// comparison downstream is built on.
    #[test]
    fn the_command_report_is_one_line_per_call_in_call_order() {
        let out = text(Mode::Cmds, &taped());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], format!("frame 0 mode cmds cmds 2 hash {:016x}", cmds_digest(&taped())));
        assert!(lines[1].starts_with("cmd 0 rect "), "{}", lines[1]);
        assert!(lines[2].starts_with("cmd 1 line "), "{}", lines[2]);
        assert_eq!(lines.len(), 3);
        assert_eq!(out, text(Mode::Cmds, &taped()));
    }

    /// The whole reason the register exists, in the one shape the image
    /// phase will actually deliver: a shape rebuilt out of a different
    /// number of triangles is the same command. Proved here on the
    /// register's own terms — a ring drawn at two different segment
    /// counts is what an SDF core does to every rounded panel.
    #[test]
    fn retessellating_a_shape_leaves_the_register_alone() {
        use nacelle::base::Rect;
        use nacelle::draw::Corner;
        let r = Rect::new(10.0, 20.0, 200.0, 100.0);
        let c = [Corner::round(12.0); 4];
        let ring = |segments: u8| {
            let mut dl = DrawList::recording();
            dl.ring(r, &c, segments, 2.0, ink());
            dl.ring_fill(r, &c, segments, ink());
            dl
        };
        let coarse = ring(3);
        let fine = ring(12);

        assert!(fine.verts.len() > coarse.verts.len(), "the two must actually differ");
        assert_ne!(digest(&coarse), digest(&fine), "the vertex report must see it");
        assert_eq!(cmds_digest(&coarse), cmds_digest(&fine));
        assert_eq!(text(Mode::Cmds, &coarse), text(Mode::Cmds, &fine));
    }

    /// And the other half of the bargain, the half that makes a clean
    /// `cmds` diff worth reading: intent that changed is seen. A recolour
    /// moves no vertex position at all, and a move inside one run is
    /// invisible to `bbox` — the register catches both.
    #[test]
    fn the_register_sees_a_recolour_and_a_move_inside_a_run() {
        let base = taped();

        let mut recoloured = DrawList::recording();
        recoloured.rect(10.0, 20.0, 30.0, 40.0, Color { a: 0.99, ..ink() });
        recoloured.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        assert_ne!(cmds_digest(&base), cmds_digest(&recoloured));

        let mut moved = DrawList::recording();
        moved.rect(11.0, 20.0, 30.0, 40.0, ink());
        moved.line(0.0, 0.0, 100.0, 100.0, 2.0, ink());
        assert_eq!(bbox_digest(&base), bbox_digest(&moved), "one run, one box");
        assert_ne!(cmds_digest(&base), cmds_digest(&moved), "the register is finer");
    }

    /// The report reaches the file the caller named, and a path that
    /// cannot be opened falls back instead of taking the program down.
    #[test]
    fn the_report_lands_in_the_file_that_was_asked_for() {
        let path = std::env::temp_dir()
            .join(format!("nacelle-hashframe-{}.txt", std::process::id()));
        let name = path.to_string_lossy().to_string();
        {
            let mut out = destination(Some(&name));
            report(&mut *out, 7, Mode::Bbox, &list());
            let _ = out.flush();
        }
        let got = std::fs::read_to_string(&path).expect("the guard made the file");
        assert_eq!(got, text(Mode::Bbox, &list()).replace("frame 0", "frame 7"));
        let _ = std::fs::remove_file(&path);

        // A directory is never openable as a file: the fallback must
        // hand back a sink, not unwind through the frame being measured.
        let _ = destination(Some("/"));
    }
}
