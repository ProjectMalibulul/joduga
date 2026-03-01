# Build & Runtime Troubleshooting Guide

## Common Build Issues

### 1. CMake Not Found
```bash
sudo apt install cmake
```

### 2. C++ Compiler Issues
```bash
sudo apt install build-essential g++
```

### 3. ALSA Development Headers Missing (Linux)
```bash
sudo apt install libasound2-dev
```

### 4. "Cannot find -ljoduga_audio" Linker Error

**Cause:** The C++ library failed to build or isn't in the linker search path.

**Fix:**
```bash
# Clean and rebuild
cargo clean
rm -rf build/
cargo build --release
```

Check that `build/lib/libjoduga_audio.so` exists after running cmake.

### 5. Permission Denied When Setting SCHED_FIFO

**Cause:** Linux requires special permissions to use real-time scheduling.

**Fix:**
```bash
# Add your user to the audio group
sudo usermod -a -G audio $USER

# Edit limits.conf
sudo nano /etc/security/limits.conf

# Add these lines:
@audio   -   rtprio     99
@audio   -   memlock    unlimited

# Log out and back in
```

---

## Runtime Issues

### 1. "Failed to set SCHED_FIFO" Warning

This is **non-fatal**—the audio engine will still run, but with lower priority.

To enable real-time scheduling, follow the permission fix above.

### 2. No Audio Output

**Current Status:** The MVP doesn't have audio device I/O yet. The engine processes audio blocks but doesn't write to speakers.

**To Add Audio Output:**
1. Integrate `cpal` for cross-platform audio device access
2. Wire the output node's buffer to `cpal`'s audio callback
3. See `TODO` comments in `audio_engine.cpp`

### 3. MIDI Device Not Found

```bash
# List available MIDI devices
amidi -l

# If no devices shown, check connections
aconnect -i
```

If using a USB MIDI controller, ensure it's recognized:
```bash
lsusb | grep -i midi
```

---

## Performance Tuning

### CPU Core Isolation (Advanced)

For **ultra-low latency**, isolate CPU core 0 from the kernel scheduler:

1. Edit `/etc/default/grub`:
```bash
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash isolcpus=0"
```

2. Update grub and reboot:
```bash
sudo update-grub
sudo reboot
```

3. Now CPU core 0 is dedicated to your audio thread.

### Check Real-Time Priority

```bash
# While the engine is running
ps -eLo pid,tid,class,rtprio,comm | grep joduga
```

You should see `FF` (SCHED_FIFO) and a priority of 98-99.

---

## Debug Mode

For debugging DSP issues, compile in debug mode:

```bash
cargo build
cargo run
```

This disables optimization and adds debug symbols for `gdb`.

---

## Benchmark Mode

To measure performance:

```bash
# Build with maximum optimization
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Run and monitor CPU usage
cargo run --release &
htop
```

The audio thread should use < 10% CPU on a modern system at 48kHz/256 samples.

---

## Getting Help

If you encounter issues not covered here:

1. Check `cargo build` output for specific error messages
2. Run `ldd target/release/joduga` to verify library linkage
3. Check `dmesg` for kernel-level real-time scheduling issues

---

## Known Platform Limitations

| Platform | Real-Time Scheduling | CPU Affinity | Status |
|----------|---------------------|--------------|--------|
| Linux    | ✅ SCHED_FIFO       | ✅ Yes       | Fully supported |
| Windows  | ✅ TIME_CRITICAL    | ✅ Yes       | Implemented, untested |
| macOS    | ⚠️ Limited          | ❌ No API    | Not implemented |

For macOS support, consider using `mach_thread_policy_set` (TODO).
