# Joduga - Real-Time Node-Based Audio Synthesizer

A high-performance, modular audio synthesizer built with **Rust**, **C++**, and **Tauri**. Designed for ultra-low latency DSP processing with lock-free inter-thread communication.

---

## 🎯 Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  FRONTEND (Tauri + React + ReactFlow)                   │
│  - Visual node graph editor                             │
│  - Parameter controls (knobs, sliders)                  │
│  - MIDI keyboard visualization                          │
└──────────────────┬──────────────────────────────────────┘
                   │ Tauri IPC
┌──────────────────▼──────────────────────────────────────┐
│  RUST MIDDLEWARE                                        │
│  ┌───────────────────────────────────────────────────┐  │
│  │ • Shadow Graph (validation, topological sort)     │  │
│  │ • Lock-free command queues (SPSC ring buffers)    │  │
│  │ • MIDI input handling (midir)                     │  │
│  │ • FFI bridge to C++ audio engine                  │  │
│  └───────────────────────────────────────────────────┘  │
└──────────────────┬──────────────────────────────────────┘
                   │ Lock-Free Queues (Zero-Copy FFI)
┌──────────────────▼──────────────────────────────────────┐
│  C++ AUDIO ENGINE                                       │
│  ┌───────────────────────────────────────────────────┐  │
│  │ • SCHED_FIFO real-time thread (Linux)             │  │
│  │ • Block-based DSP processing (256 samples/block)  │  │
│  │ • Cache-coherent node graph execution             │  │
│  │ • Zero-allocation audio callback                  │  │
│  └───────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────┐  │
│  │ DSP Nodes: Oscillator, Filter, Gain, ADSR, etc.  │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 🚀 Key Features

### 1. **Lock-Free Real-Time Architecture**
- **Zero mutexes** on the audio thread—all synchronization uses lock-free SPSC ring buffers
- Audio thread runs at **SCHED_FIFO priority** (Linux) or **THREAD_PRIORITY_TIME_CRITICAL** (Windows)
- **CPU core affinity** to isolate the audio thread from system interrupts

### 2. **Hot-Swappable Parameter Updates**
- Turn knobs and adjust parameters **while audio is playing** without dropouts
- Parameter changes are batched and applied at block boundaries for smooth transitions
- Automatic parameter smoothing prevents clicks/pops

### 3. **Graph Validation & Cycle Detection**
- Rust layer validates the graph topology before sending to C++
- **Topological sorting** ensures correct execution order
- Detects and rejects **algebraic loops** (feedback without delay nodes)

### 4. **MIDI Integration**
- Native MIDI input support via `midir`
- MIDI events bypass the UI and are injected directly into the audio thread
- Supports Note On/Off, Control Change, Pitch Bend

### 5. **Block-Based DSP Processing**
- Processes **256-512 samples at a time** for SIMD optimization opportunities
- Minimizes FFI boundary crossings (only occurs during graph updates, not per-sample)

---

## 📦 Project Structure

```
joduga/
├── Cargo.toml              # Rust workspace root
├── CMakeLists.txt          # C++ build configuration
│
├── rust/                   # Rust middleware & FFI layer
│   ├── src/
│   │   ├── lib.rs                    # Main library
│   │   ├── main.rs                   # Test entry point
│   │   ├── lockfree_queue.rs         # SPSC ring buffer
│   │   ├── ffi.rs                    # C++ FFI bindings
│   │   ├── shadow_graph.rs           # Graph validation & sorting
│   │   ├── audio_engine_wrapper.rs   # Safe Rust wrapper
│   │   └── midi_input.rs             # MIDI input handling
│   ├── build.rs            # CMake build script
│   └── Cargo.toml
│
└── cpp/                    # C++ audio DSP engine
    ├── include/
    │   ├── audio_engine.h            # C FFI interface
    │   ├── audio_node.h              # Base node class
    │   ├── nodes/
    │   │   ├── oscillator.h          # Sine oscillator
    │   │   ├── filter.h              # 2nd-order butterworth LPF
    │   │   └── gain.h                # Linear amplitude scaler
    │   └── platform/
    │       └── rt_platform.h         # Real-time thread utilities
    │
    └── src/
        ├── audio_engine.cpp          # Core engine & audio loop
        ├── nodes/
        │   ├── oscillator.cpp
        │   ├── filter.cpp
        │   └── gain.cpp
        └── platform/
            ├── linux_rt.cpp          # Linux SCHED_FIFO
            └── windows_rt.cpp        # Windows RT priority
```

---

## 🛠️ Building & Running

### Prerequisites

**Linux:**
```bash
# Install dependencies
sudo apt install build-essential cmake libasound2-dev
```

**Pop!_OS / Ubuntu:**
```bash
# For real-time audio permissions
sudo usermod -a -G audio $USER
```

### Build

```bash
cd joduga
cargo build --release
```

This will:
1. Use CMake to compile the C++ audio engine (`libjoduga_audio.so`)
2. Compile the Rust middleware and link against the C++ library

### Run the Test

```bash
cargo run --release
```

Expected output:
```
🎵 Joduga Audio Engine Test
============================

✓ Graph created with 4 nodes and 3 edges
✓ Graph validated (no cycles detected)
✓ Graph compiled successfully
  Execution order: [0, 1, 2, 3]

🔊 Initializing audio engine...
✓ Audio engine initialized
  Sample rate: 48000 Hz
  Block size: 256 samples
✓ Audio engine started (real-time thread running)

⏳ Processing audio for 5 seconds...

🎛️  Testing parameter updates:
  • Setting oscillator frequency to 880 Hz
  • Setting filter cutoff to 2000 Hz

🛑 Stopping audio engine...
✓ Audio engine stopped gracefully

✅ Test completed successfully!
```

---

## 🔬 System Design Decisions

### Why C++ Owns the Audio Thread?
- **Minimal latency:** C++ can immediately set SCHED_FIFO and pin to a CPU core
- **Zero FFI overhead during processing:** The audio callback never crosses back into Rust
- **Deterministic:** No Rust runtime interactions (GC, async) during audio processing

### Why Lock-Free Queues?
- **Priority inversion avoidance:** Traditional mutexes can cause the audio thread to block
- **Wait-free reads:** The audio thread can always read without blocking
- **Cache-coherent:** Ring buffers are allocated contiguously for optimal cache performance

### Why Topological Sort in Rust?
- **Safety:** Rust's type system prevents invalid graph mutations
- **Validation before execution:** C++ trusts the Rust-provided execution order
- **Debugging:** Easier to inspect graph topology in high-level Rust code

---

## 🎛️ DSP Nodes Implemented

| Node Type   | Description                          | Parameters                |
|-------------|--------------------------------------|---------------------------|
| Oscillator  | Sine wave generator                  | `frequency` (Hz)          |
| Filter      | 2nd-order Butterworth low-pass       | `cutoff` (Hz), `resonance`|
| Gain        | Linear amplitude scaler              | `gain` (0.0-2.0)          |
| Output      | Final output (DAC interface)         | None                      |

---

## 🐛 Known Limitations (MVP)

- ❌ No audio device I/O yet (currently just processes in a loop)
- ❌ ADSR envelopes not implemented
- ❌ No reverb/delay effects
- ❌ Frontend (Tauri/React) not integrated yet
- ❌ Output node doesn't write to speakers (stub implementation)

---

## 🚧 Next Steps

1. **Audio Device I/O:** Integrate `cpal` or RtAudio to write processed audio to the system's audio output
2. **ADSR Envelope:** Implement attack/decay/sustain/release envelope for MIDI-triggered notes
3. **Frontend:** Build the React node graph editor with ReactFlow
4. **More Nodes:** Add reverb, delay, more oscillator types (saw, square, triangle)
5. **Node Deletion:** Support removing nodes dynamically from the running graph

---

## 📜 License

MIT License - See `LICENSE` file for details

---

## 🙏 Acknowledgments

This project is built for a **CS Engineering student** who is also a **pianist**. The design prioritizes:
- **Low-latency MIDI response** (< 5ms latency from key press to audio output)
- **Cache-friendly DSP code** (block-based processing, SIMD-ready)
- **Deterministic real-time behavior** (no allocations on audio thread)

Built with love for systems programming and digital signal processing. 🎹✨
