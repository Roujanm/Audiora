/*++

Copyright (c) Roujan Digital Agency. Part of Audiora Virtual Audio (derived from Microsoft's
SimpleAudioSample, MS-PL).

Module Name:

    cable.h

Abstract:

    The Audiora mic cable. The "Audiora Mic Feed" render endpoint writes into it and the
    "Audiora Microphone" capture endpoint reads from it, so whatever the Audiora engine plays
    into the feed is what Discord, Chrome or any other program hears from the microphone.

    Both endpoints use the same device format (48 kHz, stereo, 32-bit PCM), so bytes are copied
    as they are: no conversion, no DSP in the kernel. Both sides are paced by the same system
    clock (QPC timers), so there is no drift to correct, only timer jitter to absorb.
--*/

#ifndef _AUDIORA_CABLE_H_
#define _AUDIORA_CABLE_H_

#define CABLE_SAMPLE_RATE       48000
#define CABLE_BYTES_PER_FRAME   8                                   // 2 channels x 32 bit
#define CABLE_CAPACITY          (CABLE_SAMPLE_RATE * CABLE_BYTES_PER_FRAME)   // 1 s

// Audio the reader keeps queued before it starts (and again after running dry): absorbs the
// jitter between the two endpoints' timers.
#define CABLE_PREFILL_BYTES     ((CABLE_SAMPLE_RATE / 50) * CABLE_BYTES_PER_FRAME)   // 20 ms
// More than this queued (the microphone started long after the feed) is trimmed back to the
// prefill so the voice never lags behind.
#define CABLE_MAX_LAG_BYTES     ((CABLE_SAMPLE_RATE / 10) * CABLE_BYTES_PER_FRAME)   // 100 ms

VOID CableInitialize();

// Called by the mic feed stream for every block the audio engine rendered.
VOID CableWrite(_In_reads_bytes_(Length) const BYTE *Source, _In_ ULONG Length);

// Called by the microphone stream; fills Length bytes, silence where no audio is queued.
VOID CableRead(_Out_writes_bytes_(Length) BYTE *Destination, _In_ ULONG Length);

// The microphone stream (re)starts: wait for the prefill again.
VOID CableResetReader();

#endif // _AUDIORA_CABLE_H_
