pub enum RealtimeCommand {
    Reset = 0x18,
    StatusReport = b'?',
    CycleStart = b'~',
    FeedHold = b'!',
    JogCancel = 0x85,
    FeedOverrideReset = 0x90,
    FeedOverridePlusTen = 0x91,
    FeedOverrideMinusTen = 0x92,
    FeedOverridePlusOne = 0x93,
    FeedOvrerideMinusOne = 0x94,
    RapidOverrideReset = 0x95,
    RapidOverrideHalf = 0x96,
    RapidOverrideQuarter = 0x97,
    SpindleOverrideReset = 0x99,
    SpindleOverridePlusTen = 0x9A,
    SpindleOverrideMinusTen = 0x9B,
    SpindleOverridePlusOne = 0x9C,
    SpindleOverrideMinusOne = 0x9D,
    ToggleSpindleStop = 0x9E, //Only in HOLD state
}


// See Serial.h in FluidNC for details
/*
enum class Cmd : uint8_t {
    Reset                 = 0x18,  // Ctrl-X
    StatusReport          = '?',
    CycleStart            = '~',
    FeedHold              = '!',
    SafetyDoor            = 0x84,
    JogCancel             = 0x85,
    DebugReport           = 0x86,  // Only when DEBUG_REPORT_REALTIME enabled, sends debug report in '{}' braces.
    Macro0                = 0x87,
    Macro1                = 0x88,
    Macro2                = 0x89,
    Macro3                = 0x8a,
    FeedOvrReset          = 0x90,  // Restores feed override value to 100%.
    FeedOvrCoarsePlus     = 0x91,
    FeedOvrCoarseMinus    = 0x92,
    FeedOvrFinePlus       = 0x93,
    FeedOvrFineMinus      = 0x94,
    RapidOvrReset         = 0x95,  // Restores rapid override value to 100%.
    RapidOvrMedium        = 0x96,
    RapidOvrLow           = 0x97,
    RapidOvrExtraLow      = 0x98,  // *NOT SUPPORTED*
    SpindleOvrReset       = 0x99,  // Restores spindle override value to 100%.
    SpindleOvrCoarsePlus  = 0x9A,  // 154
    SpindleOvrCoarseMinus = 0x9B,
    SpindleOvrFinePlus    = 0x9C,
    SpindleOvrFineMinus   = 0x9D,
    SpindleOvrStop        = 0x9E,
    CoolantFloodOvrToggle = 0xA0,
    CoolantMistOvrToggle  = 0xA1,
};

*/