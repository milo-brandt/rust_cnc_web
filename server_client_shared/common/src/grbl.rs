use serde::{Serialize, Deserialize};

// See: the Real-time Status Reports section at:  https://github.com/gnea/grbl/blob/master/doc/markdown/interface.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize, Debug)]
pub struct GrblFullInfo {
    pub state: GrblState,
    pub machine_position: Vec<f64>,
    pub work_coordinate_offset: Vec<f64>,
    pub feed_override: u8,
    pub spindle_override: u8,
    pub rapid_override: u8,
    pub probe: bool,
}
impl GrblFullInfo {
    pub fn work_position(&self) -> Vec<f64> {
        self.machine_position.iter().zip(self.work_coordinate_offset.iter()).map(|(x, y)| x - y).collect()
    }
}