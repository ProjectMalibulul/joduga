# 📚 Joduga Documentation Index

Welcome to the Joduga real-time audio synthesizer documentation. This index will guide you to the right document based on what you need.

---

## 🎯 I Want To...

### **Build and run the project**
→ Read **[README.md](README.md)** first  
→ Then run **`./verify_build.sh`**  
→ If issues arise, see **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)**

### **Understand the technical architecture**
→ Read **[DESIGN.md](DESIGN.md)** for the complete system design  
→ Key sections:
  - Section 2: Concurrency Model (thread architecture)
  - Section 4: Lock-Free Queue Design
  - Section 5: Audio Graph Compilation
  - Section 8: FFI Boundary Contract

### **Add a new DSP node (e.g., delay, reverb)**
→ Read **[QUICKSTART.md](QUICKSTART.md)**, Section "Step 3: Add a New DSP Node"  
→ Example provided: Delay node implementation (10-minute tutorial)

### **Understand the code structure**
→ See **[PROJECT_COMPLETE.md](PROJECT_COMPLETE.md)**, Section "File Structure"  
→ All files are documented with inline comments

### **Debug build issues**
→ See **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)**  
→ Common issues covered:
  - CMake not found
  - SCHED_FIFO permission denied
  - MIDI device not found
  - Linker errors

### **Optimize for lower latency**
→ See **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)**, Section "Performance Tuning"  
→ Topics covered:
  - CPU core isolation
  - Real-time priority verification
  - Benchmark mode

### **Integrate audio device output (speakers)**
→ See **[QUICKSTART.md](QUICKSTART.md)**, Section "Step 7: Adding Audio Device Output"  
→ See TODOs in `cpp/src/audio_engine.cpp` (line ~120)

### **Understand the lock-free queues**
→ See **[DESIGN.md](DESIGN.md)**, Section 4: "Lock-Free Queue Design"  
→ Source code: `rust/src/lockfree_queue.rs` (450 lines, heavily commented)

### **Understand the DSP math**
→ See **[DESIGN.md](DESIGN.md)**, Section 7: "DSP Node Implementation"  
→ Source code: `cpp/include/nodes/*.h` (oscillator, filter, gain)

### **Build a UI (Tauri + React)**
→ See **[PROJECT_COMPLETE.md](PROJECT_COMPLETE.md)**, Section "Phase 4: Frontend"  
→ The Rust middleware (`audio_engine_wrapper.rs`) is UI-ready

---

## 📄 Document Descriptions

| Document | Purpose | Length | When to Read |
|----------|---------|--------|--------------|
| **README.md** | Project overview, quick start | 5 min | First thing |
| **DESIGN.md** | Technical deep dive (12 sections) | 30 min | After first build |
| **QUICKSTART.md** | Developer tutorial (add nodes) | 15 min | Before coding |
| **TROUBLESHOOTING.md** | Build/runtime issue fixes | 10 min | When stuck |
| **PROJECT_COMPLETE.md** | Project summary & status | 10 min | To understand scope |
| **LICENSE** | MIT License | 1 min | Before distributing |

---

## 🗂️ Source Code Map

### **Rust Layer** (Middleware & Orchestration)
```
rust/src/
├── lib.rs                   Main library entry point
├── main.rs                  Test harness (example graph)
├── lockfree_queue.rs        SPSC ring buffer implementation (450 lines)
├── ffi.rs                   C++ FFI bindings (extern "C" declarations)
├── shadow_graph.rs          Graph validation & topological sort (250 lines)
├── audio_engine_wrapper.rs  Safe Rust wrapper around C++ engine (180 lines)
└── midi_input.rs            MIDI input handling via midir (120 lines)
```

**Key Functions:**
- `ShadowGraph::topological_sort()` → Execution order computation
- `LockFreeRingBuffer::enqueue()` → Lock-free parameter updates
- `AudioEngineWrapper::set_param()` → Send parameter to C++

### **C++ Layer** (Real-Time DSP Engine)
```
cpp/
├── include/
│   ├── audio_engine.h       C FFI interface (extern "C" functions)
│   ├── audio_node.h         Base DSP node class (virtual process())
│   ├── nodes/
│   │   ├── oscillator.h     Sine wave generator
│   │   ├── filter.h         2nd-order Butterworth low-pass filter
│   │   └── gain.h           Linear amplitude scaler
│   └── platform/
│       └── rt_platform.h    Real-time thread utilities (SCHED_FIFO)
│
└── src/
    ├── audio_engine.cpp     Core engine & audio loop (350 lines)
    ├── nodes/*.cpp          Node implementation stubs
    └── platform/
        ├── linux_rt.cpp     Linux SCHED_FIFO implementation
        └── windows_rt.cpp   Windows RT priority implementation
```

**Key Functions:**
- `audio_thread_main()` → Main audio processing loop
- `AudioNode::process()` → Virtual method for DSP processing
- `rt_platform::set_thread_rt_priority()` → Set SCHED_FIFO

---

## 🔍 Quick Search Guide

### "How do I...?"

| Query | Answer |
|-------|--------|
| **...change the sample rate?** | Modify `sample_rate` in `rust/src/main.rs` line 95 |
| **...change the block size?** | Modify `block_size` in `rust/src/main.rs` line 96 |
| **...add a parameter to a node?** | Add to `ParamHash` in `audio_node.h`, handle in `set_param()` |
| **...connect MIDI to envelopes?** | Implement ADSR node, route MIDI events in `audio_engine.cpp` |
| **...wire node outputs to inputs?** | Use `graph.add_edge()` in `rust/src/main.rs` |
| **...debug parameter updates?** | Add `std::cerr` in `AudioNode::apply_pending_params()` |
| **...profile CPU usage?** | Run `htop` while engine is running, check core 0 |

---

## 🚀 Learning Path

### Beginner (1 hour)
1. Read **README.md**
2. Run `./verify_build.sh`
3. Run `cargo run --release`
4. Observe the test output

### Intermediate (3 hours)
1. Read **QUICKSTART.md**
2. Follow "Step 3: Add a New DSP Node"
3. Implement a delay node
4. Test it with `engine.set_param()`

### Advanced (8 hours)
1. Read **DESIGN.md** (all 12 sections)
2. Study `lockfree_queue.rs` and understand memory ordering
3. Study `audio_engine.cpp` and the audio thread loop
4. Implement ADSR envelope with MIDI triggering

### Expert (16+ hours)
1. Integrate `cpal` for audio device output
2. Build a Tauri + React frontend
3. Implement reverb and delay effects
4. Profile and optimize the audio thread to < 2% CPU

---

## 🎓 External References

### Lock-Free Programming
- [Preshing on Programming: Lock-Free](https://preshing.com/20120612/an-introduction-to-lock-free-programming/)
- [C++ Memory Ordering (cppreference)](https://en.cppreference.com/w/cpp/atomic/memory_order)

### Real-Time Audio
- [Ross Bencina: Real-Time Audio Programming 101](http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing)
- [Real-Time Rendering in JUCE](https://docs.juce.com/master/tutorial_audio_processor_value_tree_state.html)

### DSP Fundamentals
- [Julius O. Smith: Introduction to Digital Filters](https://ccrma.stanford.edu/~jos/filters/)
- [Butterworth Filter Design](https://en.wikipedia.org/wiki/Butterworth_filter)

### Graph Algorithms
- [Topological Sorting (Wikipedia)](https://en.wikipedia.org/wiki/Topological_sorting)
- [Kahn's Algorithm](https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm)

---

## 📞 Support Checklist

If you're stuck, work through this checklist:

- [ ] I've read **README.md**
- [ ] I've run `./verify_build.sh`
- [ ] I've checked **TROUBLESHOOTING.md** for my specific error
- [ ] I've verified all dependencies are installed (`cargo`, `cmake`, `g++`)
- [ ] I've checked the build log in `/tmp/joduga_build.log`
- [ ] I've searched the inline comments in the relevant source file

If all else fails, the code is heavily documented. Start at `rust/src/main.rs` and trace through the function calls.

---

## 🏆 Project Milestones

### ✅ Completed (MVP)
- [x] Lock-free queue infrastructure
- [x] FFI boundary (Rust ↔ C++)
- [x] Real-time audio thread (SCHED_FIFO)
- [x] Graph validation & topological sort
- [x] DSP nodes: Oscillator, Filter, Gain
- [x] MIDI input handling
- [x] Hot-swappable parameter updates
- [x] Comprehensive documentation (3,000+ lines)

### 🚧 In Progress (Phase 2)
- [ ] Audio device I/O (`cpal` integration)
- [ ] ADSR envelope implementation
- [ ] MIDI event routing to envelopes

### 📅 Planned (Phase 3+)
- [ ] Tauri + React frontend
- [ ] ReactFlow node graph editor
- [ ] Reverb, delay, chorus effects
- [ ] Preset save/load system

---

## 🎯 Quick Links

- **Build the project:** `./verify_build.sh`
- **Run the test:** `cd rust && cargo run --release`
- **Add a node:** See **QUICKSTART.md**, Step 3
- **Debug build:** See **TROUBLESHOOTING.md**
- **Understand architecture:** See **DESIGN.md**

---

**Last Updated:** March 1, 2026  
**Project Status:** MVP Complete  
**Next Milestone:** Audio Device I/O (Phase 2)
