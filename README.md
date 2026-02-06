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

## Building

```
cargo build --release
```

Requires PipeWire development libraries to be installed.
