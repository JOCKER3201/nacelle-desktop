//! The one reader of `render.vector` — the token that decides whether a
//! silhouette is tessellated into a ring of triangles or solved as a
//! signed distance field (f3 §6, step K3a).
//!
//! # Why the reader is here and not in the toolkit
//!
//! `DrawList::set_vector` has existed since the SDF core landed, and
//! until this module nothing but the toolkit's own tests called it, so
//! `render.vector = true` and `= false` drew exactly the same picture.
//! The token's comment in the master says so in as many words.
//!
//! The reader could not be put in `libnacelle`. In production no draw
//! list belongs to the toolkit: the host builds one per screen, clears
//! it per frame and hands `shapes()` to the renderer. Reading the token
//! inside `DrawList::clear` would have been the right file and the
//! wrong place — it would turn a MODE into frame state and ask the
//! theme a question sixty times a second on behalf of an object that
//! reads no tokens at all by design. So the answer lives with whoever
//! owns the list, which is this crate.
//!
//! # Why the arming is not a one-off in the constructor
//!
//! A theme can be loaded at any moment — the settings window loads one,
//! a mood swaps a sibling, `--theme` at start-up is only the first of
//! them — and `render.vector` is a token like any other, so the answer
//! can change while the program runs. A host that armed its list once,
//! where the list is built, would draw the theme it started with
//! forever. Hence [`Lane`], which is asked at every frame boundary and
//! re-arms the list when the answer has moved.
//!
//! # Why the list and the arming are one object
//!
//! [`FrameList`] exists because the first shape of this wiring was
//! three loose lines — arm where the list is built, arm where the frame
//! takes it over, arm in the dialog's own loop — and a loose line is
//! exactly what a merge drops without anything saying so: the program
//! keeps building, keeps drawing, and silently draws every silhouette
//! on the lane the theme did not ask for. So the list is not reachable
//! except through [`FrameList::begin`], which empties it and arms it in
//! one move. Losing the arming now means losing the list with it, which
//! is a question the compiler asks instead of a reviewer.
//!
//! # Why the question is asked per frame, and what it costs
//!
//! Reading the flag is `ACTIVE.load(Acquire)` followed by one bounds
//! checked slice index — the same cost class as the atomic load the
//! pixel guard already pays on every frame, and cheaper than the
//! `Instant::now()` beside it. `DrawList::set_vector` itself is called
//! only when the answer CHANGED, which is what keeps this a mode.
//!
//! The obvious alternative — gate the read on `theme::content_epoch()`
//! and skip even that — was weighed and rejected, because that counter
//! deliberately does not move for a PREVIEW: `theme::set_preview`
//! publishes a fresh bake and leaves the content counter where it is,
//! so that the font system it guards does not walk the font directories
//! behind a slider that pulses ten times a second. (The counter exists
//! at all because that guard used to be `theme::epoch()`, which on a
//! desktop of unequal screen heights alternates every frame by design
//! and so put `--desktop` at 100 % CPU — the epoch's doing, not the
//! editor's.) Gating on it would leave a live-previewed `render.vector`
//! silently ignored, which is a hole with no upside when the un-gated
//! read is two instructions.

use nacelle::draw::DrawList;
use nacelle::theme::{self, TokenId};
use std::sync::OnceLock;

/// The token id, resolved once per process. A master that does not
/// declare the name degrades to the flag kind's own fallback — `false`,
/// the tessellated lane — rather than to a number somebody remembered.
fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

/// Whether the running theme asks for the vector lane.
///
/// Private, and staying private until something outside this module has
/// a reason to ask: the lane is a property of a list, and everything
/// that owns a list here owns a [`FrameList`] that answers for it.
fn wanted() -> bool {
    static VECTOR: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().flag(tok(&VECTOR, "render.vector"))
}

/// One draw list's arming, remembered between frames.
///
/// There is one of these per list, which on this desktop means one per
/// screen: two monitors are two lists and the mode is a property of a
/// list, not of the process.
#[derive(Default, Debug)]
struct Lane {
    /// What the list was last told, or `None` while it has never been
    /// told anything. `None` rather than `false` on purpose: a fresh
    /// list is already tessellating, but "nobody has answered yet" and
    /// "the theme answered no" are different states, and only the first
    /// of them must arm unconditionally.
    armed: Option<bool>,
}

impl Lane {
    const fn new() -> Self {
        Lane { armed: None }
    }

    /// Brings `dl` into line with the running theme, and says whether
    /// it had to.
    ///
    /// Called at the frame boundary, after `clear()`. `clear()` does not
    /// disturb the mode — the toolkit tests that — so a steady frame
    /// answers `false` here and touches nothing.
    fn arm(&mut self, dl: &mut DrawList) -> bool {
        let want = wanted();
        if self.armed == Some(want) {
            return false;
        }
        self.armed = Some(want);
        dl.set_vector(want);
        true
    }
}

/// A draw list and the lane it is armed on, kept between frames.
///
/// One per surface that draws with the toolkit: one per screen, and one
/// for the resolution dialog, which hands `shapes()` to the same
/// renderer and would otherwise be a second answer to the same token.
pub struct FrameList {
    /// The list between frames, kept so a steady frame allocates
    /// nothing. While a frame holds it this is an empty stand-in — see
    /// `out`.
    dl: DrawList,
    /// True from [`FrameList::begin`] until [`FrameList::end`] takes the
    /// list back. It is how a second `begin` learns that the lane
    /// remembers a list that never came home: a frame that returns early
    /// takes the memory of its arming with it, and without this the next
    /// frame would be handed a fresh list the lane believes it has
    /// already armed.
    out: bool,
    lane: Lane,
}

impl FrameList {
    pub fn new() -> Self {
        FrameList { dl: DrawList::new(), out: false, lane: Lane::new() }
    }

    /// The list for the frame about to be drawn: empty, armed against
    /// the theme running right now, and carrying the capacity the last
    /// frame earned.
    ///
    /// It goes out by value because a frame holds the list and the
    /// screen the list belongs to at the same time; [`FrameList::end`]
    /// puts it back.
    pub fn begin(&mut self) -> DrawList {
        let mut dl = std::mem::replace(&mut self.dl, DrawList::new());
        if self.out {
            // What is in hand is the stand-in, not the list the lane
            // remembers arming. Forgetting is what makes the arming
            // below unconditional again.
            self.lane = Lane::new();
        }
        self.out = true;
        dl.clear();
        self.lane.arm(&mut dl);
        dl
    }

    /// Takes the drawn list back, capacity and all.
    pub fn end(&mut self, dl: DrawList) {
        self.dl = dl;
        self.out = false;
    }

    /// What the last finished frame drew — the list the renderer is
    /// handed. Empty while a frame holds it.
    pub fn list(&self) -> &DrawList {
        &self.dl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{theme_test_lock, Themed};
    use nacelle::base::Rect;
    use nacelle::draw::Corner;
    use nacelle::theme::Color;

    /// One framed surface, spelled the way the whole toolkit spells one:
    /// a bed, then a border over it.
    fn panel(dl: &mut DrawList) {
        let r = Rect::new(10.0, 20.0, 200.0, 100.0);
        let c = [Corner::round(6.5); 4];
        dl.ring_fill(r, &c, 16, Color::rgba8(20, 30, 40, 190));
        dl.ring(r, &c, 16, 1.0, Color::rgba8(230, 210, 120, 220));
    }

    /// A shape RECORD is the whole of the difference: on the vector lane
    /// a panel is one record and one quad, on the old lane it is a fan
    /// of triangles and no record at all. So `shape_len()` is the
    /// yes/no this test needs, and it needs no GPU to ask it.
    fn drew_shapes(dl: &DrawList) -> usize {
        dl.shape_len()
    }

    /// The token DOES something — which, before this module existed, it
    /// did not. Both directions, because "the flag is ignored" and "the
    /// flag is stuck on" are both failures and only one of them is
    /// visible from a single case.
    #[test]
    fn the_token_decides_which_lane_a_panel_takes() {
        let _lock = theme_test_lock();

        {
            let _theme = Themed::new("vector-on", "[render]\nvector = true\n");
            assert!(wanted(), "the fixture theme asks for the vector lane");
            let mut frame = FrameList::new();
            let mut dl = frame.begin();
            panel(&mut dl);
            assert_eq!(
                drew_shapes(&dl),
                1,
                "render.vector = true must put the panel on the shape lane"
            );
            frame.end(dl);
        }

        let _theme = Themed::new("vector-off", "[render]\nvector = false\n");
        assert!(!wanted());
        let mut frame = FrameList::new();
        let mut dl = frame.begin();
        panel(&mut dl);
        assert_eq!(
            drew_shapes(&dl),
            0,
            "render.vector = false must keep the tessellated lane"
        );
        assert!(
            dl.verts.len() > 100,
            "the tessellated lane spends a ring of triangles, not a quad"
        );
    }

    /// What [`FrameList::begin`] promises the frame that calls it, and
    /// the reason the promise is one call instead of two lines: the list
    /// arrives EMPTY and on the lane the theme asks for RIGHT NOW.
    ///
    /// The order is the one every screen sees — the list is built while
    /// the master is running, and the theme that raises the switch is
    /// loaded afterwards — so a `begin` that handed out the list without
    /// asking the theme again would be caught here and nowhere else.
    #[test]
    fn a_frame_opens_on_an_empty_list_armed_by_the_running_theme() {
        let _lock = theme_test_lock();
        // Built under the master, which ships the switch down.
        let mut frame = FrameList::new();

        let _theme = Themed::new("late-load", "[render]\nvector = true\n");
        let mut dl = frame.begin();
        assert!(dl.verts.is_empty(), "a frame opens on an empty list");
        panel(&mut dl);
        assert_eq!(
            drew_shapes(&dl),
            1,
            "the list must be armed by the theme running when the frame opened"
        );
        frame.end(dl);

        // And the frame after it: the geometry of the frame before is
        // gone, the lane it was on is not.
        let mut dl = frame.begin();
        assert!(dl.verts.is_empty(), "the next frame opens on an empty list too");
        assert_eq!(drew_shapes(&dl), 0, "and on an empty record table");
        panel(&mut dl);
        assert_eq!(drew_shapes(&dl), 1, "a steady frame does not leave the lane");
        frame.end(dl);
    }

    /// The condition f3 §6 K3 calls the easy one to miss: the mode is set
    /// at EVERY theme load, not once where the list is built. A host that
    /// armed its list in the constructor would draw the theme it started
    /// with for the rest of the session, and no amount of loading would
    /// move it.
    ///
    /// The `FrameList` here is the one from the first theme, kept across
    /// the swap exactly as a screen keeps its own between frames.
    #[test]
    fn the_list_survives_a_theme_swap() {
        let _lock = theme_test_lock();
        let mut frame = FrameList::new();

        {
            let _theme = Themed::new("swap-off", "[render]\nvector = false\n");
            let mut dl = frame.begin();
            panel(&mut dl);
            assert_eq!(drew_shapes(&dl), 0, "the first theme says no");
            frame.end(dl);
        }

        // A second theme is loaded under the SAME list — the swap the
        // settings window makes, and the one a mood makes.
        {
            let _theme = Themed::new("swap-on", "[render]\nvector = true\n");
            let mut dl = frame.begin();
            panel(&mut dl);
            assert_eq!(
                drew_shapes(&dl),
                1,
                "the list must follow the theme that was loaded after it was built"
            );
            frame.end(dl);
        }

        // And back, because a switch that only latches one way is not a
        // switch. Dropping the fixture above already restored the master
        // — whose `render.vector` is true as of K3d (2026-08-23) — so
        // this is the master's own answer arriving at a list that had
        // been told otherwise.
        let mut dl = frame.begin();
        panel(&mut dl);
        assert_eq!(drew_shapes(&dl), 1, "the master ships the vector lane");
        frame.end(dl);
    }

    /// A mode is not frame state: clearing the list for the next frame
    /// must not disarm it, and a steady frame must not have to re-arm.
    ///
    /// The second half is the one that would go unnoticed: an `arm` that
    /// answered `true` every frame would still draw correctly and would
    /// silently drop the weld the toolkit uses to fuse a bed, its wash
    /// and its border into one record — `set_vector` resets it. So this
    /// one asks the lane directly, which is the only place the answer
    /// "did it have to" exists.
    #[test]
    fn a_frame_boundary_is_not_a_theme_load() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("steady", "[render]\nvector = true\n");
        let mut dl = DrawList::new();
        let mut lane = Lane::new();
        assert!(lane.arm(&mut dl), "a fresh lane must arm unconditionally");
        for frame in 0..4 {
            dl.clear();
            assert!(
                !lane.arm(&mut dl),
                "frame {frame} re-armed a list whose theme did not move"
            );
            panel(&mut dl);
            assert_eq!(drew_shapes(&dl), 1, "frame {frame} left the lane");
        }
    }

    /// The other end of the same memo: a list that was handed out and
    /// never came back takes the lane's memory of arming it with it.
    ///
    /// This is what a frame that returns early looks like from here, and
    /// without the `out` flag it would be the quiet failure — the next
    /// frame gets a FRESH list, the lane still believes it armed one,
    /// and the desktop draws the rest of the session on the wrong lane
    /// while every test that owns its own list keeps passing.
    #[test]
    fn a_list_that_never_came_back_is_armed_from_scratch() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("early-out", "[render]\nvector = true\n");
        let mut frame = FrameList::new();

        let dl = frame.begin();
        drop(dl); // the frame returned before `end`

        let mut dl = frame.begin();
        panel(&mut dl);
        assert_eq!(
            drew_shapes(&dl),
            1,
            "the list that replaced the lost one was never told which lane it is on"
        );
    }

    /// The shipping answer, stated as a test rather than as a promise:
    /// the master raises the switch as of K3d (2026-08-23), a decision
    /// taken on the measurement in `.gap-program/pomiar-wektor-k3c.md`
    /// and `.gap-program/pomiar-wektor-k3d.md`, and not by whoever wires
    /// the reader.
    #[test]
    fn the_master_ships_the_switch_up() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("plain", "");
        assert!(
            wanted(),
            "render.vector went back down without a K3d reversal; \
             see .gap-program/pomiar-wektor-k3d.md"
        );
    }

    /// **K3c, measurement 3: what a REAL frame of this program costs in
    /// draw calls on each lane, read off the pixel guard's own report.**
    ///
    /// The number comes out of `NACELLE_HASH_FRAME`'s `verts` mode —
    /// the same `report` an armed run writes — because a figure quoted
    /// in `.gap-program/pomiar-wektor-k3c.md` has to be the figure the
    /// tool prints, not a paraphrase of it. What is NOT taken here is a
    /// dump of the live desktop: that wants a display session and a
    /// Vulkan device, and this measurement is meant to run in `cargo
    /// test` on a machine with neither.
    ///
    /// Two drawers, both real chrome and both reachable without a
    /// screen: the resolution dialog (a framed surface, its buttons and
    /// its text) and the editor's button stack (`nacelle::object::button`
    /// — the toolkit's own control, drawn by the library).
    ///
    /// **What it shows.** A drawer that spends a handful of silhouettes
    /// pays a handful of runs, and the vertex saving is real; the price
    /// is that every silhouette breaks the run its neighbours were
    /// sharing. On the dense synthetic board in
    /// `libnacelle/src/sdf.rs` (204 silhouettes) that arithmetic ends at
    /// 408 runs against 1. Here it is small because the drawers are
    /// small — which is the honest shape of the finding, not a
    /// contradiction of it.
    #[test]
    fn a_real_frame_reports_its_runs_on_both_lanes() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("k3c-runs", "");

        let numbers = |line: &str| -> (usize, usize) {
            let f: Vec<&str> = line.split_whitespace().collect();
            // `rposition`, because the header names the MODE before it
            // names the counts and the mode's word is `verts` too:
            // "frame 0 mode verts verts 774 runs 1 hash ...".
            let at = |k: &str| {
                f.iter()
                    .rposition(|w| *w == k)
                    .and_then(|i| f.get(i + 1))
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or_else(|| panic!("the verts header has no {k}: {line}"))
            };
            (at("verts"), at("runs"))
        };
        let measure = |name: &str, vector: bool| {
            let dl = crate::widgets::drawn_list(1080.0, 0.0, vector, |ctx| match name {
                "dialog" => crate::widgets::popup::draw_resolution_dialog(ctx, 1920, 1080),
                _ => {
                    let mut ed =
                        crate::widgets::editor::Editor::new(crate::screen::Gutter::of_test(8.0));
                    ed.draw_buttons(ctx);
                }
            });
            let head = crate::hashframe::report_text("verts", &dl);
            let line = head.lines().next().expect("the report has a header").to_string();
            let (verts, runs) = numbers(&line);
            (verts, runs, dl.shape_len())
        };

        for name in ["dialog", "buttons"] {
            let (v_old, r_old, s_old) = measure(name, false);
            let (v_new, r_new, s_new) = measure(name, true);
            assert_eq!(s_old, 0, "{name}: the tessellated lane writes no records");
            assert!(s_new > 0, "{name}: the vector lane drew no silhouette at all");
            assert!(
                v_new < v_old,
                "{name}: the vector lane spent {v_new} vertices against {v_old} —                  the whole point of the record is that it spends fewer"
            );
            assert!(
                r_new > r_old,
                "{name}: {r_new} runs against {r_old} — the cut is supposed to COST                  runs, and a measurement that says otherwise is measuring nothing"
            );
        }
    }

}
