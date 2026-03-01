/// Windows real-time thread management.

#include "platform/rt_platform.h"

#ifdef _WIN32
#include <windows.h>
#include <iostream>

namespace rt_platform {

int set_thread_rt_priority(uint32_t cpu_core) {
    // Set to THREAD_PRIORITY_TIME_CRITICAL
    HANDLE thread = GetCurrentThread();
    if (!SetThreadPriority(thread, THREAD_PRIORITY_TIME_CRITICAL)) {
        std::cerr << "Failed to set thread priority" << std::endl;
        return -1;
    }

    // Pin to CPU core
    if (pin_thread_to_core(cpu_core) != 0) {
        std::cerr << "Warning: Failed to pin thread to core " << cpu_core << std::endl;
    }

    return 0;
}

int pin_thread_to_core(uint32_t core_id) {
    HANDLE thread = GetCurrentThread();
    DWORD_PTR mask = 1ULL << core_id;

    DWORD_PTR result = SetThreadAffinityMask(thread, mask);
    if (result == 0) {
        std::cerr << "Failed to set CPU affinity" << std::endl;
        return -1;
    }

    return 0;
}

void sleep_precise_ns(uint64_t nanoseconds) {
    LARGE_INTEGER frequency, start, end;
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&start);

    DWORD milliseconds = static_cast<DWORD>(nanoseconds / 1000000ULL);
    Sleep(milliseconds);

    // Spin-loop for sub-millisecond precision
    QueryPerformanceCounter(&end);
    uint64_t elapsed_ns = (end.QuadPart - start.QuadPart) * 1000000000ULL / frequency.QuadPart;
    while (elapsed_ns < nanoseconds) {
        QueryPerformanceCounter(&end);
        elapsed_ns = (end.QuadPart - start.QuadPart) * 1000000000ULL / frequency.QuadPart;
    }
}

}  // namespace rt_platform

#else

namespace rt_platform {

int set_thread_rt_priority(uint32_t cpu_core) {
    return -1;  // Not implemented on this platform
}

int pin_thread_to_core(uint32_t core_id) {
    return -1;
}

void sleep_precise_ns(uint64_t nanoseconds) {
    // Fallback: not available
}

}

#endif
