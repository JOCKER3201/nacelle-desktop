//! The platform half of high contrast — the desktop portal, not the theme.
//!
//! `config::wanted_variant` already knows how to weigh an explicit
//! `variant:` choice against the platform's own answer; what it did not
//! have, until this file, was anyone TELLING it what the platform's answer
//! is. libnacelle's `motion.rs` draws the same line for reduced motion and
//! says so in its own doc comment: the preference lives in
//! `org.freedesktop.appearance`, a desktop portal is the HOST's business to
//! read, and until a host does, the toolkit's own answer is a fixed
//! `false`. This is that host, for high contrast (`org.freedesktop.
//! appearance`'s `contrast`) rather than reduced motion — the reduced-
//! motion caller is a second, separately tracked gap, and nothing here
//! calls `nacelle::motion::set_platform_reduce_motion`.
//!
//! ## Why a thread, and why nothing else
//!
//! nacelle-desktop's render loop (`main.rs`) is plain synchronous winit —
//! no tokio anywhere in the dependency tree — so a portal client that
//! blocks its caller cannot run there. `zbus::blocking` is the other half
//! of that trade: a real D-Bus connection with a synchronous call surface,
//! its own small `async-io` reactor confined entirely to the thread that
//! opened it. [`spawn`] starts exactly one such thread and nothing this
//! file does ever touches a winit type — the two halves meet only through
//! [`crate::config::set_platform_high_contrast`] and
//! [`crate::config::apply_engine_variant`], the same two functions a
//! config-file reload already goes through.
//!
//! ## Why no wake-up call
//!
//! A changed setting has to reach the screen, and the obvious tool is
//! `winit::event_loop::EventLoopProxy::send_event` — except main.rs
//! already has a mechanism for exactly this shape of problem and reuses it
//! rather than adding a second one. Every sibling swap (`nacelle::theme::
//! set_variant`, called below through `apply_engine_variant`) bumps the
//! engine's `CONTENT_EPOCH`, and the render loop's own `Event::AboutToWait`
//! arm compares that epoch every time it wakes — which, by construction, it
//! always does within one frame's cadence: `ControlFlow` there is
//! `WaitUntil(next_frame)` and NEVER a bare `Wait`, so ninety-six percent of
//! the time the next wake is at most `IDLE_FRAME` (250 ms) away, and the
//! draw itself re-reads `theme::resolved()` fresh regardless. A proxied
//! wake would only save the difference between "changes color within one
//! redraw" and "changes color within one redraw, sooner" — not worth a
//! second cross-thread channel next to the one this module already needs
//! for the atomic bool.
//!
//! ## The one D-Bus quirk this file works around
//!
//! `org.freedesktop.portal.Settings.Read` is documented to return a single
//! variant, but several portal backends have shipped it doubly wrapped —
//! `Variant[Variant[uint]]` instead of `Variant[uint]` — a long-standing,
//! still-live quirk (flatpak/xdg-desktop-portal#789). `zvariant::Value::
//! downcast` is written to see through exactly this: unlike `TryFrom<Value>`
//! for a concrete type, `downcast` also unwraps a `Value::Value(inner)`
//! before converting, so [`decode_contrast`] downcasts through `Value`
//! rather than converting `OwnedValue` directly and needs no bespoke
//! unwrap loop of its own.

use crate::config;

/// `org.freedesktop.appearance`'s own namespace and key for this
/// preference, verified against the current xdg-desktop-portal
/// specification (there is nothing in either repository to check it
/// against): `contrast` is `u`, `0` is no preference and `1` is higher
/// contrast, and an unknown value is specified to read as `0`.
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "contrast";

/// Starts the portal listener on a thread of its own and returns
/// immediately — nothing in `main()` waits on it, because nothing it
/// answers is needed to draw the first frame: `apply_engine_variant` has
/// already run once, through `config::load`, with the platform's answer
/// at its default of "no preference", which is the only honest answer
/// before anyone has asked the portal.
///
/// A session bus this desktop cannot reach (no portal running, no bus at
/// all — true of plenty of test and container environments) is not a
/// reason to fail the program: it is one line on stderr and the desktop
/// goes on taking its high-contrast answer from `variant:` alone, exactly
/// as it always could.
///
/// Started through `crate::threads::spawn` rather than `std::thread`
/// directly — the one door `threads.rs` documents as the whole of this
/// tree's thread accounting, and its own source scan fails the build on
/// anything that goes around it.
pub fn spawn() {
    if let Err(e) = crate::threads::spawn(crate::threads::A11Y_PORTAL, run) {
        eprintln!("nacelle-desktop: cannot start the accessibility portal thread: {e}");
    }
}

/// The thread body: connect, read the setting once, then sit on
/// `SettingChanged` for the rest of the session. Returns as soon as any
/// step fails, which ends the thread — there is nothing to retry onto,
/// since a portal that is not there at startup is not coming back without
/// a service restart this process would not see either way.
fn run() {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "nacelle-desktop: no session bus for the accessibility portal ({e}) \u{2014} \
                 high contrast follows variant: alone"
            );
            return;
        }
    };
    let proxy = match zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "nacelle-desktop: no Settings portal ({e}) \u{2014} high contrast follows \
                 variant: alone"
            );
            return;
        }
    };

    // The starting answer. A read that fails (the key genuinely does not
    // exist on this desktop, an older portal, …) leaves the platform flag
    // at its default `false` — "no preference known" is the correct
    // reading of a question nobody could answer, not a reason to guess.
    match proxy.call::<_, _, zbus::zvariant::OwnedValue>("Read", &(NAMESPACE, KEY)) {
        Ok(value) => apply(decode_contrast(value)),
        Err(e) => eprintln!(
            "nacelle-desktop: could not read {NAMESPACE}'s {KEY} from the portal ({e}) \u{2014} \
             high contrast follows variant: alone until the desktop reports a change"
        ),
    }

    // The live half: this preference can change mid-session (a system
    // settings panel, a profile switch), exactly as motion.rs's own doc
    // comment says about its sibling in the same portal namespace.
    let signals = match proxy.receive_signal("SettingChanged") {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "nacelle-desktop: cannot watch the portal for changes ({e}) \u{2014} high \
                 contrast will not follow the desktop after this point"
            );
            return;
        }
    };
    for msg in signals {
        let Ok((namespace, key, value)) =
            msg.body().deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
        else {
            continue;
        };
        if namespace == NAMESPACE && key == KEY {
            apply(decode_contrast(value));
        }
    }
}

/// The variant's payload, read past the double-wrap quirk the module
/// doc comment describes. Anything that is not the `u32` the spec
/// promises — a portal that answers a different type, a signal for a key
/// this desktop does not expect a number from — reads as "no preference"
/// rather than panicking a thread the rest of the program does not watch.
fn decode_contrast(value: zbus::zvariant::OwnedValue) -> bool {
    let value: zbus::zvariant::Value = value.into();
    matches!(value.downcast::<u32>(), Ok(1))
}

/// Records the platform's answer and, only if it actually changed,
/// re-derives the wanted variant through the one function that turns a
/// choice into a `nacelle::theme::set_variant` call — see
/// `config::apply_engine_variant`'s own doc comment for why THAT function
/// is the single place this and a config reload both have to go through.
fn apply(on: bool) {
    if config::set_platform_high_contrast(on) != on {
        config::apply_engine_variant();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ordinary case: a bare `u32`, no wrapping at all, which is what
    /// the specification promises and what a spec-following backend sends.
    #[test]
    fn a_plain_uint_decodes_directly() {
        assert!(!decode_contrast(zbus::zvariant::Value::from(0u32).try_into().unwrap()));
        assert!(decode_contrast(zbus::zvariant::Value::from(1u32).try_into().unwrap()));
    }

    /// The quirk this module exists to survive: `Read`'s reply doubly
    /// wrapped in a variant, `Variant[Variant[uint]]`, which is what
    /// several real portal backends have shipped for years
    /// (flatpak/xdg-desktop-portal#789).
    #[test]
    fn a_doubly_wrapped_uint_still_decodes() {
        let inner = zbus::zvariant::Value::from(1u32);
        let doubled = zbus::zvariant::Value::Value(Box::new(inner));
        let owned: zbus::zvariant::OwnedValue = doubled.try_into().unwrap();
        assert!(decode_contrast(owned));
    }

    /// A `u32` other than `0` or `1` — legal on the wire, since the
    /// specification only promises the TYPE and reserves the values for
    /// future preferences, and it says explicitly that an unknown one
    /// "should be treated as 0 (no preference)".
    #[test]
    fn an_unknown_value_reads_as_no_preference() {
        assert!(!decode_contrast(zbus::zvariant::Value::from(2u32).try_into().unwrap()));
    }

    /// Not the `u32` the specification promises at all — a portal
    /// disagreeing with its own spec, or a signal misrouted onto this
    /// key — reads as "no preference" rather than panicking a thread the
    /// rest of the program does not watch.
    #[test]
    fn a_value_of_the_wrong_type_reads_as_no_preference() {
        assert!(!decode_contrast(zbus::zvariant::Value::from("nonsense").try_into().unwrap()));
    }
}
