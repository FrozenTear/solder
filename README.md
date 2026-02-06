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

```sh
curl -fsSL https://raw.githubusercontent.com/FrozenTear/solder/master/install.sh | sh
```

This installs the binary to `~/.local/bin/`, the desktop entry and icon to the right places so it shows up in your app launcher.

Requires PipeWire to be running and `~/.local/bin` on your `PATH`.

## Building from source

```
cargo build --release
```

Requires PipeWire development libraries.
