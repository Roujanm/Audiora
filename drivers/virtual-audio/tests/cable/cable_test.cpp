// Tests the Audiora mic cable (Source/Main/cable.cpp) in user mode: the exact driver source,
// compiled against tests/cable/definitions.h. Every frame written carries its index in both
// channels, so the reader can check that audio arrives complete, in order and frame-aligned.
//
// Build and run: scripts\driver-test.ps1

#include "definitions.h"
#include "cable.h"

#include <atomic>
#include <cstdint>
#include <cstdio>
#include <random>
#include <thread>
#include <vector>

static int g_failures = 0;
#define CHECK(cond, ...)                                                                        \
    do {                                                                                        \
        if (!(cond)) {                                                                          \
            ++g_failures;                                                                       \
            std::printf("  FAIL %s:%d: %s  ", __FILE__, __LINE__, #cond);                      \
            std::printf(__VA_ARGS__);                                                           \
            std::printf("\n");                                                                  \
        }                                                                                       \
    } while (0)

static const ULONG kFrame = CABLE_BYTES_PER_FRAME;
static const ULONG kMs = CABLE_SAMPLE_RATE / 1000;   // frames per millisecond

struct Writer {
    int32_t next = 1;   // 0 means silence on the reading side
    void write_frames(ULONG frames) {
        std::vector<int32_t> buf(frames * 2);
        for (ULONG i = 0; i < frames; ++i) {
            buf[i * 2] = buf[i * 2 + 1] = next++;
        }
        CableWrite(reinterpret_cast<const BYTE *>(buf.data()), frames * kFrame);
    }
};

static std::vector<int32_t> read_frames(ULONG frames) {
    std::vector<int32_t> buf(frames * 2, -1);
    CableRead(reinterpret_cast<BYTE *>(buf.data()), frames * kFrame);
    return buf;
}

// Both channels equal, and each frame either silence or exactly one more than the last audio.
struct Continuity {
    int32_t last = 0;
    size_t gaps = 0, jumps = 0, torn = 0, audio = 0, silence = 0;
    void feed(const std::vector<int32_t> &b) {
        for (size_t i = 0; i < b.size(); i += 2) {
            if (b[i] != b[i + 1]) { ++torn; continue; }
            if (b[i] == 0) { ++silence; if (last) ++gaps; continue; }
            if (last && b[i] != last + 1) ++jumps;
            last = b[i];
            ++audio;
        }
    }
};

static void reset_all() {
    CableInitialize();
}

static void test_prefill_then_exact_audio() {
    std::puts("prefill, then the exact audio");
    reset_all();
    Writer w;
    w.write_frames(10 * kMs);
    auto b = read_frames(10 * kMs);
    bool all_zero = true;
    for (auto v : b) all_zero &= v == 0;
    CHECK(all_zero, "reader must stay silent until 20 ms + one block are queued");
    w.write_frames(30 * kMs);
    b = read_frames(10 * kMs);
    CHECK(b[0] == 1 && b[1] == 1, "first frame %d", b[0]);
    CHECK(b[(10 * kMs - 1) * 2] == int32_t(10 * kMs), "last frame %d", b[(10 * kMs - 1) * 2]);
}

static void test_steady_stream_has_no_gaps() {
    std::puts("10 s steady stream, 10 ms blocks");
    reset_all();
    Writer w;
    Continuity c;
    w.write_frames(30 * kMs);
    for (int i = 0; i < 1000; ++i) {
        w.write_frames(10 * kMs);
        c.feed(read_frames(10 * kMs));
    }
    CHECK(c.gaps == 0 && c.jumps == 0 && c.torn == 0, "gaps %zu jumps %zu torn %zu", c.gaps, c.jumps, c.torn);
    CHECK(c.audio == 1000 * 10 * kMs, "audio frames %zu", c.audio);
}

static void test_timer_jitter_is_absorbed() {
    std::puts("writer jitter of up to 15 ms is absorbed by the 20 ms prefill");
    reset_all();
    Writer w;
    Continuity c;
    std::mt19937 rng(7);
    // the writer delivers the same average rate, but in late and early clumps
    w.write_frames(10 * kMs);
    int owed_ms = 0;
    for (int i = 0; i < 3000; ++i) {
        owed_ms += 1;
        if (owed_ms >= 15 || rng() % 4 == 0) {
            w.write_frames(owed_ms * kMs);
            owed_ms = 0;
        }
        c.feed(read_frames(kMs));
    }
    // after priming, nothing may be lost or repeated
    CHECK(c.jumps == 0 && c.torn == 0, "jumps %zu torn %zu", c.jumps, c.torn);
    CHECK(c.gaps == 0, "gaps %zu", c.gaps);
}

static void test_late_reader_is_trimmed_to_low_latency() {
    std::puts("a reader that starts late skips ahead instead of lagging");
    reset_all();
    Writer w;
    w.write_frames(500 * kMs);   // half a second queued before anyone listens
    auto b = read_frames(10 * kMs);
    int32_t newest = w.next - 1;
    int32_t first = b[0];
    // what is left queued after this read must be about the prefill, never the 500 ms backlog
    int32_t lag_frames = newest - b[(10 * kMs - 1) * 2];
    CHECK(first > 0, "audio expected");
    CHECK(lag_frames <= int32_t(CABLE_PREFILL_BYTES / kFrame) + 1, "lag %d frames", lag_frames);
    Continuity c;
    c.feed(b);
    CHECK(c.jumps == 0 && c.torn == 0, "skip must land on a frame boundary");
}

static void test_overflow_keeps_newest_second() {
    std::puts("nobody reading for 3 s: only the newest second is kept");
    reset_all();
    Writer w;
    for (int i = 0; i < 300; ++i) w.write_frames(10 * kMs);
    auto b = read_frames(10 * kMs);
    int32_t newest = w.next - 1;
    CHECK(b[0] > newest - int32_t(CABLE_SAMPLE_RATE), "old audio %d survived (newest %d)", b[0], newest);
    // one block larger than the whole ring
    w.write_frames(3 * CABLE_SAMPLE_RATE);
    b = read_frames(kMs);
    CHECK(b[0] > 0 && b[0] == b[1], "audio expected after a huge write");
}

static void test_underrun_goes_silent_then_recovers() {
    std::puts("engine stops: silence, then clean audio again when it returns");
    reset_all();
    Writer w;
    w.write_frames(40 * kMs);
    read_frames(10 * kMs);
    auto b = read_frames(100 * kMs);   // more than is queued
    CHECK(b[b.size() - 1] == 0, "tail must be silence");
    Continuity c;
    c.feed(b);
    CHECK(c.jumps == 0 && c.torn == 0, "no garbage while running dry");
    b = read_frames(10 * kMs);
    bool silent = true;
    for (auto v : b) silent &= v == 0;
    CHECK(silent, "still silent until the prefill is back");
    w.write_frames(40 * kMs);
    b = read_frames(10 * kMs);
    CHECK(b[0] != 0, "audio again after re-priming");
}

static void test_reset_reader() {
    std::puts("microphone restart waits for the prefill again");
    reset_all();
    Writer w;
    w.write_frames(40 * kMs);
    read_frames(10 * kMs);
    CableResetReader();
    auto b = read_frames(10 * kMs);   // 30 ms queued >= 20 ms + 10 ms: primes again at once
    CHECK(b[0] != 0, "enough queued, so audio continues");
    CableResetReader();
    b = read_frames(10 * kMs);        // 20 ms queued < 30 ms: silent
    CHECK(b[0] == 0, "not enough queued, so silence");
}

static void test_two_threads() {
    std::puts("writer and reader on two threads for 2 s, random block sizes");
    reset_all();
    std::atomic<bool> stop{false};
    std::thread writer([&] {
        Writer w;
        std::mt19937 rng(1);
        while (!stop) w.write_frames(1 + rng() % (20 * kMs));
    });
    Continuity c;
    std::mt19937 rng(2);
    auto until = GetTickCount64() + 2000;
    size_t reads = 0;
    // the reader may skip ahead (lag trim) or go silent here, but must never tear frames or
    // deliver audio out of order
    int32_t prev = 0;
    size_t backwards = 0;
    while (GetTickCount64() < until) {
        auto b = read_frames(1 + rng() % (20 * kMs));
        for (size_t i = 0; i < b.size(); i += 2) {
            if (b[i] != b[i + 1]) ++c.torn;
            if (b[i] && b[i] <= prev) ++backwards;
            if (b[i]) prev = b[i];
        }
        ++reads;
    }
    stop = true;
    writer.join();
    CHECK(c.torn == 0 && backwards == 0, "torn %zu backwards %zu over %zu reads", c.torn, backwards, reads);
}

int main() {
    test_prefill_then_exact_audio();
    test_steady_stream_has_no_gaps();
    test_timer_jitter_is_absorbed();
    test_late_reader_is_trimmed_to_low_latency();
    test_overflow_keeps_newest_second();
    test_underrun_goes_silent_then_recovers();
    test_reset_reader();
    test_two_threads();
    if (g_failures) {
        std::printf("%d check(s) failed\n", g_failures);
        return 1;
    }
    std::puts("all cable tests passed");
    return 0;
}
