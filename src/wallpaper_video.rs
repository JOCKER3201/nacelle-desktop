//! Video wallpapers: `nacelle-wallpapers`' ffmpeg backend, decoded on
//! its own thread and handed to the screen one frame at a time.
//!
//! THE SHAPE OF THE PROBLEM. A still-image wallpaper (`theme::backdrop`,
//! `Screen::poll_plates`) is baked ONCE per theme/size change and holds
//! until the next one — the same shape as the decor plates it already
//! shares a worker with. A video wallpaper is a CONTINUOUS stream: a new
//! frame belongs on screen every 1/fps seconds for as long as the clip
//! is chosen, which is a different lifetime from "kicked on a change,
//! collected once it lands" entirely. This module is that second shape.
//!
//! THE MAILBOX, NOT A QUEUE. [`spawn_decoder`] hands back an
//! `Arc<Mutex<Option<VideoFrame>>>` rather than an `mpsc` receiver: a
//! wallpaper's own screen is drawn far more often than a typical video's
//! frame rate, so the right amount of backlog to carry between one poll
//! and the next is zero or one frame, never a queue of stale ones a slow
//! consumer falls behind on. The decoder thread OVERWRITES the slot;
//! [`Screen::poll_wallpaper_video`](crate::screen::Screen::poll_wallpaper_video)
//! takes whatever is there, or nothing.
//!
//! THE DECODER STOPS ITSELF. The thread holds only a `Weak` reference to
//! the mailbox; the screen holds the one `Arc`. Choosing a different
//! wallpaper drops that `Arc`, `Weak::upgrade` starts failing, and the
//! thread notices on its next frame and returns — which drops the
//! `FfmpegDecoder`, which kills the `ffmpeg` child (that type's own
//! `Drop`). Nothing here signals the thread directly; the mailbox going
//! away IS the signal.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use nacelle_wallpapers::video::ffmpeg::{probe_framerate, FfmpegDecoder};
use nacelle_wallpapers::video::FrameSource;

/// Whether `path`'s extension names a video container this pipeline
/// plays — the three formats the project was scoped to from the start
/// (project notes: "wideo w formatach webm, mp4, mkv"), matched
/// case-insensitively since a file picked by hand may carry either.
pub fn is_video_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mp4") | Some("webm") | Some("mkv")
    )
}

/// One decoded frame, tightly-packed RGBA8 — the same shape
/// `nacelle::theme::Plate` carries for a still-image bake, so the
/// screen's own texture create/update code does not need to know which
/// kind of wallpaper produced what it is holding.
pub struct VideoFrame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// Starts decoding `path` on its own named thread (`threads::
/// WALLPAPER_VIDEO`), looping the clip forever and pacing itself to the
/// file's own frame rate (re-probed each time the clip restarts, in
/// case `ffprobe` and `ffmpeg` ever disagree — cheap next to spawning
/// the decoder itself). Returns the mailbox the decoded frames land in;
/// dropping it is how a caller stops the decode (see the module docs).
///
/// A frame rate `ffprobe` cannot report, or reports as something absurd
/// (a corrupt or unusual file), falls back to 30 fps rather than
/// spinning the loop at whatever CPU-bound rate `next_frame` alone would
/// allow — a wrong-but-plausible pace is a wallpaper playing a little
/// off; an unpaced one is a wallpaper burning a core for no picture
/// anyone asked to see sixty times a second.
pub fn spawn_decoder(path: PathBuf) -> std::io::Result<Arc<Mutex<Option<VideoFrame>>>> {
    let mailbox = Arc::new(Mutex::new(None));
    let weak: Weak<Mutex<Option<VideoFrame>>> = Arc::downgrade(&mailbox);
    crate::threads::spawn(crate::threads::WALLPAPER_VIDEO, move || loop {
        if weak.upgrade().is_none() {
            return;
        }
        let fps = probe_framerate(&path).unwrap_or(30.0);
        let fps = if fps.is_finite() && fps > 0.0 { fps.clamp(1.0, 60.0) } else { 30.0 };
        let period = Duration::from_secs_f64(1.0 / fps);
        let mut decoder = match FfmpegDecoder::open(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "nacelle-desktop: wallpaper video {} did not open ({e}) — giving up",
                    path.display()
                );
                return;
            }
        };
        loop {
            let Some(mailbox) = weak.upgrade() else { return };
            let frame = match decoder.next_frame() {
                Ok(Some(f)) => f,
                // Clean end of stream: loop the clip from the start.
                Ok(None) => break,
                Err(e) => {
                    eprintln!(
                        "nacelle-desktop: wallpaper video {} stopped decoding ({e})",
                        path.display()
                    );
                    return;
                }
            };
            *mailbox.lock().unwrap() = Some(VideoFrame {
                w: frame.width(),
                h: frame.height(),
                rgba: frame.into_pixels(),
            });
            drop(mailbox);
            std::thread::sleep(period);
        }
    })?;
    Ok(mailbox)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_project_formats_are_recognised_case_insensitively() {
        for ext in ["mp4", "MP4", "webm", "WebM", "mkv", "MKV"] {
            assert!(is_video_path(Path::new(&format!("clip.{ext}"))), "{ext} was not recognised");
        }
    }

    #[test]
    fn a_still_image_extension_is_not_a_video() {
        for ext in ["png", "jpg", "jpeg", "bmp", "jxl"] {
            assert!(!is_video_path(Path::new(&format!("wall.{ext}"))), "{ext} was read as a video");
        }
    }

    #[test]
    fn a_path_with_no_extension_is_not_a_video() {
        assert!(!is_video_path(Path::new("no-extension-at-all")));
    }
}
