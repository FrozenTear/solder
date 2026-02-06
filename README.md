# Solder

A visual node graph editor for PipeWire audio connections, built with Rust and Iced.

> **Warning:** This is a hobby project for my own personal use. It may be broken, incomplete, or change without notice. Use at your own risk.

## What it does

Solder provides a visual interface for managing PipeWire audio/MIDI routing. It displays audio nodes and their ports as an interactive graph, letting you connect and disconnect them by dragging between ports.

## Features

- Real-time PipeWire graph visualization
- Drag-to-connect port routing
- Auto-layout (sources left, sinks right)
- Pan and zoom
- Undo/redo
- Search with hotkey activation
- Port type coloring (Audio, MIDI, Video)
- Configurable node positions

## Install

Download and install the latest binary:

```sh
curl -L https://github.com/FrozenTear/solder/releases/latest/download/solder -o ~/.local/bin/solder
chmod +x ~/.local/bin/solder
```

To add it to your app launcher, also install the desktop entry and icon:

```sh
curl -L https://raw.githubusercontent.com/FrozenTear/solder/master/assets/solder.desktop -o ~/.local/share/applications/solder.desktop
curl -L https://raw.githubusercontent.com/FrozenTear/solder/master/assets/solder.svg -o ~/.local/share/icons/hicolor/scalable/apps/solder.svg
gtk-update-icon-cache ~/.local/share/icons/hicolor/ 2>/dev/null; true
```

Requires PipeWire to be running and `~/.local/bin` on your `PATH`.

## Building from source

```
cargo build --release
```

Requires PipeWire development libraries.
