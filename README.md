# ng-term

Nowoczesna alternatywa dla [eDEX-UI](https://github.com/GitSquared/edex-ui), napisana od zera
w **Rust** z renderowaniem przez **Vulkan API** (`ash`). Zero Electrona, zero przeglądarki —
cały interfejs (tekst, ramki, wykresy, glob) to jeden pipeline graficzny rysujący trójkąty
z atlasu glifów.

Wygląd odwzorowuje eDEX-UI 1:1 (motyw domyślny: **tron**):

- **Lewa kolumna** — zegar, data/uptime/zasilanie, inspektor sprzętu (MANUFACTURER / MODEL /
  CHASSIS), wykresy CPU per rdzeń, MEMORY (siatka kropek + swap), TOP PROCESSES.
- **Środek** — MAIN SHELL: pełnoprawny emulator terminala (PTY + parser VT/ANSI, kolory
  16/256/truecolor, alternatywny ekran, scrollback) w ramce ze ściętymi rogami i skośnymi
  zakładkami.
- **Prawa kolumna** — NETWORK STATUS (STATE / IPV4 / PING).
- **Dół** — FILESYSTEM (siatka ikon śledząca katalog roboczy powłoki) oraz klikalna
  klawiatura ekranowa (układ en-US, lepkie SHIFT/CTRL/ALT/FN).
- Sekwencja bootowania przy starcie, migający kursor, migające dwukropki zegara.

## Budowanie i instalacja

```sh
make install        # czysty build + instalacja do ~/.local/bin
sudo make install   # czysty build + instalacja do /usr/local/bin
```

`make install` zawsze usuwa stary katalog `target/`, buduje od zera, instaluje
binarkę, fonty z `./fonts` (jeśli są), ikonę (hicolor 48–512 px) oraz plik
`.desktop` (wpis w menu aplikacji), po czym sprząta build po sobie. Prefiks
można nadpisać: `make install PREFIX=/opt/ng-term`. Odinstalowanie:
`make uninstall` / `sudo make uninstall`.

Budowanie bez instalacji:

```sh
cargo build --release
./target/release/ng-term
```

Wymagania: sterownik Vulkan (`libvulkan`), Linux (PTY przez `/dev/ptmx`, dane z `/proc`
i `/sys`). Shadery GLSL są kompilowane do SPIR-V w czasie startu przez `naga` — nie
potrzeba `glslc` ani `shaderc`.

## Fonty

eDEX-UI używa krojów **United Sans** (UI) i **Fira Mono** (terminal). ng-term szuka fontów
w tej kolejności:

1. zmienne `NGTERM_FONT_UI` / `NGTERM_FONT_MONO` (ścieżki do plików .ttf/.otf),
2. katalog `./fonts`,
3. fonty systemowe (Fira Mono/Code, JetBrains Mono, DejaVu Sans Mono itd.).

Aby uzyskać wygląd identyczny z eDEX, przekonwertuj fonty z repozytorium eDEX-UI
(`src/assets/fonts/*.woff2`) do TTF i wrzuć do `./fonts`, np.:

```sh
woff2_decompress ../edex-ui/src/assets/fonts/united_sans_medium.woff2
woff2_decompress ../edex-ui/src/assets/fonts/fira_mono.woff2
mv ../edex-ui/src/assets/fonts/*.ttf fonts/
```

## Konfiguracja (~/.config/ng-term)

Przy pierwszym uruchomieniu program tworzy `~/.config/ng-term/` z plikiem
`ng-term.conf` i katalogiem `themes/` (wraz z przykładowym motywem `tron`).
Każdy motyw to katalog w `themes/` zawierający:

- `meta` — metaplik z polem `Name=` (ta nazwa idzie do `ng-term.conf`),
- `*.css` — styl: blok `:root` (`--color-r/g/b`, `--background`, `--grey`),
  blok `terminal` (`foreground`, `background`, `cursor`), opcjonalnie blok
  `palette` (`color0`–`color15`),
- `*.layaut` — układ paneli: `panel = x y szerokość wysokość` w jednostkach
  vw/vh (panele: `left_col`, `shell`, `right_col`, `filesystem`, `keyboard`,
  `control`).

W `ng-term.conf` opcja `Themes=<nazwa>` wybiera motyw po polu `Name=` z jego
metapliku. Pusta wartość lub brak opcji = domyślny motyw wbudowany w program.

## Konfiguracja (zmienne środowiskowe)

| Zmienna | Opis |
|---|---|
| `NGTERM_THEME` | ścieżka do motywu w formacie eDEX-UI (np. `../edex-ui/src/assets/themes/tron.json`) |
| `NGTERM_FONT_UI` / `NGTERM_FONT_MONO` | ścieżki do fontów |
| `NGTERM_OFFLINE` | wyłącza sondę sieci (TCP do 1.1.1.1 co 5 s) |

## Sterowanie

- pisanie — trafia do terminala (pełna klawiatura fizyczna + klikalna ekranowa),
- kółko myszy nad terminalem — scrollback; nad FILESYSTEM — przewijanie listy,
- klik w katalog w FILESYSTEM — nawigacja ręczna; klik w nagłówek panelu — powrót do
  śledzenia katalogu powłoki,
- `F11` — pełny ekran, `Ctrl+Shift+Q` — wyjście (zamknięcie powłoki też kończy program).

## Licencja

GPL-3.0-or-later. Projekt graficzny wzorowany na eDEX-UI (GPL-3.0) autorstwa
Gabriela „GitSquared" Saillarda; kod ng-term jest niezależną implementacją.
