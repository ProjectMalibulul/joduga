/// macOS real-time thread management.
/// Uses mach thread policies for time-constraint scheduling and
/// thread affinity tags for soft core pinning.

#include "platform/rt_platform.h"

#ifdef __APPLE__
#include <mach/mach.h>
#include <mach/thread_policy.h>
#include <mach/thread_act.h>
#include <pthread.h>
#include <unistd.h>
#include <iostream>
#include <ctime>
#include <cerrno>

namespace rt_platform
{

    int set_thread_rt_priority(uint32_t cpu_core)
    {
        // macOS uses Mach time-constraint scheduling rather than POSIX
        // SCHED_FIFO. Set a time-constraint policy for the audio thread.
        mach_port_t thread = mach_thread_self();

        // Time-constraint policy: period, computation, constraint, preemptible
        // These values are in Mach absolute time units.
        // For 256 samples at 48 kHz, the period is ~5.3 ms.
        mach_timebase_info_data_t tb;
        mach_timebase_info(&tb);

        // Convert nanoseconds to Mach absolute time
        uint64_t period_ns = 5333333ULL;      // ~5.3 ms
        uint64_t computation_ns = 2000000ULL; // 2 ms
        uint64_t constraint_ns = 4000000ULL;  // 4 ms

        uint64_t period = period_ns * tb.denom / tb.numer;
        uint64_t computation = computation_ns * tb.denom / tb.numer;
        uint64_t constraint = constraint_ns * tb.denom / tb.numer;

        thread_time_constraint_policy_data_t policy;
        policy.period = static_cast<uint32_t>(period);
        policy.computation = static_cast<uint32_t>(computation);
        policy.constraint = static_cast<uint32_t>(constraint);
        policy.preemptible = 0;

        kern_return_t kr = thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            reinterpret_cast<thread_policy_t>(&policy),
            THREAD_TIME_CONSTRAINT_POLICY_COUNT);

        if (kr != KERN_SUCCESS)
        {
            std::cerr << "[joduga] Failed to set time-constraint policy: " << kr << std::endl;
            // Fall back to high pthread priority
            struct sched_param param;
            param.sched_priority = sched_get_priority_max(SCHED_FIFO);
            pthread_setschedparam(pthread_self(), SCHED_FIFO, &param);
        }

        // macOS does not have a public API for hard CPU affinity.
        // Use affinity tag as a hint (same tag = same core group).
        if (pin_thread_to_core(cpu_core) != 0)
        {
            std::cerr << "[joduga] Warning: thread affinity tag not set" << std::endl;
        }

        mach_port_deallocate(mach_task_self(), thread);
        return 0;
    }

    int pin_thread_to_core(uint32_t core_id)
    {
        // macOS uses affinity tags as soft hints, not hard pinning.
        thread_affinity_policy_data_t policy;
        policy.affinity_tag = static_cast<integer_t>(core_id + 1); // 0 = no affinity

        kern_return_t kr = thread_policy_set(
            mach_thread_self(),
            THREAD_AFFINITY_POLICY,
            reinterpret_cast<thread_policy_t>(&policy),
            THREAD_AFFINITY_POLICY_COUNT);

        return (kr == KERN_SUCCESS) ? 0 : -1;
    }

    void sleep_precise_ns(uint64_t nanoseconds)
    {
        struct timespec req, rem;
        req.tv_sec = static_cast<time_t>(nanoseconds / 1000000000ULL);
        req.tv_nsec = static_cast<long>(nanoseconds % 1000000000ULL);

        while (nanosleep(&req, &rem) == -1 && errno == EINTR)
        {
            req = rem;
        }
    }

} // namespace rt_platform

#else

// Stub for non-Apple Unix that isn't Linux
namespace rt_platform
{

    int set_thread_rt_priority(uint32_t) { return -1; }
    int pin_thread_to_core(uint32_t) { return -1; }
    void sleep_precise_ns(uint64_t) {}

}

#endif
