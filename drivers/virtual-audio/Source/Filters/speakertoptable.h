/*++

Copyright (c) Microsoft Corporation All Rights Reserved

Module Name:

    speakertoptable.h

Abstract:

    Declaration of topology tables.
--*/

#ifndef _SIMPLEAUDIOSAMPLE_SPEAKERTOPTABLE_H_
#define _SIMPLEAUDIOSAMPLE_SPEAKERTOPTABLE_H_

//=============================================================================
static
KSDATARANGE SpeakerTopoPinDataRangesBridge[] =
{
 {
   sizeof(KSDATARANGE),
   0,
   0,
   0,
   STATICGUIDOF(KSDATAFORMAT_TYPE_AUDIO),
   STATICGUIDOF(KSDATAFORMAT_SUBTYPE_ANALOG),
   STATICGUIDOF(KSDATAFORMAT_SPECIFIER_NONE)
 }
};

//=============================================================================
static
PKSDATARANGE SpeakerTopoPinDataRangePointersBridge[] =
{
  &SpeakerTopoPinDataRangesBridge[0]
};

//=============================================================================
static
KSJACK_DESCRIPTION SpeakerJackDescBridge =
{
    KSAUDIO_SPEAKER_STEREO,
    JACKDESC_RGB(0xB3,0xC9,0x8C),              // Color spec for green
    eConnTypeUnknown,
    eGeoLocFront,
    eGenLocPrimaryBox,
    ePortConnIntegratedDevice,
    TRUE
};

// Only return a KSJACK_DESCRIPTION for the physical bridge pin.
static 
PKSJACK_DESCRIPTION SpeakerJackDescriptions[] =
{
    NULL,
    &SpeakerJackDescBridge
};

//=============================================================================
static
PCPROPERTY_ITEM SpeakerPropertiesVolume[] =
{
    {
    &KSPROPSETID_Audio,
    KSPROPERTY_AUDIO_VOLUMELEVEL,
    KSPROPERTY_TYPE_GET | KSPROPERTY_TYPE_SET | KSPROPERTY_TYPE_BASICSUPPORT,
    PropertyHandler_SpeakerTopology
    }
};

DEFINE_PCAUTOMATION_TABLE_PROP(AutomationSpeakerVolume, SpeakerPropertiesVolume);

//=============================================================================
static
PCPROPERTY_ITEM SpeakerPropertiesMute[] =
{
  {
    &KSPROPSETID_Audio,
    KSPROPERTY_AUDIO_MUTE,
    KSPROPERTY_TYPE_GET | KSPROPERTY_TYPE_SET | KSPROPERTY_TYPE_BASICSUPPORT,
    PropertyHandler_SpeakerTopology
  }
};

DEFINE_PCAUTOMATION_TABLE_PROP(AutomationSpeakerMute, SpeakerPropertiesMute);

//=============================================================================
static
PCNODE_DESCRIPTOR SpeakerTopologyNodes[] =
{
    // KSNODE_TOPO_VOLUME
    {
      0,                              // Flags
      &AutomationSpeakerVolume,     // AutomationTable
      &KSNODETYPE_VOLUME,             // Type
      &KSAUDFNAME_MASTER_VOLUME         // Name
    },
    // KSNODE_TOPO_MUTE
    {
      0,                              // Flags
      &AutomationSpeakerMute,       // AutomationTable
      &KSNODETYPE_MUTE,               // Type
      &KSAUDFNAME_MASTER_MUTE            // Name
    }
};

C_ASSERT(KSNODE_TOPO_VOLUME == 0);
C_ASSERT(KSNODE_TOPO_MUTE == 1);

static
PCCONNECTION_DESCRIPTOR SpeakerTopoMiniportConnections[] =
{
    //  FromNode,                 FromPin,                    ToNode,                 ToPin
    {   PCFILTER_NODE,            KSPIN_TOPO_WAVEOUT_SOURCE,    KSNODE_TOPO_VOLUME,     1 },
    {   KSNODE_TOPO_VOLUME,       0,                          KSNODE_TOPO_MUTE,       1 },
    {   KSNODE_TOPO_MUTE,         0,                          PCFILTER_NODE,          KSPIN_TOPO_LINEOUT_DEST }
};

//=============================================================================
static
PCPROPERTY_ITEM PropertiesSpeakerTopoFilter[] =
{
    {
        &KSPROPSETID_Jack,
        KSPROPERTY_JACK_DESCRIPTION,
        KSPROPERTY_TYPE_GET |
        KSPROPERTY_TYPE_BASICSUPPORT,
        PropertyHandler_SpeakerTopoFilter
    },
    {
        &KSPROPSETID_Jack,
        KSPROPERTY_JACK_DESCRIPTION2,
        KSPROPERTY_TYPE_GET |
        KSPROPERTY_TYPE_BASICSUPPORT,
        PropertyHandler_SpeakerTopoFilter
    }
};

DEFINE_PCAUTOMATION_TABLE_PROP(AutomationSpeakerTopoFilter, PropertiesSpeakerTopoFilter);

//=============================================================================
//
// Audiora render endpoints. They share everything except the name Windows shows for them: the
// bridge pin carries a name GUID that the INF registers under MediaCategories ("Audiora Game",
// ...). The GUIDs are internal linkage on purpose: every translation unit that includes this
// header gets its own copy and nothing has to be instantiated elsewhere.
//
// The bridge pin is a line connector, not a speaker: Windows ignores the pin name of any endpoint
// whose category is KSNODETYPE_SPEAKER and calls it "Speakers", so the outputs would be five
// identical "Speakers". As line connectors they show up as "Audiora Game", "Audiora Chat", ...
//
static const GUID AUDIORA_NAME_GAME     = { 0xec7886ef, 0x060a, 0x488f, { 0x92, 0x7d, 0x2b, 0x52, 0x3f, 0x5f, 0x75, 0xd2 } };
static const GUID AUDIORA_NAME_CHAT     = { 0x82c2fa21, 0x67ed, 0x4aeb, { 0xac, 0x99, 0x48, 0xd3, 0x01, 0x9b, 0x2f, 0xd7 } };
static const GUID AUDIORA_NAME_MEDIA    = { 0x49bc6d4d, 0x4f67, 0x44b4, { 0xa4, 0xf1, 0xd3, 0x35, 0xac, 0x33, 0xe3, 0x06 } };
static const GUID AUDIORA_NAME_AUX      = { 0x97806044, 0xd36e, 0x4633, { 0x90, 0x65, 0x12, 0x99, 0x3b, 0xe9, 0x01, 0x27 } };
static const GUID AUDIORA_NAME_MICFEED  = { 0x71e5f8b0, 0x439e, 0x4008, { 0x9e, 0x62, 0x57, 0xfe, 0x0d, 0x18, 0x10, 0x13 } };

#define AUDIORA_RENDER_TOPOLOGY(Prefix, NameGuid)                                               \
static                                                                                          \
PCPIN_DESCRIPTOR Prefix##TopoMiniportPins[] =                                                   \
{                                                                                               \
  /* KSPIN_TOPO_WAVEOUT_SOURCE */                                                               \
  {                                                                                             \
    0, 0, 0, NULL,                                                                              \
    {                                                                                           \
      0, NULL, 0, NULL,                                                                         \
      SIZEOF_ARRAY(SpeakerTopoPinDataRangePointersBridge),                                      \
      SpeakerTopoPinDataRangePointersBridge,                                                    \
      KSPIN_DATAFLOW_IN,                                                                        \
      KSPIN_COMMUNICATION_NONE,                                                                 \
      &KSCATEGORY_AUDIO,                                                                        \
      NULL,                                                                                     \
      0                                                                                         \
    }                                                                                           \
  },                                                                                            \
  /* KSPIN_TOPO_LINEOUT_DEST: the endpoint's name comes from this pin */                       \
  {                                                                                             \
    0, 0, 0, NULL,                                                                              \
    {                                                                                           \
      0, NULL, 0, NULL,                                                                         \
      SIZEOF_ARRAY(SpeakerTopoPinDataRangePointersBridge),                                      \
      SpeakerTopoPinDataRangePointersBridge,                                                    \
      KSPIN_DATAFLOW_OUT,                                                                       \
      KSPIN_COMMUNICATION_NONE,                                                                 \
      &KSNODETYPE_LINE_CONNECTOR,                                                               \
      &NameGuid,                                                                                \
      0                                                                                         \
    }                                                                                           \
  }                                                                                             \
};                                                                                              \
static                                                                                          \
PCFILTER_DESCRIPTOR Prefix##TopoMiniportFilterDescriptor =                                      \
{                                                                                               \
  0,                                                                                            \
  &AutomationSpeakerTopoFilter,                                                                 \
  sizeof(PCPIN_DESCRIPTOR),                                                                     \
  SIZEOF_ARRAY(Prefix##TopoMiniportPins),                                                       \
  Prefix##TopoMiniportPins,                                                                     \
  sizeof(PCNODE_DESCRIPTOR),                                                                    \
  SIZEOF_ARRAY(SpeakerTopologyNodes),                                                           \
  SpeakerTopologyNodes,                                                                         \
  SIZEOF_ARRAY(SpeakerTopoMiniportConnections),                                                 \
  SpeakerTopoMiniportConnections,                                                               \
  0,                                                                                            \
  NULL                                                                                          \
};

AUDIORA_RENDER_TOPOLOGY(Game,    AUDIORA_NAME_GAME)
AUDIORA_RENDER_TOPOLOGY(Chat,    AUDIORA_NAME_CHAT)
AUDIORA_RENDER_TOPOLOGY(Media,   AUDIORA_NAME_MEDIA)
AUDIORA_RENDER_TOPOLOGY(Aux,     AUDIORA_NAME_AUX)
AUDIORA_RENDER_TOPOLOGY(MicFeed, AUDIORA_NAME_MICFEED)

#endif // _SIMPLEAUDIOSAMPLE_SPEAKERTOPTABLE_H_
