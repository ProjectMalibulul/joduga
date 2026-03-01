# Joduga - Real-Time Node-Based Audio Synthesizer

A high-performance, modular audio synthesizer built with **Rust**, **C++**, and **Tauri + React Flow**. Ultra-low latency DSP processing with lock-free inter-thread communication and a visual node graph editor.

---

## Architecture

```
+-----------------------------------------------------------+
|  FRONTEND  (Tauri + React + React Flow)                   |
|  - Visual node graph editor (67 node types)               |
|  - Parameter sliders (log/linear, live update)            |
|  - Drag-and-drop from categorised sidebar                 |
+------------------------+----------------------------------+
                         | Tauri IPC
+------------------------v----------------------------------+
|  RUST MIDDLEWARE                                          |
|  - Shadow Graph (validation, topological sort)            |
|  - Lock-free SPSC ring buffers (param + MIDI queues)      |
|  - cpal audio output (reads from C++ ring buffer)         |
|  - MIDI input handling (midir)                            |
|  - FFI bridge to C++ audio engine                         |
+------------------------+----------------------------------+
                         | Lock-Free Queues (Zero-Copy FFI)
+------------------------v----------------------------------+
|  C++ AUDIO ENGINE                                         |
|  - SCHED_FIFO real-time thread (Linux)                    |
|  - Block-based DSP (256 samples/block)                    |
|  - Cache-coherent node graph execution                    |
|  - Zero-allocation audio callback                         |
|  - DSP: Oscillator, Filter (biquad+comb), Gain, Output   |
+-----------------------------------------------------------+
```

---

## Features

- **Lock-free real-time audio** -- zero mutexes on the audio thread; SPSC ring buffers for all inter-thread communication.
- **Live parameter tweaking** -- sliders update the C++ engine in real time via lock-free param queue. No dropouts.
- **Graph validation** -- Rust validates topology, detects cycles, and computes execution order before handing off to C++.
- **cpal audio output** -- ring buffer written by C++ is consumed by a cpal stream for system audio playback.
- **MIDI input** -- native MIDI via midir; events injected directly into the audio thread.
- **67-node catalog** -- Oscillators, Filters, Dynamics, Effects, Modulators, Utility, Output.
- **Zed-inspired UI** -- minimal dark theme, monospace typography, clean node design.

---

## Project Structure

```
joduga/
+-- Cargo.toml              # Rust workspace root
+-- CMakeLists.txt          # C++ build config
+-- rust/                   # Rust middleware + FFI
|   +-- src/
|       +-- lib.rs
|       +-- ffi.rs                   # C++ FFI bindings
|       +-- shadow_graph.rs          # Graph validation + topo sort
|       +-- audio_engine_wrapper.rs  # Safe engine wrapper + ring buffer
|       +-- lockfree_queue.rs        # SPSC ring buffer
|       +-- midi_input.rs            # MIDI input
|       +-- main.rs                  # CLI test harness
+-- cpp/                    # C++ audio DSP engine
|   +-- include/
|   +-- src/
|       +-- audio_engine.cpp         # Core engine + audio loop
|       +-- nodes/                   # Oscillator, Filter, Gain
|       +-- platform/                # Linux/Windows RT threads
+-- tauri-ui/               # Tauri + React Flow frontend
    +-- src/
    |   +-- App.tsx                  # Main app layout
    |   +-- AudioNode.tsx            # Custom node component
    |   +-- catalog.ts               # 67 node templates
    |   +-- store.ts                 # Zustand state management
    |   +-- styles.css               # Zed-inspired theme
    |   +-- types.ts                 # Shared TypeScript types
    +-- src-tauri/
        +-- src/main.rs              # Tauri backend (engine + cpal)
```

---

## Building and Running

### Prerequisites

```bash
# Linux (Debian/Ubuntu)
sudo apt install build-essential cmake libasound2-dev nodejs npm

# Tauri CLI
cargo install tauri-cli
```

### Development

```bash
cd joduga/tauri-ui
cargo tauri dev
```

This will:
1. Start the Vite dev server (port 1420)
2. Build the C++ audio engine via CMake
3. Compile the Rust backend and open the Tauri window

### Production Build

```bash
cd joduga/tauri-ui
cargo tauri build
```

### CLI Test (no UI)

```bash
cargo run --release --bin joduga
```

Runs a headless test: Osc -> Filter -> Gain -> Output for a few seconds.

---

## Usage

1. Launch with `cargo tauri dev`
2. Drag nodes from the sidebar onto the canvas (or double-click to add)
3. Connect nodes by dragging between handles (output -> input)
4. Click **Play** to start the audio engine
5. Adjust parameters with sliders -- changes apply in real time
6. Click **Stop** to shut down the engine

Minimal patch: **Sine Oscillator** -> **Speaker Output** -> click Play.

---

## Design Decisions

**C++ owns the audio thread** -- SCHED_FIFO, CPU pinning, zero FFI during processing. The audio callback never crosses back into Rust.

**Lock-free queues** -- avoid priority inversion. The audio thread never blocks. Ring buffers are cache-coherent.

**Topological sort in Rust** -- type-safe graph validation before C++ trusts the execution order.

**cpal for output** -- the C++ engine writes to a shared ring buffer; a cpal stream on the Rust side reads it and sends to the system audio device.

---

## DSP Nodes

| Type       | Implemented                                     |
|------------|------------------------------------------------ |
| Oscillator | Sine, multi-waveform (square, saw, triangle)    |
| Filter     | 2nd-order biquad (LP, HP, BP, notch), comb      |
| Gain       | Linear amplitude scaler                          |
| Output     | Final output (feeds ring buffer -> cpal -> DAC)  |

The catalog includes 67 UI nodes mapped to these 4 engine types. Additional node types (reverb, delay, compressor, etc.) are represented in the UI but map to existing engine types with parameter-driven behaviour.

---

## License

MIT License -- see LICENSE file.
