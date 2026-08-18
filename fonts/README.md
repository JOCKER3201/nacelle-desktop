# fonts/

Drop .ttf/.otf files here, and `make install` them, to override the fonts
nacelle-desktop uses:

- a display font for the UI (headers, clock, keyboard) — free sci-fi
  choices under the SIL Open Font License include Orbitron and Oxanium,
- `fira_mono.ttf` — terminal font; Fira Mono is under the SIL OFL,
  available from Mozilla or Google Fonts.

Fonts under the SIL OFL may be redistributed only together with the text
of that license — which is why no font file is ever committed to this
repository (see .gitignore). Without these files nacelle-desktop falls
back to the closest system fonts.

**This directory is read at INSTALL time, not at run time.** `make install`
copies what is here into `$(PREFIX)/share/fonts/nacelle-desktop` — which is
`~/.local/share/fonts/nacelle-desktop` for a user install and
`/usr/local/share/fonts/nacelle-desktop` under `sudo`, and the program
searches both like any other system font directory.

The lookup used to include a bare relative `fonts` as well — this folder,
but resolved against whatever directory the program happened to be started
from, so which typefaces the interface came up in depended on the shell's
`cd`, and a stranger's `fonts/` in a directory you had cd'd into was picked
up in silence. To run a checkout against these files without installing
them, name the directory outright instead:

    NACELLE_FONT_DIR=$PWD/fonts cargo run

The variable is ignored unless the path is absolute, since a relative one
would be the old behaviour under a new name.

`NACELLE_FONT_DIR` is read by the toolkit, not by this program, so it needs
a libnacelle at or after the commit that added it — with an older pin in
`Cargo.lock` the variable is simply not looked at, and neither is the
relative `fonts` entry it replaced.
