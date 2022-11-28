use serde::{Serialize, Deserialize};

pub struct OverrideControl<'a> {
    pub reset: &'a str,
    pub plus_10: &'a str,
    pub plus_1: &'a str,
    pub minus_1: &'a str,
    pub minus_10: &'a str,
}
pub struct RapidOverride<'a> {
    pub reset: &'a str,
    pub half: &'a str,
    pub quarter: &'a str,
}

#[derive(Serialize, Deserialize)]
pub struct RunGcodeFile {
    pub path: String,
}
#[derive(Serialize, Deserialize)]
pub struct DeleteGcodeFile {
    pub path: String,
}

//////
// Job
//////
pub const RUN_GCODE_FILE: &str = "/job/run_file";
pub const UPLOAD_GCODE_FILE: &str = "/job/upload_file";
pub const DELETE_GCODE_FILE: &str = "/job/delete_file";
pub const LIST_GCODE_FILES: &str = "/job/list_files";

/////
// Debug utilities
/////
pub const SEND_RAW_GCODE: &str = "/debug/send";
// Status
pub const LISTEN_TO_RAW_MACHINE: &str = "/debug/listen_raw";
pub const LISTEN_TO_JOB_STATUS: &str = "/debug/listen_status";
pub const LISTEN_TO_MACHINE_STATUS: &str = "/debug/listen_position";

//////
// Commands
//////
pub const COMMAND_PAUSE: &str = "/command/pause";
pub const COMMAND_RESUME: &str = "/command/resume";
pub const COMMAND_STOP: &str = "/command/stop";
pub const COMMAND_RESET: &str = "/command/reset";
pub const FEED_OVERRIDE: OverrideControl = OverrideControl {
    reset: "/command/override/feed/reset",
    plus_10: "/command/override/feed/plus10",
    plus_1: "/command/override/feed/plus1",
    minus_1: "/command/override/feed/minus1",
    minus_10: "/command/override/feed/minus10",
};
pub const SPINDLE_OVERRIDE: OverrideControl = OverrideControl {
    reset: "/command/override/spindle/reset",
    plus_10: "/command/override/spindle/plus10",
    plus_1: "/command/override/spindle/plus1",
    minus_1: "/command/override/spindle/minus1",
    minus_10: "/command/override/spindle/minus10",
};
pub const RAPID_OVERRIDE: RapidOverride = RapidOverride {
    reset: "/command/override/rapid/reset",
    half: "/command/override/rapid/half",
    quarter: "/command/override/rapid/quarter",
};
