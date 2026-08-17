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
//! deliberately does not move for a PREVIEW. It cannot: it is what
//! guards the font-face reload, and a preview pulses ten times a second
//! behind the theme editor's sliders (moving it is how `--desktop` once
//! reached 100 % CPU). Gating on it would leave a live-previewed
//! `render.vector` silently ignored, which is a hole with no upside
//! when the un-gated read is two instructions.

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
/// Public because it is the answer to "which lane is this build
/// drawing", and a report is entitled to ask it without owning a list.
pub fn wanted() -> bool {
    static VECTOR: OnceLock<TokenId> = OnceLock::new();
    theme::resolved().flag(tok(&VECTOR, "render.vector"))
}

/// One draw list's arming, remembered between frames.
///
/// There is one of these per list, which on this desktop means one per
/// screen: two monitors are two lists and the mode is a property of a
/// list, not of the process.
#[derive(Default, Debug)]
pub struct Lane {
    /// What the list was last told, or `None` while it has never been
    /// told anything. `None` rather than `false` on purpose: a fresh
    /// list is already tessellating, but "nobody has answered yet" and
    /// "the theme answered no" are different states, and only the first
    /// of them must arm unconditionally.
    armed: Option<bool>,
}

impl Lane {
    pub const fn new() -> Self {
        Lane { armed: None }
    }

    /// Brings `dl` into line with the running theme, and says whether
    /// it had to.
    ///
    /// Call at the frame boundary, after `clear()`. `clear()` does not
    /// disturb the mode — the toolkit tests that — so a steady frame
    /// answers `false` here and touches nothing.
    pub fn arm(&mut self, dl: &mut DrawList) -> bool {
        let want = wanted();
        if self.armed == Some(want) {
            return false;
        }
        self.armed = Some(want);
        dl.set_vector(want);
        true
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
            let mut dl = DrawList::new();
            let mut lane = Lane::new();
            assert!(lane.arm(&mut dl), "a fresh lane must arm unconditionally");
            panel(&mut dl);
            assert_eq!(
                drew_shapes(&dl),
                1,
                "render.vector = true must put the panel on the shape lane"
            );
        }

        let _theme = Themed::new("vector-off", "[render]\nvector = false\n");
        assert!(!wanted());
        let mut dl = DrawList::new();
        let mut lane = Lane::new();
        lane.arm(&mut dl);
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

    /// The condition f3 §6 K3 calls the easy one to miss: the mode is set
    /// at EVERY theme load, not once where the list is built. A host that
    /// armed its list in the constructor would draw the theme it started
    /// with for the rest of the session, and no amount of loading would
    /// move it.
    ///
    /// The list here is the one from the first theme, kept across the
    /// swap exactly as a screen keeps its own between frames.
    #[test]
    fn the_list_survives_a_theme_swap() {
        let _lock = theme_test_lock();
        let mut dl = DrawList::new();
        let mut lane = Lane::new();

        {
            let _theme = Themed::new("swap-off", "[render]\nvector = false\n");
            lane.arm(&mut dl);
            panel(&mut dl);
            assert_eq!(drew_shapes(&dl), 0, "the first theme says no");
        }

        // A second theme is loaded under the SAME list — the swap the
        // settings window makes, and the one a mood makes.
        {
            let _theme = Themed::new("swap-on", "[render]\nvector = true\n");
            dl.clear();
            assert!(lane.arm(&mut dl), "the swap must re-arm the list");
            panel(&mut dl);
            assert_eq!(
                drew_shapes(&dl),
                1,
                "the list must follow the theme that was loaded after it was built"
            );
        }

        // And back, because a switch that only latches one way is not a
        // switch. Dropping the fixture above already restored the master
        // — whose `render.vector` is false — so this is the master's own
        // answer arriving at a list that had been told otherwise.
        dl.clear();
        assert!(lane.arm(&mut dl), "unloading the fixture must re-arm too");
        panel(&mut dl);
        assert_eq!(drew_shapes(&dl), 0, "the master ships the tessellated lane");
    }

    /// A mode is not frame state: clearing the list for the next frame
    /// must not disarm it, and a steady frame must not have to re-arm.
    ///
    /// The second half is the one that would go unnoticed: an `arm` that
    /// answered `true` every frame would still draw correctly and would
    /// silently drop the weld the toolkit uses to fuse a bed, its wash
    /// and its border into one record — `set_vector` resets it.
    #[test]
    fn a_frame_boundary_is_not_a_theme_load() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("steady", "[render]\nvector = true\n");
        let mut dl = DrawList::new();
        let mut lane = Lane::new();
        assert!(lane.arm(&mut dl));
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

    /// The shipping answer, stated as a test rather than as a promise:
    /// the master keeps the switch DOWN until K3d, which is a decision
    /// taken on the measurement in `.gap-program/pomiar-wektor-k3c.md`
    /// and not by whoever wires the reader.
    #[test]
    fn the_master_ships_the_switch_down() {
        let _lock = theme_test_lock();
        let _theme = Themed::new("plain", "");
        assert!(
            !wanted(),
            "render.vector went up without K3d; see .gap-program/pomiar-wektor-k3c.md"
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
