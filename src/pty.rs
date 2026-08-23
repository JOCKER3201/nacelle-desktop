//! PTY: running the user's shell on a pseudoterminal (libc).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};

pub enum PtyEvent {
    Data(Vec<u8>),
    /// The shell has exited.
    Exited,
}

pub struct Pty {
    pub master: RawFd,
    pub child: libc::pid_t,
    /// The thread draining `master`, and the handle that stops it.
    /// Owned by the `Pty` precisely so that it can be stopped BEFORE
    /// the descriptor it reads is closed — see [`Drain`].
    drain: Drain,
}

/// Why the reader thread stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    /// The owner asked, through the wake pipe. `master` is still open
    /// and still ours when this answer comes back — which is the whole
    /// reason the pipe exists.
    Asked,
    /// The shell went away: `read` returned 0, or the master reported
    /// the hangup that closing the last slave leaves behind (EIO).
    Hangup,
    /// The receiving end of the channel is gone, so nobody is listening.
    Orphaned,
}

/// The thread that drains a PTY master, plus the pipe that stops it.
///
/// THE RISK THIS REMOVES. The reader used to be stopped by closing the
/// master out from under it while it sat blocked in `read`. That works —
/// the blocked read wakes, the next one gets EBADF, the thread exits —
/// and strace caught it doing exactly that at every shutdown, 149 µs
/// apart. It is nevertheless the wrong shape, because a descriptor
/// NUMBER is not a descriptor: the moment it is closed the kernel is
/// free to hand the same number to the next thing this process opens,
/// and there is a window in which the reader is still inside `read` on
/// that number. In this very process fd 71 had already been the AI
/// daemon's socket before it was a PTY master. A reader that woke a
/// moment later would have eaten bytes belonging to somebody else's
/// connection and posted them into a terminal — silently, and only
/// sometimes.
///
/// WHY THE PIPE RATHER THAN A FLAG. A flag cannot be seen by a thread
/// blocked in `read`; something has to make the blocking call return,
/// and the only choices are closing the descriptor (the bug), a signal
/// (process-wide, and EINTR would have to be distinguished from every
/// other EINTR), or a second descriptor the reader waits on at the same
/// time. The pipe is that second descriptor: `poll` watches both, the
/// wake byte ends the wait, the thread returns, and only then — after
/// the join — does anyone close the master. Nothing is ever inside
/// `read` on a number that is about to be recycled.
struct Drain {
    /// Write end of the stop pipe. The reader holds the read end and
    /// closes it on its way out.
    wake: RawFd,
    thread: Option<std::thread::JoinHandle<Stop>>,
}

impl Drain {
    /// Starts draining `master` into `tx`.
    fn start(master: RawFd, tx: Sender<PtyEvent>) -> io::Result<Drain> {
        // CLOEXEC on both ends: a stop pipe inherited by every shell the
        // user starts is one more descriptor that keeps this one alive.
        let mut ends: [libc::c_int; 2] = [-1, -1];
        if unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let (read_end, wake) = (ends[0], ends[1]);
        // Named, because a thread with no name is a thread an audit cannot
        // place — and this one outlives the call that made it.
        let thread = crate::threads::spawn(crate::threads::PTY, move || {
            read_loop(master, read_end, tx)
        })?;
        Ok(Drain {
            wake,
            thread: Some(thread),
        })
    }

    /// Stops the reader and waits for it to be gone, answering why it
    /// stopped. AFTER this returns, and not before, the master may be
    /// closed. A second call answers None: there is nothing left to stop.
    fn stop(&mut self) -> Option<Stop> {
        let thread = self.thread.take()?;
        if self.wake >= 0 {
            // One byte is all the reader looks for. A full pipe would
            // mean it never drained one, which cannot happen — it reads
            // this pipe only to exit.
            let byte = 0u8;
            unsafe {
                libc::write(self.wake, &byte as *const u8 as *const libc::c_void, 1);
                libc::close(self.wake);
            }
            self.wake = -1;
        }
        thread.join().ok()
    }
}

impl Drop for Drain {
    fn drop(&mut self) {
        // This runs on EVERY session teardown, right after
        // `Pty::end_session` already stopped the reader — a struct's
        // fields are dropped once its own `Drop` body has returned. So
        // the second stop is the normal path, not a hypothetical one,
        // and the `take()` in `stop` is what keeps it from writing to a
        // pipe end that is closed and whose number may already belong to
        // something else. The other way in is a `Pty` that was never
        // finished being built.
        self.stop();
    }
}

/// Waits on the master and the stop pipe together, forwarding whatever
/// the shell says until one of them ends it.
fn read_loop(master: RawFd, wake: RawFd, tx: Sender<PtyEvent>) -> Stop {
    let mut buf = [0u8; 8192];
    let mut fds = [
        libc::pollfd { fd: master, events: libc::POLLIN, revents: 0 },
        libc::pollfd { fd: wake, events: libc::POLLIN, revents: 0 },
    ];
    let stop = loop {
        fds[0].revents = 0;
        fds[1].revents = 0;
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break Stop::Hangup;
        }
        // The ask wins over pending data. Whatever the shell managed to
        // say in its last instant is not worth holding the exit for, and
        // the terminal it would have gone into is being torn down.
        if fds[1].revents != 0 {
            break Stop::Asked;
        }
        if fds[0].revents == 0 {
            continue;
        }
        // POLLHUP is reported alongside any bytes still buffered, so the
        // read comes first either way: it is the read, not the poll,
        // that distinguishes "more to say" from "gone".
        let n = unsafe { libc::read(master, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if n <= 0 {
            break Stop::Hangup;
        }
        if tx.send(PtyEvent::Data(buf[..n as usize].to_vec())).is_err() {
            break Stop::Orphaned;
        }
    };
    // A HANGUP IS ANNOUNCED FROM ONE PLACE, whichever way it was
    // learned. The reader that came before this one had no `poll` at
    // all, so every failure it could have arrived at was a failing
    // `read` and every one of them sent `Exited`; adding a second
    // syscall added a second way to leave the loop, and a `poll` that
    // fails for its own reasons (ENOMEM, and it is allowed others)
    // would have ended the thread in silence — the channel simply stops
    // delivering and the terminal keeps a dead session looking alive.
    // Announcing on the way out instead of at each site is what makes
    // that impossible to reintroduce by adding a third exit.
    if stop == Stop::Hangup {
        let _ = tx.send(PtyEvent::Exited);
    }
    unsafe {
        libc::close(wake);
    }
    stop
}

/// Whether `name` is an executable somewhere on PATH.
fn in_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| {
        let p = dir.join(name);
        std::fs::metadata(&p)
            .map(|m| m.is_file() && std::os::unix::fs::MetadataExt::mode(&m) & 0o111 != 0)
            .unwrap_or(false)
    })
}

impl Pty {
    pub fn spawn(
        cols: u16,
        rows: u16,
        cwd: Option<&Path>,
    ) -> io::Result<(Pty, Receiver<PtyEvent>)> {
        // TERM like in eDEX-UI (xterm.js) — full colors.
        std::env::set_var("TERM", "xterm-256color");
        std::env::set_var("COLORTERM", "truecolor");

        // Under gamescope everything the shell starts must STAY in the
        // gamescope session, where the frames are. Two doors lead out,
        // and both get closed here: a Wayland handle would let a
        // toolkit bypass the managed X display, and the desktop's
        // D-Bus session would let activation (KDE's KIO among others)
        // spawn a program's helpers back on the real desktop — which
        // is exactly where dolphin's children escaped to.
        let contain = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .map(|d| d.to_lowercase().contains("gamescope"))
                .unwrap_or(false);
        if contain {
            std::env::remove_var("WAYLAND_DISPLAY");
        }

        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let mut master: RawFd = -1;
        let mut slave: RawFd = -1;
        let ret = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &ws,
            )
        };
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }

        // The master must not leak into forked children (and their execs):
        // without CLOEXEC every new shell inherits the PTY fds of the
        // already-open sessions.
        unsafe {
            let flags = libc::fcntl(master, libc::F_GETFD);
            if flags >= 0 {
                libc::fcntl(master, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let cwd_c = cwd.and_then(|p| CString::new(p.as_os_str().as_bytes()).ok());

        // For bash we inject the nacelle-desktop startup file (source ~/.bashrc +
        // opening files with the associated app when a name is typed).
        let shellrc = crate::config::shellrc_path();
        let use_rc = shell.rsplit('/').next() == Some("bash") && shellrc.is_file();
        let rc_c = CString::new(shellrc.as_os_str().as_bytes()).unwrap_or_default();
        let init_flag = CString::new("--init-file").unwrap();

        // A contained shell runs under its own D-Bus session; the bus
        // daemon inherits the gamescope display, so whatever it
        // activates lands there too.
        let wrap = contain && in_path("dbus-run-session");

        // Everything the child needs is built HERE, before fork(): the
        // child of a multithreaded process may only call async-signal-safe
        // functions between fork and exec — no allocation (CString, Vec).
        let shell_c = CString::new(shell.as_str()).unwrap_or_default();
        let wrap_c = CString::new("dbus-run-session").unwrap();
        let prog = if wrap { wrap_c.clone() } else { shell_c.clone() };
        let mut argv: Vec<*const libc::c_char> = Vec::new();
        if wrap {
            argv.push(wrap_c.as_ptr());
        }
        argv.push(shell_c.as_ptr());
        if use_rc {
            argv.push(init_flag.as_ptr());
            argv.push(rc_c.as_ptr());
        }
        argv.push(std::ptr::null());

        let pid = unsafe { libc::fork() };
        if pid < 0 {
            let err = io::Error::last_os_error();
            // fork failed: neither fd is owned by a Pty yet — close both.
            unsafe {
                libc::close(master);
                libc::close(slave);
            }
            return Err(err);
        }
        if pid == 0 {
            // Child process: attach the slave as the controlling terminal
            // and exec the shell. Only async-signal-safe calls below — the
            // CStrings/argv were allocated before the fork.
            unsafe {
                libc::close(master);
                libc::setsid();
                libc::ioctl(slave, libc::TIOCSCTTY, 0);
                libc::dup2(slave, 0);
                libc::dup2(slave, 1);
                libc::dup2(slave, 2);
                if slave > 2 {
                    libc::close(slave);
                }
                // Shell start directory (home directory by default).
                if let Some(ref d) = cwd_c {
                    libc::chdir(d.as_ptr());
                }
                libc::execvp(prog.as_ptr(), argv.as_ptr());
                // If execvp failed:
                libc::_exit(127);
            }
        }
        unsafe {
            libc::close(slave);
        }

        let (tx, rx): (Sender<PtyEvent>, Receiver<PtyEvent>) = channel();
        let drain = match Drain::start(master, tx) {
            Ok(d) => d,
            Err(e) => {
                // No reader means no session. Nothing owns the master or
                // the child yet, so both are cleaned up here rather than
                // handed to a `Pty` that would never be read from.
                unsafe {
                    libc::kill(pid, libc::SIGHUP);
                    libc::close(master);
                    let mut status = 0;
                    libc::waitpid(pid, &mut status, 0);
                }
                return Err(e);
            }
        };

        Ok((
            Pty {
                master,
                child: pid,
                drain,
            },
            rx,
        ))
    }

    pub fn write(&self, data: &[u8]) {
        let mut off = 0;
        while off < data.len() {
            let n = unsafe {
                libc::write(
                    self.master,
                    data[off..].as_ptr() as *const libc::c_void,
                    data.len() - off,
                )
            };
            if n <= 0 {
                break;
            }
            off += n as usize;
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        unsafe {
            libc::ioctl(self.master, libc::TIOCSWINSZ, &ws);
        }
    }

    /// Current working directory of the shell process (from /proc).
    pub fn child_cwd(&self) -> Option<std::path::PathBuf> {
        std::fs::read_link(format!("/proc/{}/cwd", self.child)).ok()
    }

    /// Ends the session, and THE ORDER IS THE FIX: the shell is told to
    /// go, the reader is stopped AND JOINED, and only after that does
    /// the master go back to the kernel. When the close happens there is
    /// no thread anywhere in this process sitting inside `read` or
    /// `poll` on that number — see [`Drain`] for what used to go wrong
    /// and why it only went wrong sometimes.
    ///
    /// The close is a PARAMETER because an order is only worth having if
    /// it can be checked, and an order that lives inside a `Drop` body
    /// cannot be: by the time a test regains control both steps have
    /// happened and the struct is gone, so a reversed teardown looks
    /// exactly like a correct one. Production hands in the real `close`
    /// (below); the test hands in one that first asks whether the reader
    /// has really finished and whether the descriptor is still ours.
    fn end_session(&mut self, close_master: impl FnOnce(RawFd)) -> Option<Stop> {
        unsafe {
            libc::kill(self.child, libc::SIGHUP);
        }
        let why = self.drain.stop();
        close_master(self.master);
        unsafe {
            // Reap the child so it does not linger as a zombie. Closing
            // the master gives the shell EOF and SIGHUP terminates it, so
            // this usually returns promptly — but nothing guarantees it: a
            // shell (or a foreground job it left running) that traps or
            // ignores SIGHUP would leave this blocked forever, and this
            // runs on the UI thread during shutdown. So the wait is
            // bounded: poll for the exit without blocking, and once the
            // deadline passes, SIGKILL — which cannot be trapped or
            // ignored — before the final, now-brief, blocking wait.
            let mut status = 0;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                if libc::waitpid(self.child, &mut status, libc::WNOHANG) != 0 {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    libc::kill(self.child, libc::SIGKILL);
                    libc::waitpid(self.child, &mut status, 0);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        why
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.end_session(|fd| unsafe {
            libc::close(fd);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A bare master/slave pair — no fork, no shell. The reader under
    /// test does not care what is on the other end, and a test that
    /// spawned a shell would be testing the shell.
    fn pair() -> (RawFd, RawFd) {
        let mut master: RawFd = -1;
        let mut slave: RawFd = -1;
        let ok = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(ok, 0, "openpty failed: {}", io::Error::last_os_error());
        (master, slave)
    }

    /// Whether this process still holds `fd` — the question the whole
    /// fix is about. A closed number answers EBADF here, and a number
    /// that has been closed is a number the kernel may hand to anything.
    fn still_open(fd: RawFd) -> bool {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        flags >= 0
    }

    /// WHICH pty this number is the master of. A closed number answers
    /// nothing, and a number the kernel has already handed to something
    /// else answers about that something else — which is the entire
    /// confusion this fix exists to prevent, so anything checking after
    /// a close has to ask by identity and not by "is it open". The other
    /// tests in this file open descriptors too, on other threads.
    fn pty_number(fd: RawFd) -> Option<libc::c_uint> {
        let mut n: libc::c_uint = 0;
        if unsafe { libc::ioctl(fd, libc::TIOCGPTN, &mut n) } != 0 {
            return None;
        }
        Some(n)
    }

    /// A child that exists and is already over: a zombie. `end_session`
    /// signals it and reaps it, and a zombie answers both without any
    /// running process that could be surprised by a SIGHUP. Its pid also
    /// cannot be recycled underneath the test, precisely because it has
    /// not been reaped yet — the test would otherwise be signalling a
    /// number, which is the same class of mistake it exists to catch.
    fn spent_child() -> libc::pid_t {
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // Between fork and exit, only async-signal-safe calls. This
            // child makes none at all.
            unsafe { libc::_exit(0) };
        }
        assert!(pid > 0, "fork failed: {}", io::Error::last_os_error());
        pid
    }

    /// THE ORDER, WHERE THE ORDER ACTUALLY LIVES. A whole session is
    /// torn down, and the close is asked, at the instant it runs, two
    /// questions that only a correct order can answer yes to: is the
    /// master still ours, and is the reader thread already finished.
    ///
    /// The second question is the one with teeth. The reader owns the
    /// sending end of the event channel, so a receiver that reports
    /// `Disconnected` is proof that the thread has run to the end —
    /// there is no other way for that sender to be dropped. Reverse the
    /// two steps in `end_session` and the first question still answers
    /// yes (the test's own closure is the thing that closes), while the
    /// second answers no, because the reader is still sitting in `poll`
    /// on a number that is on its way back to the kernel. That reversal
    /// is exactly the pre-change teardown.
    #[test]
    fn the_session_stops_the_reader_before_it_closes_the_master() {
        let (master, slave) = pair();
        let (tx, rx) = channel();
        let drain = Drain::start(master, tx).expect("stop pipe");
        let mut pty = Pty {
            master,
            child: spent_child(),
            drain,
        };

        // The property is about interrupting a BLOCKED reader, so give
        // it time to reach its wait.
        std::thread::sleep(Duration::from_millis(50));

        let mut seen = None;
        let why = pty.end_session(|fd| {
            let reader_gone = loop {
                match rx.try_recv() {
                    // Whatever a pty pair says to itself is beside the
                    // point; only the end of the channel is.
                    Ok(_) => continue,
                    Err(e) => break e == std::sync::mpsc::TryRecvError::Disconnected,
                }
            };
            seen = Some((still_open(fd), reader_gone));
            unsafe {
                libc::close(fd);
            }
        });

        // Forgotten rather than dropped: the teardown has already run,
        // and running it again would close a number the kernel is free
        // to have handed to something else in the meantime — which is
        // the hazard under test, and no nicer inside a test than out.
        std::mem::forget(pty);
        unsafe {
            libc::close(slave);
        }

        assert_eq!(why, Some(Stop::Asked), "the reader must answer the ask");
        let (open_at_close, reader_gone) = seen.expect("the close never ran");
        assert!(
            open_at_close,
            "the master was already gone when the session closed it"
        );
        assert!(
            reader_gone,
            "the master was closed while the reader thread was still running — the very race"
        );
    }

    /// And `Drop` is that teardown, not another one beside it. Without
    /// this the test above would be checking a method the program never
    /// reaches: a `Drop` that closed the master itself would leave every
    /// assertion in it true and the race back in place.
    #[test]
    fn dropping_a_session_ends_it() {
        let (master, slave) = pair();
        let (tx, rx) = channel();
        let drain = Drain::start(master, tx).expect("stop pipe");
        let pty = Pty {
            master,
            child: spent_child(),
            drain,
        };
        let ours = pty_number(master).expect("a fresh master knows its pty");
        std::thread::sleep(Duration::from_millis(50));

        drop(pty);

        assert_ne!(
            pty_number(master),
            Some(ours),
            "the session was dropped and the master is still this pty's"
        );
        let reader_gone = loop {
            match rx.try_recv() {
                Ok(_) => continue,
                Err(e) => break e == std::sync::mpsc::TryRecvError::Disconnected,
            }
        };
        assert!(reader_gone, "the reader outlived the session it belonged to");
        unsafe {
            libc::close(slave);
        }
    }

    /// The ask alone stops the reader: nothing has to be closed for it
    /// to come back, and at the moment it has come back the master is
    /// still open.
    ///
    /// This is the half of the fix that lives in [`Drain`]. The
    /// predecessor had no such thing — closing the master WAS the stop
    /// signal — and could not have answered `Asked` at all.
    #[test]
    fn the_reader_is_stopped_before_the_descriptor_is_closed() {
        let (master, slave) = pair();
        let (tx, _rx) = channel();
        let mut drain = Drain::start(master, tx).expect("stop pipe");

        // Let the reader reach its wait; the property under test is
        // about interrupting a BLOCKED read, so it has to be blocked.
        std::thread::sleep(Duration::from_millis(50));

        let why = drain.stop();
        assert_eq!(why, Some(Stop::Asked), "the reader must answer the ask");
        assert!(
            still_open(master),
            "the master was closed while the reader was in it — the very race"
        );

        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }

    /// Stopping is not the same as being deaf: what the shell said
    /// before the ask still arrives.
    #[test]
    fn the_reader_forwards_what_the_slave_writes() {
        let (master, slave) = pair();
        let (tx, rx) = channel();
        let mut drain = Drain::start(master, tx).expect("stop pipe");

        let msg = b"nacelle";
        let n = unsafe {
            libc::write(slave, msg.as_ptr() as *const libc::c_void, msg.len())
        };
        assert_eq!(n, msg.len() as isize);

        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the reader delivered nothing");
        match got {
            PtyEvent::Data(d) => assert!(d.windows(msg.len()).any(|w| w == msg), "got {d:?}"),
            PtyEvent::Exited => panic!("a live slave must not read as a hangup"),
        }

        assert_eq!(drain.stop(), Some(Stop::Asked));
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }

    /// The other exit stays exactly as it was: a shell that goes away is
    /// still reported as `Exited`, and the reader still ends itself.
    /// This is what the poll must not have broken.
    #[test]
    fn a_departed_shell_still_reports_itself_gone() {
        let (master, slave) = pair();
        let (tx, rx) = channel();
        let mut drain = Drain::start(master, tx).expect("stop pipe");

        unsafe {
            libc::close(slave);
        }

        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a hangup must be reported, not swallowed");
        assert!(matches!(got, PtyEvent::Exited), "expected Exited");
        assert_eq!(drain.stop(), Some(Stop::Hangup));

        unsafe {
            libc::close(master);
        }
    }

    /// Asking twice is not an error, and the second ask does not write
    /// to a descriptor number that has been closed and possibly reused —
    /// the same hazard one level up.
    ///
    /// Every teardown asks twice: `Pty::end_session` stops the reader,
    /// and then the `Drain` field is dropped and stops it again. So this
    /// is the shape of the real path, not a defensive extra.
    #[test]
    fn stopping_twice_is_harmless() {
        let (master, slave) = pair();
        let (tx, _rx) = channel();
        let mut drain = Drain::start(master, tx).expect("stop pipe");
        assert_eq!(drain.stop(), Some(Stop::Asked));
        assert_eq!(drain.stop(), None);
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }
}
