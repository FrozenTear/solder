PREFIX ?= $(HOME)/.local

.PHONY: build install uninstall

build:
	cargo build --release

install: build
	install -Dm755 target/release/solder $(PREFIX)/bin/solder
	install -Dm644 assets/solder.svg $(PREFIX)/share/icons/hicolor/scalable/apps/solder.svg
	sed -e 's|Exec=solder|Exec=$(PREFIX)/bin/solder|' \
	    -e 's|Icon=solder|Icon=$(PREFIX)/share/icons/hicolor/scalable/apps/solder.svg|' \
	    assets/solder.desktop > /tmp/solder.desktop
	install -Dm644 /tmp/solder.desktop $(PREFIX)/share/applications/solder.desktop
	rm /tmp/solder.desktop

uninstall:
	rm -f $(PREFIX)/bin/solder
	rm -f $(PREFIX)/share/applications/solder.desktop
	rm -f $(PREFIX)/share/icons/hicolor/scalable/apps/solder.svg
