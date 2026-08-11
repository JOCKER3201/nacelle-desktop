# nacelle-desktop build system.
#
#   make install       — clean build and install to ~/.local/
#   sudo make install  — clean build and install to /usr/local/
#
# This installs the PROGRAM: the binary, its fonts, the icons and the
# desktop entry. The widgets and the themes are separate repositories
# with installers of their own, because they are separately replaceable:
#
#   nacelle-widgets — the widgets (scripts and compiled .so)
#   nacelle-themes  — looks, styles, sound themes, layouts, config
#
# Every install: removes the old build, builds, installs, removes the build.
# The prefix can be overridden: make install PREFIX=/opt/nacelle-desktop

ifeq ($(shell id -u),0)
PREFIX ?= /usr/local
else
PREFIX ?= $(HOME)/.local
endif

BINDIR     = $(DESTDIR)$(PREFIX)/bin
FONTDIR    = $(DESTDIR)$(PREFIX)/share/fonts/nacelle-desktop
APPDIR     = $(DESTDIR)$(PREFIX)/share/applications
ICONDIR    = $(DESTDIR)$(PREFIX)/share/icons/hicolor
ICON_SIZES = 48 64 128 256 512

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

install:
	rm -rf target
	cargo build --release
	install -Dm755 target/release/nacelle-desktop "$(BINDIR)/nacelle-desktop"
	@# Fonts from ./fonts (optional) — into $(PREFIX)/share/fonts/nacelle-desktop,
	@# where the program's font lookup will find them.
	@found=0; \
	for f in fonts/*.ttf fonts/*.otf; do \
		[ -f "$$f" ] || continue; \
		if [ $$found -eq 0 ]; then mkdir -p "$(FONTDIR)"; found=1; fi; \
		install -m644 "$$f" "$(FONTDIR)/"; \
		echo "installed font: $$f"; \
	done; true
	@# Icons (hicolor) + .desktop file with the binary path substituted.
	@for s in $(ICON_SIZES); do \
		install -Dm644 "assets/nacelle-desktop-$$s.png" \
			"$(ICONDIR)/$${s}x$${s}/apps/nacelle-desktop.png"; \
	done
	@mkdir -p "$(APPDIR)"
	sed "s|@BINDIR@|$(PREFIX)/bin|" assets/nacelle-desktop.desktop.in \
		> "$(APPDIR)/nacelle-desktop.desktop"
	@chmod 644 "$(APPDIR)/nacelle-desktop.desktop"
	-@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	-@gtk-update-icon-cache -f "$(ICONDIR)" 2>/dev/null || true
	rm -rf target
	@echo "nacelle-desktop installed at $(BINDIR)/nacelle-desktop"

uninstall:
	rm -f "$(BINDIR)/nacelle-desktop"
	rm -rf "$(FONTDIR)"
	@# $(PREFIX)/share/nacelle-desktop is deliberately NOT removed: it
	@# holds the widgets and themes, which are installed from their own
	@# repositories and may have been edited. Uninstalling the program
	@# must not delete them — use their own `make uninstall`.
	rm -f "$(APPDIR)/nacelle-desktop.desktop"
	@for s in $(ICON_SIZES); do \
		rm -f "$(ICONDIR)/$${s}x$${s}/apps/nacelle-desktop.png"; \
	done
	-@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@echo "nacelle-desktop uninstalled from $(PREFIX)"

clean:
	rm -rf target
