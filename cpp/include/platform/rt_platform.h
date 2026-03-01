/// Platform-specific real-time thread utilities.
/// Abstracts SCHED_FIFO (Linux) and THREAD_PRIORITY (Windows) differences.

#pragma once

#include <cstdint>
#include <thread>

namespace rt_platform {

/// Set the current thread to real-time priority.
/// On Linux: SCHED_FIFO
/// On Windows: THREAD_PRIORITY_TIME_CRITICAL
/// Returns 0 on success, -1 on failure.
int set_thread_rt_priority(uint32_t cpu_core = 0);

/// Detach the current thread from all but one CPU core.
/// This isolates the audio thread to prevent interference.
int pin_thread_to_core(uint32_t core_id);

/// Sleep for a given number of nanoseconds with minimal jitter.
void sleep_precise_ns(uint64_t nanoseconds);

}  // namespace rt_platform
