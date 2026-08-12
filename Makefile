# nacelle-desktop build system.
#
#   make install       — clean build and install to ~/.local/
#   sudo make install  — clean build and install to /usr/local/ + /etc/xdg/
#
# This installs the PROGRAM: the binary, its fonts, the icons, the
# desktop entry and — on a system install only — the configuration
# template in /etc/xdg. The addons and the themes are separate
# repositories with installers of their own, because they are
# separately replaceable:
#
#   nacelle-addons  — the addons (scripts and compiled .so)
#   nacelle-themes  — sound themes, layauts, the shell startup file
#
# Nothing here writes to ~/.config or to the program's own data
# directory: the program reads its search path where things are
# installed, and writes to the home directory only when the user
# changes a setting or saves a layaut.
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

# Where the system configuration template goes — the XDG system
# configuration directory, the same place GTK reads settings.ini from
# and Qt its system-scope settings. A SYSTEM install lands it there; a $HOME install
# leaves it empty and installs no template at all, because a home
# prefix has no /etc to own and this program never copies a file into
# ~/.config. With no template the cascade simply has nothing on its
# system end and the program uses what is built into it — the same
# thing GTK does with no /etc/xdg/gtk-3.0/settings.ini.
HOMEDIR    := $(if $(HOME),$(HOME),/nonexistent)
HOMEPREFIX := $(patsubst $(HOMEDIR)/%,inside,$(PREFIX)/)
XDGCONFDIR := $(if $(filter inside,$(HOMEPREFIX)),,$(DESTDIR)/etc/xdg/nacelle-desktop)

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
	@# The configuration template, on the system end of the XDG cascade.
	@# Never overwritten: a site that edited it keeps its edits.
	@if [ -n "$(XDGCONFDIR)" ]; then \
		if [ -e "$(XDGCONFDIR)/nacelle-desktop.conf" ]; then \
			echo "kept (edited) $(XDGCONFDIR)/nacelle-desktop.conf"; \
		else \
			install -Dm644 assets/nacelle-desktop.conf \
				"$(XDGCONFDIR)/nacelle-desktop.conf"; \
			echo "installed configuration template: $(XDGCONFDIR)/nacelle-desktop.conf"; \
		fi; \
	else \
		echo "user install — no system template; nothing written to ~/.config"; \
	fi
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
	@# The configuration template goes only if it is still the shipped
	@# one — a site default somebody wrote is not ours to throw away.
	@if [ -n "$(XDGCONFDIR)" ] && [ -e "$(XDGCONFDIR)/nacelle-desktop.conf" ]; then \
		if cmp -s assets/nacelle-desktop.conf "$(XDGCONFDIR)/nacelle-desktop.conf"; then \
			rm -f "$(XDGCONFDIR)/nacelle-desktop.conf"; \
			rmdir "$(XDGCONFDIR)" 2>/dev/null || true; \
		else \
			echo "kept (edited) $(XDGCONFDIR)/nacelle-desktop.conf"; \
		fi; \
	fi
	-@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@echo "nacelle-desktop uninstalled from $(PREFIX)"

clean:
	rm -rf target
