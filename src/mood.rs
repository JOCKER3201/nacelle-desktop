//! Who chooses the mood, and when (§5.24's three triggers, host side) —
//! and the one thing a mood change says for itself, [`Wash`].
//!
//! The engine has had moods since it was written: `[mood.alert]` resolves to
//! its own sibling theme, `theme::set_mood` swaps an index, and the whole
//! interface re-skins for the price of one store. Nothing in this program
//! ever called it. The alarm skin was built, tested, cached and unreachable —
//! and `[mood.alert]` carried the sentence that says when it should arrive,
//! `when = "severity >= critical"`, which nothing read. This module is the
//! reader.
//!
//! # Three triggers, one arbiter
//!
//! §5.24 orders them: **explicit API > external signal > the declarative
//! rule**. Two of the three live here as one field each.
//!
//! * The **host's** choice ([`Moods::set_host`]) latches. It is what image
//!   5's `SYSTEM LOCKDOWN` calls, and it is not something the telemetry may
//!   argue with: a lockdown that a falling temperature could lift would be a
//!   lockdown in name only. The rule keeps evaluating underneath while the
//!   latch is on, so the second the host lets go the picture is right — not
//!   a second later.
//! * The **theme's** rule is evaluated here, once a second, against the
//!   telemetry the desktop already collects ([`crate::system`], which
//!   rewrites its snapshot at exactly 1 Hz). Never per frame: a mood is a
//!   pre-resolved sibling precisely so that having moods costs nothing while
//!   drawing, and evaluating predicates at 144 Hz would put the cost back in
//!   the one place it was designed out of.
//!
//! # The five seconds
//!
//! A rising edge is applied at once — an alarm that waits is not an alarm.
//! A falling edge waits [`HOLD`], and any second in which the predicate holds
//! again cancels the wait. This is the whole reason the hysteresis is in the
//! specification: a value shivering around its threshold would otherwise
//! re-skin the entire interface twice a second, which is worse than never
//! showing the mood at all — it is unreadable, and it teaches the user to
//! ignore the one thing that is supposed to be unignorable.
//!
//! # And the change has to be visible
//!
//! Everything else in the picture re-reads its tokens while drawing, so an
//! index swap re-skins it for free. That is also why a mood change on its
//! own looks like a DRAWING FAULT: one frame the console is azure, the next
//! it is amber, with nothing in between to say that anything happened.
//! §5.24 answers that with one quad — [`Wash`] — and it is the only part of
//! a mood the host has to draw itself.

use nacelle::telemetry::Snapshot;
use nacelle::theme::{self, Color, MoodInput, MoodRule};
use nacelle::view::scroll::Easing;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How often a rule is evaluated (§5.24). The cadence of the telemetry, not
/// of the frame: the collector rewrites its snapshot once a second, so a
/// second evaluation inside one second could only reach the same answer.
const EVERY: Duration = Duration::from_secs(1);

/// How long a mood outlives the predicate that raised it (§5.24's
/// falling-edge hysteresis).
const HOLD: Duration = Duration::from_secs(5);

/// What the panels are reporting, replaced whole every frame by the pass
/// that already asks every widget for its chrome (u2 §4.1).
///
/// A severity is the widget's judgement of its own data, and §5.24's two
/// severity forms ask about it. The host does not judge the telemetry itself
/// and does not run a second collection for the alarm's sake: it reads the
/// answer it was already being given and throwing away.
static REPORTED: Mutex<Vec<u16>> = Mutex::new(Vec::new());

/// Publishes the severities of the frame just measured. **Replaces, never
/// accumulates** — a count of criticals must be a count of panels, not a
/// count of panels times frames.
pub fn note_severities(sevs: &[u16]) {
    if let Ok(mut g) = REPORTED.lock() {
        g.clear();
        g.extend_from_slice(sevs);
    }
}

fn reported() -> Vec<u16> {
    REPORTED.lock().map(|g| g.clone()).unwrap_or_default()
}

/// Where a decision goes. The engine in the running program; the tests hand
/// in a recorder instead, so a test of the ARBITRATION never re-skins the
/// process-wide theme under a test running beside it.
type Apply = fn(Option<&str>) -> bool;

/// §5.24's host-side arbiter: the theme's rules, the host's latch, and the
/// clock both of them are read against.
pub struct Moods {
    /// The theme's declared moods and their parsed `when`, in declaration
    /// order — so a later index is a stronger mood (§5.24: lockdown > alert
    /// > normal is the master's own order, read backwards).
    rules: Vec<MoodRule>,
    apply: Apply,
    /// When the next evaluation is due. `None` = now.
    next_eval: Option<Instant>,
    /// Which rule the predicate layer currently stands on.
    standing: Option<usize>,
    /// When the standing rule stopped holding. Cleared the moment it holds
    /// again, which is what makes a shivering value a non-event.
    falling_since: Option<Instant>,
    /// The host's latch (§5.24 trigger 1). Beats everything below it.
    host: Option<String>,
    /// What the engine was last told, so a second that changed nothing costs
    /// one comparison and no theme swap.
    applied: Option<String>,
}

impl Moods {
    /// Reads the loaded theme's rules. Construct **after** the theme is
    /// loaded; [`Moods::on_theme_reload`] keeps it in step afterwards.
    pub fn new() -> Moods {
        Moods::with_rules(theme::mood_rules(), theme::set_mood)
    }

    fn with_rules(rules: Vec<MoodRule>, apply: Apply) -> Moods {
        Moods {
            rules,
            apply,
            next_eval: None,
            standing: None,
            falling_since: None,
            host: None,
            applied: None,
        }
    }

    /// §5.24 trigger 1 — the explicit choice, latched until the host itself
    /// clears it with `None`.
    ///
    /// A name the theme does not declare is refused rather than remembered:
    /// latching a mood that can never resolve would mute the rule layer for
    /// the rest of the session over a typo.
    pub fn set_host(&mut self, name: Option<&str>) -> bool {
        match name {
            Some(n) if !self.rules.iter().any(|r| r.name == n) => {
                eprintln!(
                    "nacelle-desktop: no mood \"{n}\" in this theme (it declares {})",
                    self.names()
                );
                return false;
            }
            _ => self.host = name.map(str::to_string),
        }
        // Both directions are immediate. Letting go hands the screen back to
        // whatever the rule layer decided while the latch was on, with no
        // second of the wrong picture in between.
        self.settle();
        true
    }

    /// Steps the latch on to the next mood the theme declares, and off the
    /// end back to none.
    ///
    /// §5.24's first trigger is "the explicit API", and until this the API
    /// had no caller anywhere in the program: image 5's `SYSTEM LOCKDOWN`
    /// is a launcher entry that does not exist yet, and a mood nobody can
    /// ask for is a mood nobody can check. One chord reaches every declared
    /// mood in turn, which is also what makes the alarm skin something a
    /// user — or a screenshot — can actually see.
    ///
    /// Answers the mood now latched, for whoever wants to say so.
    pub fn cycle_host(&mut self) -> Option<String> {
        let next = match &self.host {
            None => self.rules.first(),
            // A latched name the theme stopped declaring cannot be stepped
            // from, so the cycle restarts rather than sticking.
            Some(cur) => match self.rules.iter().position(|r| &r.name == cur) {
                Some(i) => self.rules.get(i + 1),
                None => self.rules.first(),
            },
        }
        .map(|r| r.name.clone());
        self.set_host(next.as_deref());
        next
    }

    /// One frame's worth of attention. Evaluates at most once per [`EVERY`]
    /// and does nothing at all in between, so calling it every frame is the
    /// intended use.
    pub fn tick(&mut self, now: Instant, sys: &Mutex<Snapshot>) {
        if self.next_eval.is_some_and(|due| now < due) {
            return;
        }
        self.next_eval = Some(now + EVERY);
        let severities = reported();
        // Held for as long as it takes to copy two numbers: the collector
        // thread must never wait on the arbiter.
        let (battery, temp_c) = match sys.lock() {
            Ok(s) => (s.battery.map(|(pct, _)| pct as f32), s.temp_c),
            Err(_) => (None, None),
        };
        let input = MoodInput { severities: &severities, battery, temp_c };
        self.evaluate(now, &input);
        self.settle();
    }

    /// A theme load rebuilds every sibling and lands on the plain one, so the
    /// rule layer starts from rest and re-decides on the next tick — which is
    /// asked for immediately, because a settings click during an alarm must
    /// not cost the alarm a whole second.
    pub fn on_theme_reload(&mut self) {
        self.rules = theme::mood_rules();
        self.standing = None;
        self.falling_since = None;
        self.applied = None;
        self.next_eval = None;
        if self.host.is_some() {
            // The host's choice survives a theme change if the new theme
            // still has the mood; otherwise the latch goes with the theme
            // that could express it.
            let host = self.host.take();
            self.set_host(host.as_deref());
        }
    }

    /// Which rule the telemetry asks for, and how the standing one gives way.
    fn evaluate(&mut self, now: Instant, input: &MoodInput) {
        let hit = self.rules.iter().rposition(|r| r.when.holds(input));
        match (hit, self.standing) {
            // Unchanged, or louder: both are answered at once. An alarm
            // getting worse must not wait out the calm timer.
            (Some(h), Some(s)) if h >= s => {
                self.standing = hit;
                self.falling_since = None;
            }
            (Some(_), None) => {
                self.standing = hit;
                self.falling_since = None;
            }
            // Quieter, or silent. The mood stays until the calm has lasted.
            (_, Some(_)) => {
                let since = *self.falling_since.get_or_insert(now);
                if now.saturating_duration_since(since) >= HOLD {
                    self.standing = hit;
                    self.falling_since = None;
                }
            }
            (None, None) => self.falling_since = None,
        }
    }

    /// Tells the engine, if what it was last told is no longer the answer.
    fn settle(&mut self) {
        let want: Option<String> = self
            .host
            .clone()
            .or_else(|| self.standing.map(|i| self.rules[i].name.clone()));
        if want == self.applied {
            return;
        }
        if !(self.apply)(want.as_deref()) && want.is_some() {
            // The engine refuses a mood it did not resolve. The rules and the
            // siblings come from the same load, so this can only mean the
            // theme changed without the reload path being told — and the
            // decision is recorded anyway rather than retried, because
            // retrying once a second would print this line once a second for
            // as long as that theme lives.
            eprintln!("nacelle-desktop: the theme engine refused mood {want:?}");
        }
        self.applied = want;
    }

    fn names(&self) -> String {
        self.rules.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
    }
}

// ------------------------------------------------------------------ the wash

/// What `motion.mood_change` says about the fade: how long it lasts, after
/// the one global scale, and the curve it runs on.
///
/// Read only while a wash is actually in flight — a few token reads for the
/// fifteen frames a quarter of a second lasts, and nothing at all on the
/// other frames of the session.
struct Fade {
    ms: f32,
    ease: Easing,
}

impl Fade {
    fn read() -> Fade {
        // ONE resolver. The five-word easing table that stood here, and
        // the hand-multiplied `duration_ms * motion.scale` beside it,
        // are `motion::Effect`'s job now — and the table was not merely
        // duplicated, it was WRONG about one word: `custom` fell through
        // to linear, so a theme's `easing_p` bezier moved every other
        // animation in the program and not this one.
        let e = nacelle::motion::Effect::of("mood_change");
        // Reduced motion (`motion.scale = 0`) does not run the wash in zero
        // milliseconds, it SKIPS it — §5.24's own word. A zero length here
        // is how that reaches the caller, and the disabled flag lands in the
        // same place because "no fade" and "no time to fade in" are the same
        // picture. `one_shot_secs` answers 0 for both.
        Fade { ms: e.one_shot_secs() * 1000.0, ease: e.one_shot_easing() }
    }
}

/// §5.24's transition tint: one full-screen quad, the entered mood's own
/// `wash` colour, fading from its declared alpha to zero over
/// `motion.mood_change`.
///
/// This is the whole of what a mood change costs to DRAW. Everything else
/// re-skins by index swap, which is exactly why the quad is needed: a
/// re-skin with no transition is indistinguishable from a rendering fault,
/// and the one moment the interface most needs to be believed is the moment
/// it turns amber.
///
/// It watches the ENGINE rather than the arbiter, so a mood arriving by any
/// of §5.24's three triggers is announced the same way — including one a
/// plugin sets, which the arbiter would never hear about.
pub struct Wash {
    /// The engine epoch the last frame saw. One atomic load, and it moves on
    /// every sibling swap — so the two questions below, which lock the
    /// engine and allocate, are asked only on the frames where their answer
    /// can have changed. Sixty string clones a second for an answer that
    /// changes once an hour is exactly the per-frame cost a pre-resolved
    /// sibling exists to avoid.
    epoch: u32,
    /// Which mood that epoch stood in. The NAME is the event, not the
    /// epoch: a resize re-bakes and moves the epoch without changing a
    /// thing about the mood, and a window drag would otherwise flash the
    /// alarm tint on every frame of the drag.
    mood: Option<String>,
    /// The tint the entered mood declared, and the frame clock it started
    /// on. Read once per change: `theme::mood_wash` re-resolves a whole
    /// sibling spec, which is a load-time cost and not a frame one.
    run: Option<(Color, f64)>,
}

impl Wash {
    /// Starts from whatever the engine is showing NOW, so the desktop's
    /// first picture is not a transition into itself. Construct it BEFORE
    /// any host latch is applied: booting straight into lockdown IS a
    /// change from the resting console, and it should announce itself like
    /// any other.
    pub fn new() -> Wash {
        Wash { epoch: theme::epoch(), mood: theme::current_mood(), run: None }
    }

    /// The quad this frame should draw, on the frame clock `now` — the same
    /// clock the widgets animate off, so an armed pixel-guard run measures
    /// the fade instead of the machine's speed.
    pub fn at(&mut self, now: f64) -> Option<Color> {
        let epoch = theme::epoch();
        if epoch != self.epoch {
            self.epoch = epoch;
            self.entered(theme::current_mood(), theme::mood_wash, now);
        }
        let (c, t0) = self.run?;
        let f = Fade::read();
        // A wash with no time to run is not drawn for one frame at full
        // strength — it is not drawn at all, and the run is dropped so the
        // question is not asked again.
        let over = |w: &mut Wash| {
            w.run = None;
            None
        };
        if f.ms <= 0.0 {
            return over(self);
        }
        // The clock is the frame's, and a frame's clock can go backwards
        // (the guard's virtual one restarts at zero), so the progress is
        // clamped rather than trusted.
        let p = (((now - t0) * 1000.0) as f32 / f.ms).max(0.0);
        if p >= 1.0 {
            return over(self);
        }
        let a = c.a * (1.0 - f.ease.at(p));
        (a > 0.0).then(|| c.alpha(a))
    }

    /// The state machine, with the engine's two answers handed in — so a
    /// test drives a mood change without re-skinning the process-wide theme
    /// under a test running beside it.
    ///
    /// The tint is a closure because it is the expensive half: it is asked
    /// only once the NAME has actually changed.
    fn entered(
        &mut self,
        mood: Option<String>,
        tint: impl FnOnce() -> Option<Color>,
        now: f64,
    ) {
        if mood == self.mood {
            return;
        }
        self.mood = mood;
        // `wash = none` is a mood saying it does not want announcing —
        // `[mood.normal]`'s own value, because coming back to rest is not
        // news. Leaving a mood for no mood at all lands on the plain
        // sibling, which has no wash either, so calm arrives quietly.
        self.run = tint().filter(|c| c.a > 0.0).map(|c| (c, now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nacelle::theme::MoodWhen;

    /// The engine is not touched by these: they assert what the arbiter
    /// DECIDED, and the decision is the whole of what this module owns.
    fn accept(_: Option<&str>) -> bool {
        true
    }

    /// The master's shape — `normal` and `lockdown` host-only, `alert`
    /// carrying the rule — with a predicate over a field the desktop's own
    /// snapshot really has, so the test drives the same path the program
    /// does rather than a mock of it.
    fn driver() -> Moods {
        let rules = vec![
            MoodRule { name: "normal".into(), when: MoodWhen::Never },
            MoodRule { name: "alert".into(), when: MoodWhen::TempAbove(90.0) },
            MoodRule { name: "lockdown".into(), when: MoodWhen::Never },
        ];
        Moods::with_rules(rules, accept)
    }

    fn at(temp: f32) -> Mutex<Snapshot> {
        Mutex::new(Snapshot { temp_c: Some(temp), ..Snapshot::default() })
    }

    #[test]
    fn a_rule_that_holds_raises_its_mood_on_the_first_evaluation() {
        let mut m = driver();
        let t0 = Instant::now();
        m.tick(t0, &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    /// Nothing happens between evaluations: the telemetry is rewritten once
    /// a second, so a frame is not an opinion.
    #[test]
    fn the_rule_is_read_once_a_second_and_not_once_a_frame() {
        let mut m = driver();
        let t0 = Instant::now();
        m.tick(t0, &at(20.0));
        // 143 more frames inside the same second.
        for i in 1..144 {
            m.tick(t0 + Duration::from_millis(i * 6), &at(95.0));
        }
        assert_eq!(m.applied, None, "a frame decided a mood");
        m.tick(t0 + EVERY, &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    #[test]
    fn the_mood_outlives_its_predicate_by_five_seconds_and_not_a_second_less() {
        let mut m = driver();
        let t0 = Instant::now();
        m.tick(t0, &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
        // The machine cools at t0+1s. Every second up to the fifth still
        // shows the alarm.
        for s in 1..=5 {
            m.tick(t0 + Duration::from_secs(s), &at(20.0));
            assert_eq!(
                m.applied.as_deref(),
                Some("alert"),
                "the alarm left after {s}s of calm"
            );
        }
        m.tick(t0 + Duration::from_secs(6), &at(20.0));
        assert_eq!(m.applied, None);
    }

    /// The reason the hysteresis exists: a reading sitting on its threshold
    /// must produce ONE mood change, not one per second.
    #[test]
    fn a_value_shivering_on_its_threshold_does_not_strobe_the_interface() {
        let mut m = driver();
        let t0 = Instant::now();
        let mut changes = 0;
        let mut last: Option<String> = None;
        for s in 0..30 {
            // 91, 89, 91, 89 … — across the threshold every single second.
            let temp = if s % 2 == 0 { 91.0 } else { 89.0 };
            m.tick(t0 + Duration::from_secs(s), &at(temp));
            if m.applied != last {
                changes += 1;
                last = m.applied.clone();
            }
        }
        assert_eq!(changes, 1, "the interface re-skinned {changes} times");
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    #[test]
    fn the_host_beats_the_rule_and_the_rule_cannot_take_it_back() {
        let mut m = driver();
        let t0 = Instant::now();
        assert!(m.set_host(Some("lockdown")));
        assert_eq!(m.applied.as_deref(), Some("lockdown"));
        // Hot, cold, hot again: the rule decides underneath and the picture
        // never moves.
        for (s, temp) in [(1, 95.0), (2, 20.0), (8, 20.0), (9, 95.0)] {
            m.tick(t0 + Duration::from_secs(s), &at(temp));
            assert_eq!(m.applied.as_deref(), Some("lockdown"), "the rule broke the latch at {s}s");
        }
        // …and the moment the host lets go, the rule's own answer is on
        // screen. It has been kept up to date all along.
        m.set_host(None);
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    #[test]
    fn a_mood_the_theme_does_not_declare_is_refused_not_latched() {
        let mut m = driver();
        assert!(!m.set_host(Some("dinner")));
        assert_eq!(m.host, None);
        m.tick(Instant::now(), &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    /// Two rules holding at once: §5.24 settles them by declaration order,
    /// so the stronger mood is the one written later in the theme.
    #[test]
    fn the_stronger_mood_wins_at_once_and_the_weaker_waits_out_the_hold() {
        let rules = vec![
            MoodRule { name: "warm".into(), when: MoodWhen::TempAbove(70.0) },
            MoodRule { name: "alert".into(), when: MoodWhen::TempAbove(90.0) },
        ];
        let mut m = Moods::with_rules(rules, accept);
        let t0 = Instant::now();
        m.tick(t0, &at(75.0));
        assert_eq!(m.applied.as_deref(), Some("warm"));
        // Rising: no wait.
        m.tick(t0 + Duration::from_secs(1), &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
        // Falling to the weaker rule is still a fall, and waits.
        m.tick(t0 + Duration::from_secs(2), &at(75.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
        m.tick(t0 + Duration::from_secs(7), &at(75.0));
        assert_eq!(m.applied.as_deref(), Some("warm"));
    }

    /// One chord walks every declared mood and comes back to none, so the
    /// explicit trigger reaches all of them and cannot strand the interface
    /// in one it has no way out of.
    #[test]
    fn the_host_chord_steps_through_every_mood_and_off_the_end() {
        let mut m = driver();
        assert_eq!(m.cycle_host().as_deref(), Some("normal"));
        assert_eq!(m.cycle_host().as_deref(), Some("alert"));
        assert_eq!(m.cycle_host().as_deref(), Some("lockdown"));
        assert_eq!(m.cycle_host(), None, "the last mood is a dead end");
        // …and off the end the rule layer has the screen back, which it has
        // been keeping up to date all along.
        m.tick(Instant::now(), &at(95.0));
        assert_eq!(m.applied.as_deref(), Some("alert"));
    }

    // ---------------------------------------------------------- the wash

    /// A colour to hand in as a mood's declared `wash`, at the master's own
    /// `[mood.alert]` alpha.
    fn tint() -> Color {
        Color { r: 1.0, g: 0.16, b: 0.21, a: 0.18 }
    }

    /// Reads `motion.mood_change` out of the process-wide engine, so the
    /// lock the rest of the binary's theme tests take is taken here too.
    fn washing() -> (std::sync::MutexGuard<'static, ()>, Wash) {
        let guard = crate::widgets::theme_test_lock();
        // Settles the engine before the epoch is recorded: a lazy first
        // load bumps it, and a `Wash` built across that would see a change
        // that is only the theme arriving.
        let _ = theme::resolved();
        (guard, Wash::new())
    }

    /// The master gives `motion.mood_change` 250 ms: full strength at the
    /// change, less on the way, and gone — not still there at a hundredth
    /// of an alpha for the rest of the session.
    #[test]
    fn the_quad_starts_at_the_declared_alpha_and_leaves_when_its_time_is_up() {
        let (_lock, mut w) = washing();
        w.entered(Some("alert".into()), || Some(tint()), 0.0);
        let first = w.at(0.0).expect("the mood change was not announced");
        assert!((first.a - tint().a).abs() < 1e-6, "started at {}", first.a);
        let mid = w.at(0.125).expect("the wash left halfway through");
        assert!(mid.a < first.a && mid.a > 0.0, "alpha {} did not fall", mid.a);
        assert_eq!(w.at(0.25), None, "the wash outlived motion.mood_change");
        assert_eq!(w.at(9.0), None, "and came back afterwards");
    }

    /// The wash announces a CHANGE OF MOOD, and the epoch is not that: a
    /// window resize re-bakes and moves it on every frame of the drag. If
    /// the epoch were the event, dragging a window edge during an alarm
    /// would strobe the alarm tint at the frame rate.
    #[test]
    fn a_rebake_that_lands_on_the_same_mood_announces_nothing() {
        let (_lock, mut w) = washing();
        w.entered(Some("alert".into()), || Some(tint()), 0.0);
        let mut asked = 0;
        w.entered(
            Some("alert".into()),
            || {
                asked += 1;
                Some(tint())
            },
            0.1,
        );
        assert_eq!(asked, 0, "a re-bake re-resolved the sibling spec for nothing");
        // The fade also keeps its own start, so it finishes when it was
        // always going to finish.
        assert_eq!(w.run.map(|(_, t0)| t0), Some(0.0));
    }

    /// `wash = none` is a mood saying it does not want announcing —
    /// `[mood.normal]`'s own value, and what the plain sibling has when an
    /// alarm ends. Coming back to rest is not news.
    #[test]
    fn a_mood_that_declares_no_wash_arrives_quietly() {
        let (_lock, mut w) = washing();
        w.entered(Some("alert".into()), || Some(tint()), 0.0);
        assert!(w.at(0.0).is_some());
        w.entered(None, || None, 1.0);
        assert_eq!(w.at(1.0), None, "the calm flashed");
    }

    /// The wash's own five-word easing table knew `linear`, `ease_out`,
    /// `ease_in`, `ease_in_out`, `sine` and `step` — and not `custom`,
    /// the one word that carries `easing_p`'s bezier with it. A theme
    /// that wrote a curve for the mood change got a straight line, in
    /// the one module whose whole job is to be believed.
    ///
    /// The curve below stands at ~0.98 halfway through the fade, so the
    /// quad is all but gone; linear would leave it at half strength.
    #[test]
    fn a_custom_curve_fades_the_mood_wash() {
        let _lock = crate::widgets::theme_test_lock();
        // The fixture goes on BEFORE the `Wash` records the epoch: a
        // theme arriving mid-life is a re-bake, and `Wash::at` would
        // read it as one.
        let _t = crate::widgets::Themed::new(
            "wash-custom",
            "[motion.mood_change]\neasing = custom\neasing_p = [0.00, 0.90, 0.10, 1.00]\n",
        );
        let _ = theme::resolved();
        let mut w = Wash::new();
        w.entered(Some("alert".into()), || Some(tint()), 0.0);
        let mid = w.at(0.125).expect("the wash left halfway through");
        assert!(
            mid.a < 0.2 * tint().a,
            "the custom curve ran as linear — halfway the quad still \
             stands at {} of a declared {}",
            mid.a,
            tint().a
        );
    }

    /// The severities a widget reports are read from the frame just drawn,
    /// never summed over frames: three critical panels are three, at any
    /// frame rate.
    #[test]
    fn the_reported_severities_are_a_count_of_panels_not_of_frames() {
        note_severities(&[3, 3, 0]);
        note_severities(&[3, 3, 0]);
        assert_eq!(reported(), vec![3, 3, 0]);
        note_severities(&[]);
        assert!(reported().is_empty());
    }
}
