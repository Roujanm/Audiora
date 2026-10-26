/*++

Copyright (c) Microsoft Corporation All Rights Reserved

Module Name:

    minipairs.h

Abstract:

    Local audio endpoint filter definitions. 
--*/

#ifndef _SIMPLEAUDIOSAMPLE_MINIPAIRS_H_
#define _SIMPLEAUDIOSAMPLE_MINIPAIRS_H_

#include "speakertopo.h"
#include "speakertoptable.h"
#include "speakerwavtable.h"

#include "micarraytopo.h"
#include "micarray1toptable.h"
#include "micarraywavtable.h"


NTSTATUS
CreateMiniportWaveRTSimpleAudioSample
( 
    _Out_       PUNKNOWN *,
    _In_        REFCLSID,
    _In_opt_    PUNKNOWN,
    _In_        POOL_FLAGS,
    _In_        PUNKNOWN,
    _In_opt_    PVOID,
    _In_        PENDPOINT_MINIPAIR
);

NTSTATUS
CreateMiniportTopologySimpleAudioSample
( 
    _Out_       PUNKNOWN *,
    _In_        REFCLSID,
    _In_opt_    PUNKNOWN,
    _In_        POOL_FLAGS,
    _In_        PUNKNOWN,
    _In_opt_    PVOID,
    _In_        PENDPOINT_MINIPAIR
);

//
// Render miniports.
//

/*********************************************************************
* Topology/Wave bridge connection for speaker (internal)             *
*                                                                    *
*              +------+                +------+                      *
*              | Wave |                | Topo |                      *
*              |      |                |      |                      *
* System   --->|0    1|--------------->|0    1|---> Line Out         *
*              |      |                |      |                      *
*              +------+                +------+                      *
*********************************************************************/
static
PHYSICALCONNECTIONTABLE SpeakerTopologyPhysicalConnections[] =
{
    {
        KSPIN_TOPO_WAVEOUT_SOURCE,  // TopologyIn
        KSPIN_WAVE_RENDER3_SOURCE,   // WaveOut
        CONNECTIONTYPE_WAVE_OUTPUT
    }
};

// One pair per Audiora output. The names must match KSNAME_Topology<X> / KSNAME_Wave<X> in the
// INF's [Strings] section.
#define AUDIORA_RENDER_MINIPAIR(Prefix, Type, TopoName, WaveName)               \
static                                                                          \
ENDPOINT_MINIPAIR Prefix##Miniports =                                           \
{                                                                               \
    Type,                                                                       \
    TopoName,                                                                   \
    NULL,                                                                       \
    CreateMiniportTopologySimpleAudioSample,                                    \
    &Prefix##TopoMiniportFilterDescriptor,                                      \
    0, NULL,                                                                    \
    WaveName,                                                                   \
    NULL,                                                                       \
    CreateMiniportWaveRTSimpleAudioSample,                                      \
    &SpeakerWaveMiniportFilterDescriptor,                                       \
    0,                                                                          \
    NULL,                                                                       \
    SPEAKER_DEVICE_MAX_CHANNELS,                                                \
    SpeakerPinDeviceFormatsAndModes,                                            \
    SIZEOF_ARRAY(SpeakerPinDeviceFormatsAndModes),                              \
    SpeakerTopologyPhysicalConnections,                                         \
    SIZEOF_ARRAY(SpeakerTopologyPhysicalConnections),                           \
    ENDPOINT_NO_FLAGS,                                                          \
};

AUDIORA_RENDER_MINIPAIR(Game,    eGameDevice,    L"TopologyGame",    L"WaveGame")
AUDIORA_RENDER_MINIPAIR(Chat,    eChatDevice,    L"TopologyChat",    L"WaveChat")
AUDIORA_RENDER_MINIPAIR(Media,   eMediaDevice,   L"TopologyMedia",   L"WaveMedia")
AUDIORA_RENDER_MINIPAIR(Aux,     eAuxDevice,     L"TopologyAux",     L"WaveAux")
AUDIORA_RENDER_MINIPAIR(MicFeed, eMicFeedDevice, L"TopologyMicFeed", L"WaveMicFeed")

//
// Capture miniports.
//

/*********************************************************************
* Topology/Wave bridge connection for mic array  1 (front)           *
*                                                                    *
*              +------+    +------+                                  *
*              | Topo |    | Wave |                                  *
*              |      |    |      |                                  *
*  Mic in  --->|0    1|===>|0    1|---> Capture Host Pin             *
*              |      |    |      |                                  *
*              +------+    +------+                                  *
*********************************************************************/
static
PHYSICALCONNECTIONTABLE MicArray1TopologyPhysicalConnections[] =
{
    {
        KSPIN_TOPO_BRIDGE,          // TopologyOut
        KSPIN_WAVE_BRIDGE,          // WaveIn
        CONNECTIONTYPE_TOPOLOGY_OUTPUT
    }
};

static
ENDPOINT_MINIPAIR MicMiniports =
{
    eMicDevice,
    L"TopologyMic",                         // make sure this matches KSNAME_TopologyMic in the inf's [Strings] section
    NULL,                                   // optional template name
    CreateMicArrayMiniportTopology,
    &MicArray1TopoMiniportFilterDescriptor,
    0, NULL,                                // Interface properties
    L"WaveMic",                             // make sure this matches KSNAME_WaveMic in the inf's [Strings] section
    NULL,                                   // optional template name
    CreateMiniportWaveRTSimpleAudioSample,
    &MicArrayWaveMiniportFilterDescriptor,
    0,                                      // Interface properties
    NULL,
    MICARRAY_DEVICE_MAX_CHANNELS,
    MicArrayPinDeviceFormatsAndModes,
    SIZEOF_ARRAY(MicArrayPinDeviceFormatsAndModes),
    MicArray1TopologyPhysicalConnections,
    SIZEOF_ARRAY(MicArray1TopologyPhysicalConnections),
    ENDPOINT_NO_FLAGS,
};


//=============================================================================
//
// Render miniport pairs. NOTE: the split of render and capture is arbitrary and
// unnessary, this array could contain capture endpoints.
//
static
PENDPOINT_MINIPAIR  g_RenderEndpoints[] = 
{
    &GameMiniports,
    &ChatMiniports,
    &MediaMiniports,
    &AuxMiniports,
    &MicFeedMiniports,
};

#define g_cRenderEndpoints  (SIZEOF_ARRAY(g_RenderEndpoints))

//=============================================================================
//
// Capture miniport pairs. NOTE: the split of render and capture is arbitrary and
// unnessary, this array could contain render endpoints.
//
static
PENDPOINT_MINIPAIR  g_CaptureEndpoints[] =
{
    &MicMiniports,
};

#define g_cCaptureEndpoints (SIZEOF_ARRAY(g_CaptureEndpoints))

//=============================================================================
//
// Total miniports = # endpoints * 2 (topology + wave).
//
#define g_MaxMiniports  ((g_cRenderEndpoints + g_cCaptureEndpoints) * 2)

#endif // _SIMPLEAUDIOSAMPLE_MINIPAIRS_H_
