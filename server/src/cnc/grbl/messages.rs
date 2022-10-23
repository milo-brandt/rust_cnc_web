use ndarray::Array1;

// See: the Real-time Status Reports section at:  https://github.com/gnea/grbl/blob/master/doc/markdown/interface.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrblState {
    Idle,
    Run,
    Hold(i64),
    Jog,
    Alarm,
    Door(i64),
    Check,
    Home,
    Sleep,
}
#[derive(Debug, Clone, PartialEq)]
pub enum GrblPosition {
    Machine(Array1<f64>),
    Work(Array1<f64>),
}
#[derive(Debug, Clone, PartialEq)]
pub struct GrblStatus {
    pub state: GrblState,
    pub machine_position: GrblPosition,
    pub current_feed: Option<f64>,
    pub current_spindle: Option<f64>,
    pub planner: Option<u64>,
    pub rx_bytes: Option<u64>,
    pub work_coordinate_offset: Option<Array1<f64>>,
    pub line_number: Option<u64>,
    pub pins: Option<String>,
    pub feed_override: Option<f64>,
    pub rapid_override: Option<f64>,
    pub spindle_override: Option<f64>,
    pub accessory_state: Option<String>,
    pub unknown_terms: Vec<String>,
}
impl GrblStatus {
    pub fn new(state: GrblState, machine_position: GrblPosition) -> Self {
        GrblStatus {
            state,
            machine_position,
            current_feed: None,
            current_spindle: None,
            planner: None,
            rx_bytes: None,
            work_coordinate_offset: None,
            line_number: None,
            pins: None,
            feed_override: None,
            rapid_override: None,
            spindle_override: None,
            accessory_state: None,
            unknown_terms: Vec::new(),
        }
    }
}
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum GrblMessage {
    ProbeEvent {
        success: bool,
        position: Array1<f64>,
    },
    StatusEvent(GrblStatus),
    GrblError(u64),
    GrblAlarm(u64),
    GrblOk,
    GrblGreeting,
    Unrecognized(String),
}
