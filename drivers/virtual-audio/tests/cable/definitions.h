// User-mode stand-in for the driver's definitions.h, so Source/Main/cable.cpp compiles unchanged
// into a normal test program. Only what cable.cpp uses is provided.
#pragma once
#include <windows.h>

typedef UCHAR KIRQL;
// winnt.h already has a KSPIN_LOCK typedef; the cable's lock becomes an SRW lock here
#define KSPIN_LOCK SRWLOCK

inline void KeInitializeSpinLock(KSPIN_LOCK *lock) { InitializeSRWLock(lock); }
inline void KeAcquireSpinLock(KSPIN_LOCK *lock, KIRQL *irql) { AcquireSRWLockExclusive(lock); *irql = 0; }
inline void KeReleaseSpinLock(KSPIN_LOCK *lock, KIRQL) { ReleaseSRWLockExclusive(lock); }

#ifndef MIN
#define MIN(x, y) ((x) < (y) ? (x) : (y))
#endif
