/*++

Copyright (c) Roujan Digital Agency. Part of Audiora Virtual Audio (derived from Microsoft's
SimpleAudioSample, MS-PL).

Module Name:

    cable.cpp

Abstract:

    Ring buffer behind the Audiora mic cable (see cable.h). Writer and reader run in timer DPCs
    or position queries of two different streams, so every access takes one spin lock; the work
    under it is a memcpy of at most a few milliseconds of audio.
--*/

#include "definitions.h"
#include "cable.h"

#pragma code_seg()

// Global data of a driver image is non-paged, so the ring may be touched at DISPATCH_LEVEL.
static BYTE         g_CableBuffer[CABLE_CAPACITY];
static KSPIN_LOCK   g_CableLock;
static ULONGLONG    g_CableWritten;     // monotonic byte counters; the ring index is counter % capacity
static ULONGLONG    g_CableRead;
static BOOLEAN      g_CablePrimed;

C_ASSERT(CABLE_CAPACITY % CABLE_BYTES_PER_FRAME == 0);
C_ASSERT(CABLE_MAX_LAG_BYTES < CABLE_CAPACITY);

VOID CableInitialize()
{
    KeInitializeSpinLock(&g_CableLock);
    g_CableWritten = 0;
    g_CableRead = 0;
    g_CablePrimed = FALSE;
}

static VOID CopyIn(_In_reads_bytes_(Length) const BYTE *Source, _In_ ULONG Length)
{
    while (Length > 0)
    {
        ULONG offset = (ULONG)(g_CableWritten % CABLE_CAPACITY);
        ULONG run = MIN(Length, CABLE_CAPACITY - offset);
        RtlCopyMemory(g_CableBuffer + offset, Source, run);
        g_CableWritten += run;
        Source += run;
        Length -= run;
    }
}

static VOID CopyOut(_Out_writes_bytes_(Length) BYTE *Destination, _In_ ULONG Length)
{
    while (Length > 0)
    {
        ULONG offset = (ULONG)(g_CableRead % CABLE_CAPACITY);
        ULONG run = MIN(Length, CABLE_CAPACITY - offset);
        RtlCopyMemory(Destination, g_CableBuffer + offset, run);
        g_CableRead += run;
        Destination += run;
        Length -= run;
    }
}

VOID CableWrite(_In_reads_bytes_(Length) const BYTE *Source, _In_ ULONG Length)
{
    KIRQL irql;
    KeAcquireSpinLock(&g_CableLock, &irql);

    // A block larger than the ring only keeps its newest second.
    if (Length > CABLE_CAPACITY)
    {
        Source += Length - CABLE_CAPACITY;
        Length = CABLE_CAPACITY;
    }
    CopyIn(Source, Length);

    // Nobody listening (or listening too slowly): drop the oldest audio, keep the newest.
    if (g_CableWritten - g_CableRead > CABLE_CAPACITY)
    {
        g_CableRead = g_CableWritten - CABLE_CAPACITY;
    }

    KeReleaseSpinLock(&g_CableLock, irql);
}

VOID CableRead(_Out_writes_bytes_(Length) BYTE *Destination, _In_ ULONG Length)
{
    KIRQL irql;
    KeAcquireSpinLock(&g_CableLock, &irql);

    ULONGLONG queued = g_CableWritten - g_CableRead;

    if (!g_CablePrimed)
    {
        if (queued >= CABLE_PREFILL_BYTES + Length)
        {
            g_CablePrimed = TRUE;
        }
    }

    if (g_CablePrimed && queued > CABLE_MAX_LAG_BYTES + Length)
    {
        // keep the voice in sync: skip ahead to the prefill, on a frame boundary
        ULONGLONG keep = CABLE_PREFILL_BYTES + Length;
        g_CableRead = g_CableWritten - keep;
        g_CableRead -= g_CableRead % CABLE_BYTES_PER_FRAME;
        queued = g_CableWritten - g_CableRead;
    }

    ULONG copied = 0;
    if (g_CablePrimed)
    {
        copied = (ULONG)MIN((ULONGLONG)Length, queued);
        copied -= copied % CABLE_BYTES_PER_FRAME;
        CopyOut(Destination, copied);
        if (copied < Length)
        {
            // ran dry (the engine stopped or stalled): silence until the prefill is back
            g_CablePrimed = FALSE;
        }
    }

    KeReleaseSpinLock(&g_CableLock, irql);

    if (copied < Length)
    {
        RtlZeroMemory(Destination + copied, Length - copied);
    }
}

VOID CableResetReader()
{
    KIRQL irql;
    KeAcquireSpinLock(&g_CableLock, &irql);
    g_CablePrimed = FALSE;
    KeReleaseSpinLock(&g_CableLock, irql);
}
