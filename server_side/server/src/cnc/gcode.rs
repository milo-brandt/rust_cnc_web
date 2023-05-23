pub mod display;
pub mod parser;
pub mod geometry;

#[derive(Debug)]
pub struct AxisValues(pub Vec<(usize, f64)>); //(axis, value) pairs
#[derive(Debug)]
pub struct OffsetAxisValues(pub Vec<(usize, f64)>); //(axis, value) pairs
#[derive(Debug, Clone, Copy)]
pub enum ArcPlane {
    XY,
    ZX,
    YZ,
}
#[derive(Debug, Clone, Copy)]
pub enum Unit {
    Inch,
    Millimeter,
}
#[derive(Debug, Clone, Copy)]
pub enum ProbeDirection {
    Towards,
    Away,
}
#[derive(Debug, Clone, Copy)]
pub enum ProbeRequirement {
    Optional,
    Require,
}
#[derive(Debug, Clone, Copy)]
pub enum CoordinateSystem {
    Coord0,
    Coord1,
    Coord2,
    Coord3,
    Coord4,
    Coord5,
}
#[derive(Debug, Clone, Copy)]
pub enum CoordinateMode {
    Absolute,
    Incremental,
}
#[derive(Debug, Clone, Copy)]
pub enum Orientation {
    Clockwise,
    Counterclockwise,
}
#[derive(Debug, Clone, Copy)]
pub enum SpindleMode {
    Clockwise,
    Off,
}
#[derive(Debug)]
pub enum GCodeModal {
    SetFeedrate(f64),
    SetArcPlane(ArcPlane),
    SetUnits(Unit),
    SetCoordinateSystem(CoordinateSystem),
    SetCoordinateMode(CoordinateMode),
    SetSpindle(SpindleMode),
    SetSpindleSpeed(f64),
    EndProgram,
}
#[derive(Debug)]
pub enum MoveMode {
    Rapid,
    Controlled,
    Unspecified,
}
#[derive(Debug)]
pub enum GCodeCommand {
    // TODO - do we care to support:
    // G0 X5
    // Y5
    Move {
        // G0, G1 or unspecified
        mode: MoveMode,
        position: AxisValues,
        machine_coordinates: bool,
    },
    ArcMove {
        orientation: Orientation, // Clockwise = G2, Counterclockwise = G3
        position: AxisValues,
        offsets: OffsetAxisValues,
        revolutions: Option<u64>,
    },
    Dwell {
        // G4
        duration: f64,
    },
    SetWorkCoordinateTo(AxisValues),
    Probe {
        position: AxisValues,
        mode: ProbeDirection,
        requirement: ProbeRequirement, // true for failure
                                       // G38.2, G38.3, G38.4, G38.5 being (TOWARDS, true), (TOWARDS, false), (AWAY, true), (AWAY, false)
    },
}
#[derive(Debug)]
pub struct GCodeLine {
    pub modals: Vec<GCodeModal>,
    pub command: Option<GCodeCommand>,
}
#[derive(Debug)]
pub struct GCodeFormatSpecification {
    pub axis_letters: Vec<u8>,
    pub offset_axis_letters: Vec<u8>,
    pub float_digits: usize,
}
