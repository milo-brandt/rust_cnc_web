use ndarray::Array1;
pub use common::grbl::GrblState;
use common::grbl::GrblFullInfo;

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
#[derive(Debug, Clone, PartialEq)]
pub struct GrblStateInfo {
    pub state: GrblState,
    pub machine_position: Array1<f64>,
    pub work_coordinate_offset: Array1<f64>,
}
impl GrblStateInfo {
    pub fn to_full_info(self) -> GrblFullInfo {
        GrblFullInfo {
            state: self.state,
            machine_position: self.machine_position.into_raw_vec(),
            work_coordinate_offset: self.work_coordinate_offset.into_raw_vec(),
        }
    }
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
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeEvent {
    pub success: bool,
    pub position: Array1<f64>,
}
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum GrblMessage {
    ProbeEvent(ProbeEvent),
    StatusEvent(GrblStatus),
    GrblError(u64),
    GrblAlarm(u64),
    GrblOk,
    GrblGreeting,
    Unrecognized(String),
}
