/// Linux real-time thread management using SCHED_FIFO.

#include "platform/rt_platform.h"
#include <sched.h>
#include <pthread.h>
#include <unistd.h>
#include <cstring>
#include <cerrno>
#include <iostream>

namespace rt_platform {

int set_thread_rt_priority(uint32_t cpu_core) {
    // Get current thread
    pthread_t thread = pthread_self();

    // Set SCHED_FIFO priority (highest non-kernel priority)
    struct sched_param param;
    param.sched_priority = sched_get_priority_max(SCHED_FIFO) - 1;

    int ret = pthread_setschedparam(thread, SCHED_FIFO, &param);
    if (ret != 0) {
        std::cerr << "Failed to set SCHED_FIFO: " << strerror(ret) << std::endl;
        return -1;
    }

    // Pin to CPU core
    if (pin_thread_to_core(cpu_core) != 0) {
        std::cerr << "Warning: Failed to pin thread to core " << cpu_core << std::endl;
        // Don't fail entirely, just warn
    }

    return 0;
}

int pin_thread_to_core(uint32_t core_id) {
    // Get the number of CPUs
    int num_cpus = sysconf(_SC_NPROCESSORS_ONLN);
    if (core_id >= (uint32_t)num_cpus) {
        std::cerr << "Core " << core_id << " out of range (available: " << num_cpus << ")" << std::endl;
        return -1;
    }

    // Create CPU set with only the specified core
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    CPU_SET(core_id, &cpuset);

    // Apply to current thread
    pthread_t thread = pthread_self();
    int ret = pthread_setaffinity_np(thread, sizeof(cpu_set_t), &cpuset);
    if (ret != 0) {
        std::cerr << "Failed to set CPU affinity: " << strerror(ret) << std::endl;
        return -1;
    }

    return 0;
}

void sleep_precise_ns(uint64_t nanoseconds) {
    struct timespec req, rem;
    req.tv_sec = nanoseconds / 1000000000ULL;
    req.tv_nsec = nanoseconds % 1000000000ULL;

    while (nanosleep(&req, &rem) == -1 && errno == EINTR) {
        req = rem;
    }
}

}  // namespace rt_platform
