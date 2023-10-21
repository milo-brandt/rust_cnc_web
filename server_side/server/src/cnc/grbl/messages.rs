use std::borrow::Cow;

use ndarray::Array1;
pub use common::grbl::GrblState;
use common::grbl::GrblFullInfo;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum GrblPosition {
    Machine(Array1<f64>),
    Work(Array1<f64>),
}
#[derive(Debug, Clone, PartialEq)]
pub struct GrblResidualStatus {
    pub work_coordinate_offset: Option<Array1<f64>>,
    pub feed_override: u8,
    pub rapid_override: u8,
    pub spindle_override: u8,
}
impl GrblResidualStatus {
    pub fn new() -> Self {
        GrblResidualStatus { work_coordinate_offset: None, feed_override: 100, rapid_override: 100, spindle_override: 100 }
    }
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
    pub feed_override: Option<u8>,
    pub rapid_override: Option<u8>,
    pub spindle_override: Option<u8>,
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
    fn enhance_with_residual(&mut self, residual: &mut GrblResidualStatus) {
        fn residual_sync<T: Clone>(received: &mut Option<T>, residual: &mut T) {
            match received {
                Some(value) => *residual = value.clone(),
                None => *received = Some(residual.clone())
            }
        }
        fn residual_sync_optional<T: Clone>(received: &mut Option<T>, residual: &mut Option<T>) {
            match received {
                Some(value) => *residual = Some(value.clone()),
                None => *received = residual.clone()
            }
        }
        residual_sync(&mut self.feed_override, &mut residual.feed_override);
        residual_sync(&mut self.rapid_override, &mut residual.rapid_override);
        residual_sync(&mut self.spindle_override, &mut residual.spindle_override);
        residual_sync_optional(&mut self.work_coordinate_offset, &mut residual.work_coordinate_offset);
    }
    pub fn to_state_with_residual(mut self, residual: &mut GrblResidualStatus) -> GrblStateInfo {
        // Precondition: either this or the residual needs to have a work coordinate offset.
        self.enhance_with_residual(residual);
        GrblStateInfo {
            state: self.state,
            machine_position: match self.machine_position {
                GrblPosition::Machine(pos) => pos,
                GrblPosition::Work(pos) => pos + self.work_coordinate_offset.as_ref().unwrap(),
            },
            work_coordinate_offset: self.work_coordinate_offset.unwrap(),
            feed_override: self.feed_override.unwrap(),
            rapid_override: self.rapid_override.unwrap(),
            spindle_override: self.spindle_override.unwrap(),
            probe: self.pins.map_or(false, |inner| inner.contains('P')),
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct GrblStateInfo {
    pub state: GrblState,
    pub machine_position: Array1<f64>,
    pub work_coordinate_offset: Array1<f64>,
    pub feed_override: u8,
    pub rapid_override: u8,
    pub spindle_override: u8,
    pub probe: bool,
}
impl GrblStateInfo {
    pub fn to_full_info(self) -> GrblFullInfo {
        GrblFullInfo {
            state: self.state,
            machine_position: self.machine_position.into_raw_vec(),
            work_coordinate_offset: self.work_coordinate_offset.into_raw_vec(),
            feed_override: self.feed_override,
            rapid_override: self.rapid_override,
            spindle_override: self.spindle_override,
            probe: self.probe,
        }
    }
}

mod array_serializer {
    use ndarray::Array1;
    use serde::{Serializer, Serialize, Deserializer, Deserialize};

    pub fn serialize<S: Serializer>(array: &Array1<f64>, serializer: S) -> Result<S::Ok, S::Error> {
        array.to_vec().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Array1<f64>, D::Error> {
        let vec = Vec::<f64>::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeEvent {
    pub success: bool,
    #[serde(with="array_serializer")]
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
impl GrblMessage {
    pub fn get_alarm_text(index: u64) -> Cow<'static, str> {
        match index {
            1 => "Hard limit triggered. Machine position is likely lost due to sudden and immediate halt. Re-homing is highly recommended.".into(),
            2 => "G-code motion target exceeds machine travel. Machine position safely retained. Alarm may be unlocked.".into(),
            3 => "Reset while in motion. Grbl cannot guarantee position. Lost steps are likely. Re-homing is highly recommended.".into(),
            4 => "Probe fail. The probe is not in the expected initial state before starting probe cycle, where G38.2 and G38.3 is not triggered and G38.4 and G38.5 is triggered.".into(),
            5 => "Probe fail. Probe did not contact the workpiece within the programmed travel for G38.2 and G38.4.".into(),
            6 => "Homing fail. Reset during active homing cycle.".into(),
            7 => "Homing fail. Safety door was opened during active homing cycle.".into(),
            8 => "Homing fail. Cycle failed to clear limit switch when pulling off. Try increasing pull-off setting or check wiring.".into(),
            9 => "Homing fail. Could not find limit switch within search distance. Defined as 1.5 * max_travel on search and 5 * pulloff on locate phases.".into(),
            _ => Cow::Owned(format!("Unknown ALARM:{}", index)),
        }
    }
    pub fn get_error_text(index: u64) -> Cow<'static, str> {
        match index {
            1 => "G-code words consist of a letter and a value. Letter was not found.".into(),
            2 => "Numeric value format is not valid or missing an expected value.".into(),
            3 => "Grbl '$' system command was not recognized or supported.".into(),
            4 => "Negative value received for an expected positive value.".into(),
            5 => "Homing cycle is not enabled via settings.".into(),
            6 => "Minimum step pulse time must be greater than 3usec".into(),
            7 => "EEPROM read failed. Reset and restored to default values.".into(),
            8 => "Grbl '$' command cannot be used unless Grbl is IDLE. Ensures smooth operation during a job.".into(),
            9 => "G-code locked out during alarm or jog state".into(),
            10 => "Soft limits cannot be enabled without homing also enabled.".into(),
            11 => "Max characters per line exceeded. Line was not processed and executed.".into(),
            12 => "(Compile Option) Grbl '$' setting value exceeds the maximum step rate supported.".into(),
            13 => "Safety door detected as opened and door state initiated.".into(),
            14 => "(Grbl-Mega Only) Build info or startup line exceeded EEPROM line length limit.".into(),
            15 => "Jog target exceeds machine travel. Command ignored.".into(),
            16 => "Jog command with no '=' or contains prohibited g-code.".into(),
            17 => "Laser mode disabled. Requires PWM output.".into(),
            20 => "Unsupported or invalid g-code command found in block.".into(),
            21 => "More than one g-code command from same modal group found in block.".into(),
            22 => "Feed rate has not yet been set or is undefined.".into(),
            23 => "G-code command in block requires an integer value.".into(),
            24 => "Two G-code commands that both require the use of the XYZ axis words were detected in the block.".into(),
            25 => "A G-code word was repeated in the block.".into(),
            26 => "A G-code command implicitly or explicitly requires XYZ axis words in the block, but none were detected.".into(),
            27 => "N line number value is not within the valid range of 1 - 9,999,999.".into(),
            28 => "A G-code command was sent, but is missing some required P or L value words in the line.".into(),
            29 => "Grbl supports six work coordinate systems G54-G59. G59.1, G59.2, and G59.3 are not supported.".into(),
            30 => "The G53 G-code command requires either a G0 seek or G1 feed motion mode to be active. A different motion was active.".into(),
            31 => "There are unused axis words in the block and G80 motion mode cancel is active.".into(),
            32 => "A G2 or G3 arc was commanded but there are no XYZ axis words in the selected plane to trace the arc.".into(),
            33 => "The motion command has an invalid target. G2, G3, and G38.2 generates this error, if the arc is impossible to generate or if the probe target is the current position.".into(),
            34 => "A G2 or G3 arc, traced with the radius definition, had a mathematical error when computing the arc geometry. Try either breaking up the arc into semi-circles or quadrants, or redefine them with the arc offset definition.".into(),
            35 => "A G2 or G3 arc, traced with the offset definition, is missing the IJK offset word in the selected plane to trace the arc.".into(),
            36 => "There are unused, leftover G-code words that aren't used by any command in the block.".into(),
            37 => "The G43.1 dynamic tool length offset command cannot apply an offset to an axis other than its configured axis. The Grbl default axis is the Z-axis.".into(),
            38 => "Tool number greater than max supported value.".into(),
            _ => Cow::Owned(format!("Unknown error:{}", index))
        }
    }
}