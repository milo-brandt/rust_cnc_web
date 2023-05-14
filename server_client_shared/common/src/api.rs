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
    pub is_directory: bool,
}
#[derive(Serialize, Deserialize)]
pub struct ListGcodeFiles {
    pub prefix: String,
}
#[derive(Serialize, Deserialize)]
pub struct CreateGcodeDirectory {
    pub directory: String,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct GcodeFile {
    pub name: String,
    pub is_file: bool,
}
#[derive(Serialize, Deserialize)]
pub struct ExamineGcodeFile {
    pub path: String,
}

//////
// Job
//////
pub const RUN_GCODE_FILE: &str = "/job/run_file";
pub const UPLOAD_GCODE_FILE: &str = "/job/upload_file";
pub const CREATE_GCODE_DIRECTORY: &str = "/job/create_directory";
pub const DELETE_GCODE_FILE: &str = "/job/delete_file";
pub const LIST_GCODE_FILES: &str = "/job/list_files";
pub const EXAMINE_LINES_IN_GCODE_FILE: &str = "/job/examine_lines_in_file";

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
//////
// Coordinate management
//////
#[derive(Serialize, Deserialize)]
pub struct SaveCoordinateOffset {
    pub name: String,
}
#[derive(Serialize, Deserialize)]
pub struct RestoreCoordinateOffset {
    pub name: String,
}
#[derive(Serialize, Deserialize)]
pub struct DeleteCoordinateOffset {
    pub name: String,
}
pub const SAVE_COORDINATE_OFFSET: &str = "/coordinates/save";
pub const LIST_COORDINATE_OFFSETS: &str = "/coordinates/list";
pub const RESTORE_COORDINATE_OFFSET: &str = "/coordinates/restore";
pub const DELETE_COORDINATE_OFFSET: &str = "/coordinates/delete";



/*
post! {
    url: "/gcode/delete_gcode_file/...",
    body(json): DeleteGcodeFile,
    response_ok(json): (),
}

#[path("/gcode")]
pub mod gcode {
    #[action(POST, "/run")]
    pub fn run(String file) -> ();
}



trait HttpConnection {
    pub fn request(&self, ...);
}

trait FullProvider: GCodeMark + ... { }
trait GcodeProvider: DeleteGCodeMark + ... { }
trait DeleteGCodeProvider: HttpConnection + ... { }

trait DeleteGCodeAPI: DeleteGCodeProvider {
    pub fn delete_gcode_file(&self, ...) -> Future<...> {
        // ... actually forms the request
    }
}
impl<T: DeleteGCodeProvider> DeleteGCodeAPI for T { }


connection.gcode().upload(...)
connection.gcode().mkdir(...)
connection.gcode().delete(...)


trait 


 */