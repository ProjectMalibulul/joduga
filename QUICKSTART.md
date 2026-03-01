# Quick Start Guide for Developers

## 🎯 Goal
Get the audio engine running in **under 5 minutes** and understand how to:
1. Add a new DSP node
2. Send parameter updates
3. Wire nodes together

---

## Step 1: Build & Run (2 minutes)

```bash
# Clone or navigate to the repo
cd joduga

# Build everything (Rust + C++)
cargo build --release

# Run the test
cargo run --release
```

**Expected Output:**
```
✓ Audio engine started (real-time thread running)
⏳ Processing audio for 5 seconds...
🎛️  Testing parameter updates:
  • Setting oscillator frequency to 880 Hz
✅ Test completed successfully!
```

---

## Step 2: Understanding the Test Graph

Open `rust/src/main.rs` and look at the graph construction:

```rust
// Create nodes
let osc_node = Node {
    id: 0,
    node_type: NodeType::Oscillator,
    num_inputs: 0,
    num_outputs: 1,
    parameters: HashMap::new(),
};

// Wire them: Osc -> Filter -> Gain -> Output
graph.add_edge(Edge {
    from_node_id: 0,
    from_output_idx: 0,
    to_node_id: 1,
    to_input_idx: 0,
}).unwrap();
```

---

## Step 3: Add a New DSP Node (10 minutes)

### 3.1 Define the Node Type

**File:** `rust/src/ffi.rs`

```rust
#[repr(C)]
pub enum NodeType {
    Oscillator = 0,
    Filter = 1,
    Gain = 2,
    Output = 3,
    Delay = 4,  // ← Add this
}
```

**File:** `cpp/include/audio_engine.h`

```c
typedef enum {
    NODE_TYPE_OSCILLATOR = 0,
    NODE_TYPE_FILTER = 1,
    NODE_TYPE_GAIN = 2,
    NODE_TYPE_OUTPUT = 3,
    NODE_TYPE_DELAY = 4,  // ← Add this
} NodeType;
```

### 3.2 Create the C++ Node Class

**File:** `cpp/include/nodes/delay.h`

```cpp
#pragma once
#include "audio_node.h"
#include <vector>

class DelayNode : public AudioNode {
private:
    std::vector<float> delay_buffer;
    uint32_t write_pos = 0;
    float delay_time_ms = 500.0f;
    uint32_t delay_samples = 0;

public:
    DelayNode() : AudioNode() {
        num_inputs = 1;
        num_outputs = 1;
        delay_buffer.resize(48000);  // 1 second max delay at 48kHz
    }

    void set_param(uint32_t param_hash, float value) override {
        if (param_hash == ParamHash::DELAY_TIME) {
            delay_time_ms = value;
            delay_samples = (uint32_t)(delay_time_ms * 0.001f * sample_rate);
        }
    }

    void reset() override {
        std::fill(delay_buffer.begin(), delay_buffer.end(), 0.0f);
        write_pos = 0;
    }

    void process(
        const float* const* inputs,
        float** outputs,
        uint32_t num_samples,
        const ParamUpdateCmd* pending_params,
        uint32_t num_params
    ) override {
        apply_pending_params(pending_params, num_params);

        const float* in = inputs[0];
        float* out = outputs[0];

        for (uint32_t i = 0; i < num_samples; ++i) {
            // Read from delay line
            uint32_t read_pos = (write_pos + delay_buffer.size() - delay_samples) % delay_buffer.size();
            out[i] = delay_buffer[read_pos];

            // Write input to delay line
            delay_buffer[write_pos] = in[i];
            write_pos = (write_pos + 1) % delay_buffer.size();
        }
    }
};

// Add to ParamHash namespace in audio_node.h:
namespace ParamHash {
    constexpr uint32_t DELAY_TIME = 2166136261u ^ ('d' ^ 'e' ^ 'l');
}
```

**File:** `cpp/src/nodes/delay.cpp`

```cpp
#include "nodes/delay.h"
```

### 3.3 Register the Node in the Engine

**File:** `cpp/src/audio_engine.cpp`

Add to the `#include` section:
```cpp
#include "nodes/delay.h"
```

Add to the `create_node()` function:
```cpp
case NODE_TYPE_DELAY:
    node = std::make_unique<DelayNode>();
    break;
```

### 3.4 Update CMakeLists.txt

**File:** `CMakeLists.txt`

```cmake
add_library(joduga_audio SHARED
  # ...existing files...
  cpp/src/nodes/delay.cpp  # ← Add this
)
```

### 3.5 Test It

**File:** `rust/src/main.rs`

```rust
// Add the delay node
let delay_node = Node {
    id: 4,
    node_type: NodeType::Delay,
    num_inputs: 1,
    num_outputs: 1,
    parameters: HashMap::new(),
};
graph.add_node(delay_node).unwrap();

// Wire it: Gain -> Delay -> Output
graph.add_edge(Edge {
    from_node_id: 2,
    from_output_idx: 0,
    to_node_id: 4,
    to_input_idx: 0,
}).unwrap();

graph.add_edge(Edge {
    from_node_id: 4,
    from_output_idx: 0,
    to_node_id: 3,
    to_input_idx: 0,
}).unwrap();

// Set delay time to 250ms
engine.set_param(4, ParamHash::DELAY_TIME, 250.0).unwrap();
```

Rebuild and run:
```bash
cargo build --release
cargo run --release
```

---

## Step 4: Sending Parameter Updates from Code

```rust
// Get the parameter hash (FNV-1a)
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn param_hash(name: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish() as u32
}

// Send a parameter update
engine.set_param(
    node_id,
    param_hash("cutoff"),
    2000.0
).unwrap();
```

---

## Step 5: Inspecting the Real-Time Thread

While the engine is running:

```bash
# In another terminal
ps -eLo pid,tid,class,rtprio,ni,comm | grep joduga
```

Look for:
- `CLASS = FF` (SCHED_FIFO)
- `RTPRIO = 98` (real-time priority)

---

## Step 6: Debugging DSP Issues

### Enable Debug Logging

**File:** `cpp/src/audio_engine.cpp`

```cpp
void audio_thread_main() {
    // Add at the start
    std::cerr << "Audio thread started on core " << sched_getcpu() << std::endl;
    
    while (is_running) {
        // Add inside the loop
        if (num_params > 0) {
            std::cerr << "Applied " << num_params << " param updates" << std::endl;
        }
    }
}
```

Rebuild and run:
```bash
cargo build
RUST_LOG=debug cargo run
```

---

## Step 7: Adding Audio Device Output (Future)

Currently, the engine processes audio but doesn't write to speakers. To add output:

1. **Add `cpal` to `rust/Cargo.toml`:**
   ```toml
   cpal = "0.15"
   ```

2. **Modify `audio_engine.cpp` to expose a "get output buffer" function:**
   ```cpp
   extern "C" {
       void audio_engine_get_output(AudioEngine* engine, float* buffer, uint32_t num_samples);
   }
   ```

3. **In Rust, create a `cpal` stream:**
   ```rust
   let stream = device.build_output_stream(
       &config,
       move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
           audio_engine_get_output(engine_ptr, data.as_mut_ptr(), data.len() as u32);
       },
       |err| eprintln!("Audio error: {}", err),
   )?;
   ```

---

## Step 8: Next Steps

- **Read `DESIGN.md`** for the full technical architecture
- **Read `TROUBLESHOOTING.md`** if you hit build issues
- **Experiment with parameter smoothing** in the node implementations
- **Profile the audio thread** with `perf` or `valgrind`

---

## Quick Reference: File Map

| File | Purpose |
|------|---------|
| `rust/src/main.rs` | Test harness entry point |
| `rust/src/shadow_graph.rs` | Graph validation & topological sort |
| `rust/src/audio_engine_wrapper.rs` | Safe Rust FFI wrapper |
| `cpp/src/audio_engine.cpp` | Core audio thread loop |
| `cpp/include/audio_node.h` | Base DSP node class |
| `cpp/include/nodes/*.h` | Individual DSP node implementations |

---

**Happy coding! 🎹✨**

If you get stuck, check the inline comments—they're verbose for a reason.
