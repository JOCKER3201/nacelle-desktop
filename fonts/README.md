# fonts/

Drop .ttf/.otf files here to override the fonts nacelle-desktop uses:

- a display font for the UI (headers, clock, keyboard) — free sci-fi
  choices under the SIL Open Font License include Orbitron and Oxanium,
- `fira_mono.ttf` — terminal font; Fira Mono is under the SIL OFL,
  available from Mozilla or Google Fonts.

Fonts under the SIL OFL may be redistributed only together with the text
of that license — which is why no font file is ever committed to this
repository (see .gitignore). Without these files nacelle-desktop falls
back to the closest system fonts.
