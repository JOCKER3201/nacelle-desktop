//! Every thread this program starts ITSELF, and the name it wears.
//!
//! A thread with no name is a thread nobody can account for. `top`, `htop`,
//! `perf` and a strace log all identify a thread by its `comm`, and an
//! unnamed one inherits the process name — so a profile of nacelle-desktop
//! shows a column of identical rows and the reader is left guessing which
//! of them is the telemetry sweep and which is the shell reader.
//!
//! That guessing is not hypothetical. The strace audit of 2026-08-18 found
//! two threads waking on a fixed period and could not say what they were
//! for, because neither carried a name; it cost a whole investigation to
//! establish that they belong to the graphics driver and not to us.
//!
//! # What this module covers, and what it does not
//!
//! It covers the threads this tree starts by writing `spawn` — four of
//! them, in the table below. `no_thread_escapes_this_module` reads the
//! sources under `src/` and fails the build if a `std::thread::spawn` or a
//! `std::thread::Builder` appears in any of them but this one.
//!
//! It does NOT cover threads a dependency starts while serving a call of
//! ours, and a source scan never could: nothing in this tree spells them.
//! They were the majority. The audited run had thirty tasks, and exactly
//! two of the thirty ever called `PR_SET_NAME` — the clipboard's own
//! worker, and the audio writer asking for a name the kernel cut back to
//! the process name. Everything else in that log was anonymous, including:
//!
//! * **A rayon pool, sixteen threads — ours in every sense that matters,
//!   because our call started it.** `sysinfo`'s default `multithread`
//!   feature refreshes the process table in parallel, so `System::new_all`
//!   on the telemetry sweep cloned one thread per logical CPU — all
//!   sixteen `CLONE_THREAD` in that thread's log, 2 MiB stacks, Rust's own
//!   — and kept them for the life of the process. What they then did with
//!   an idle desktop: 109 053 `sched_yield` calls in 88 seconds, work
//!   stealing with nothing to steal, and not one `sched_yield` anywhere
//!   else in the process. That one IS gone — the feature is off in
//!   Cargo.toml, and `system.rs` has the test that counts live threads
//!   across a sample to keep it off. Note what it took to SEE it: a count
//!   at run time. Nobody reading this file may conclude from a nameless
//!   thread that it belongs to somebody else.
//! * **The clipboard worker.** `smithay-clipboard` runs a queue of its
//!   own and names it `smithay-clipboa` itself. What it costs, and why
//!   nothing here can take that cost off, is written out in `clipboard.rs`.
//! * **The graphics driver's, the remainder, two of which tick.** Not
//!   startable, nameable or stoppable from here: the two tickers' stacks
//!   are 8 MiB where every thread Rust starts gets 2 MiB, they wait on
//!   `CLOCK_REALTIME` condition variables in their own malloc arenas, and
//!   both `clone3` calls sit inside the Vulkan ICD's device creation. The
//!   driver names one `[vkps] Update` by writing `/proc/self/task/<tid>/
//!   comm` directly, which is why an audit grepping for `PR_SET_NAME`
//!   found nothing.
//!
//! So the claim this module can make is narrow: a nameless thread in a
//! profile of this program is not ours to name — but it may still be ours
//! to remove, and the pool above is what that looks like.
//!
//! Naming is not the same as being justified — a named thread can still be
//! idle work — but it is the precondition for judging one.

use std::io;
use std::thread::JoinHandle;

/// How long a thread name may be before the kernel cuts it.
///
/// `comm` is a sixteen-byte field of which one byte is the terminator, so
/// fifteen characters survive and everything past them is silently lost.
/// The loss is worse than it sounds: the audit's own audio thread asked
/// for `nacelle-desktop-audio` and was recorded as `nacelle-desktop`,
/// which is exactly the process name — a thread that HAD a name showed up
/// indistinguishable from one that had none. Hence the table below is
/// checked against this number rather than trusted to be short.
pub const COMM_MAX: usize = 15;

/// The ALSA writer: fills a period and hands it to the card, forever.
pub const AUDIO: &str = "nacelle-audio";

/// The telemetry sweep: CPU, memory, network and the process table.
pub const TELEMETRY: &str = "nacelle-telem";

/// The terminal's reader: blocks on the PTY master until the shell exits.
pub const PTY: &str = "nacelle-pty";

/// A backdrop/overlay bake, one screen's worth. Short-lived, one per
/// theme or size change — named anyway, because a bake that outstays its
/// welcome is exactly the kind of thing a profile has to be able to name.
pub const PLATE: &str = "nacelle-plate";

/// The roll call: every name above, gathered so the tests can check them
/// all at once. It carries no run-time duty — a thread is started by
/// naming its own constant — so it is built only for the test binary,
/// and a test below makes sure it never falls behind the table.
#[cfg(test)]
pub const ALL: [&str; 4] = [AUDIO, TELEMETRY, PTY, PLATE];

/// Starts a named thread. The only way this program makes one.
///
/// The name is `&'static str` on purpose: a thread name assembled at run
/// time (a screen index, a session number) reads well in one log line and
/// ruins aggregation across a session, which is the thing a profile is
/// for. If a caller ever genuinely needs to tell two instances apart, the
/// answer is a second constant, not a format string.
pub fn spawn<F>(name: &'static str, body: F) -> io::Result<JoinHandle<()>>
where
    F: FnOnce() + Send + 'static,
{
    debug_assert!(
        name.len() <= COMM_MAX,
        "thread name longer than the kernel keeps"
    );
    std::thread::Builder::new().name(name.to_string()).spawn(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name in the table survives the kernel's `comm` field whole.
    ///
    /// This is the test the audio thread failed: `nacelle-desktop-audio`
    /// is twenty-one bytes, so the kernel kept `nacelle-desktop` and the
    /// writer thread became indistinguishable from the process itself.
    #[test]
    fn names_fit_the_kernel_field() {
        for name in ALL {
            assert!(
                !name.is_empty() && name.len() <= COMM_MAX,
                "{name:?} is {} bytes; the kernel keeps {COMM_MAX}",
                name.len()
            );
        }
    }

    /// No two threads answer to the same name, or a profile that groups
    /// by name adds two unrelated workloads into one row.
    #[test]
    fn names_are_distinct() {
        let mut seen: Vec<&str> = ALL.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "two threads share a name: {seen:?}");
    }

    /// The name actually reaches the kernel, whole.
    ///
    /// Read from inside the thread through `/proc/thread-self`, which is
    /// that thread's own directory — no tid arithmetic, no libc call. If
    /// a name were ever too long this is where the truncation would show
    /// up as a mismatch rather than as a puzzling profile months later.
    #[test]
    fn a_spawned_thread_wears_its_name() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = spawn(TELEMETRY, move || {
            let comm = std::fs::read_to_string("/proc/thread-self/comm").unwrap_or_default();
            let _ = tx.send(comm.trim_end().to_string());
        })
        .expect("the test process can start one thread");
        let seen = rx.recv().expect("the named thread reported its comm");
        handle.join().expect("the named thread finished");
        assert_eq!(seen, TELEMETRY);
    }

    /// The roll call names every thread the table declares.
    ///
    /// Without this, adding a fifth constant and forgetting to list it
    /// would leave the name checks above silently passing over a thread
    /// nobody measured — the tests would go on being green about three
    /// names out of four.
    #[test]
    fn the_roll_call_is_complete() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/threads.rs"),
        )
        .expect("this module can read its own source");
        let mut declared = 0usize;
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("pub const ") else { continue };
            let Some((_, value)) = rest.split_once(": &str = ") else { continue };
            let name = value.trim_end_matches(';').trim_matches('"');
            declared += 1;
            assert!(ALL.contains(&name), "{name:?} is declared but not in ALL");
        }
        assert_eq!(declared, ALL.len(), "ALL and the table disagree in size");
    }

    /// Nothing in the tree starts a thread behind this module's back.
    ///
    /// A source scan rather than a run-time check, because the property
    /// is about code that may never execute in a test: the plate baker
    /// needs a window, the PTY reader needs a shell. The scan is what
    /// keeps the table above honest as the tree grows.
    #[test]
    fn no_thread_escapes_this_module() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stray: Vec<String> = Vec::new();
        let mut files = 0usize;
        walk(&src, &mut |path| {
            if path.file_name().is_some_and(|f| f == "threads.rs") {
                return;
            }
            let Ok(text) = std::fs::read_to_string(path) else { return };
            files += 1;
            for (n, line) in text.lines().enumerate() {
                // Doc prose about threading is not a thread; only the
                // two constructors that actually make one count.
                let code = line.split("//").next().unwrap_or("");
                if code.contains("thread::spawn(") || code.contains("thread::Builder") {
                    stray.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        });
        assert!(files > 0, "the source scan found no files to read");
        assert!(
            stray.is_empty(),
            "these start a thread outside crate::threads: {stray:?}"
        );
    }

    fn walk(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path)) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, f);
            } else if path.extension().is_some_and(|e| e == "rs") {
                f(&path);
            }
        }
    }
}
