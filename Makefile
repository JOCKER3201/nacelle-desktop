# nacelle-desktop build system.
#
#   make install       — clean build and install to ~/.local/
#   sudo make install  — clean build and install to /usr/local/ + /etc/xdg/
#
# This installs the PROGRAM and nothing else: the binary, its fonts,
# the icons and the desktop entry. The addons, the assets and the
# CONFIGURATION are separate repositories with installers of their own,
# because they are separately replaceable:
#
#   nacelle-addons  — the addons (scripts and compiled .so)
#   nacelle-themes  — sound themes, layauts, the shell startup file,
#                     and /etc/xdg/nacelle/nacelle-desktop.ron
#
# This file used to install a configuration template of its own, in the
# format that came before RON, into that same /etc/xdg/nacelle. Two
# installers writing one directory is bad enough; the two files were
# also the two FORMATS, and within one directory the .ron is read whole
# and the Key=Value file beside it is not read at all. So every system
# install laid down a file that was dead the moment the other installer
# ran, and nobody would have found out from the outside. One owner, and
# it is the repository that ships the file the program actually reads.
#
# Nothing here writes to ~/.config, to /etc/xdg or to the program's own
# data directory: the program reads its search path where things are
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

# The XDG system configuration directory, named after the nacelle
# FAMILY. Nothing is installed into it from here — nacelle-themes owns
# it — and it is named only so that `uninstall` can sweep up the dead
# Key=Value template earlier releases of THIS Makefile left in it. A
# home prefix has no /etc to own, so it is empty there and the sweep
# does not run.
HOMEDIR    := $(if $(HOME),$(HOME),/nonexistent)
HOMEPREFIX := $(patsubst $(HOMEDIR)/%,inside,$(PREFIX)/)
XDGCONFDIR := $(if $(filter inside,$(HOMEPREFIX)),,$(DESTDIR)/etc/xdg/nacelle)

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
	@# No configuration is installed from here at all. Said out loud
	@# rather than left as an absence, because a file this Makefile used
	@# to write may still be standing where it put it.
	@if [ -n "$(XDGCONFDIR)" ] && [ -e "$(XDGCONFDIR)/nacelle-desktop.conf" ]; then \
		echo "NOTE: $(XDGCONFDIR)/nacelle-desktop.conf was installed by an"; \
		echo "      earlier release of this Makefile and is no longer read —"; \
		echo "      within one directory the .ron file answers whole. Nothing"; \
		echo "      has been deleted; 'make uninstall' removes it if it is"; \
		echo "      still the shipped one."; \
	fi
	@echo "no configuration installed — nacelle-themes ships nacelle-desktop.ron"
	-@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	-@gtk-update-icon-cache -f "$(ICONDIR)" 2>/dev/null || true
	rm -rf target
	@echo "nacelle-desktop installed at $(BINDIR)/nacelle-desktop"

uninstall:
	rm -f "$(BINDIR)/nacelle-desktop"
	rm -rf "$(FONTDIR)"
	@# $(PREFIX)/share/nacelle is deliberately NOT removed: it
	@# holds the widgets and themes, which are installed from their own
	@# repositories and may have been edited. Uninstalling the program
	@# must not delete them — use their own `make uninstall`.
	rm -f "$(APPDIR)/nacelle-desktop.desktop"
	@for s in $(ICON_SIZES); do \
		rm -f "$(ICONDIR)/$${s}x$${s}/apps/nacelle-desktop.png"; \
	done
	@# Sweeping up after earlier releases of this Makefile, which
	@# installed a Key=Value template here. Nothing installs it any
	@# more, so this is the only place it is mentioned — and it goes
	@# only if it is still the shipped one, which is the whole reason
	@# assets/nacelle-desktop.conf is still in the repository: a site
	@# default somebody wrote is not ours to throw away, and that file
	@# is the only way to tell the two apart.
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
