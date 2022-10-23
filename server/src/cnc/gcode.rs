pub mod display;
pub mod parser;

#[derive(Debug)]
pub struct AxisValues(Vec<(usize, f64)>); //(axis, value) pairs
#[derive(Debug)]
pub struct OffsetAxisValues(Vec<(usize, f64)>); //(axis, value) pairs
#[derive(Debug)]
pub enum Plane {
    XY,
    ZX,
    YZ,
}
#[derive(Debug)]
pub enum Unit {
    Inch,
    Millimeter,
}
#[derive(Debug)]
pub enum ProbeDirection {
    Towards,
    Away,
}
#[derive(Debug)]
pub enum ProbeRequirement {
    Optional,
    Require,
}
#[derive(Debug)]
pub enum CoordinateSystem {
    Coord0,
    Coord1,
    Coord2,
    Coord3,
    Coord4,
    Coord5,
}
#[derive(Debug)]
pub enum CoordinateMode {
    Absolute,
    Incremental,
}
#[derive(Debug)]
pub enum Orientation {
    Clockwise,
    Counterclockwise,
}
#[derive(Debug)]
pub enum SpindleMode {
    Clockwise,
    Off,
}
#[derive(Debug)]
pub enum GCodeModal {
    SetFeedrate(f64),
    SetArcPlane(Plane),
    SetUnits(Unit),
    SetCoordinateSystem(CoordinateSystem),
    SetCoordinateMode(CoordinateMode),
    SetSpindle(SpindleMode),
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
    modals: Vec<GCodeModal>,
    command: Option<GCodeCommand>,
}
impl GCodeModal {
    fn same_group(lhs: &GCodeModal, rhs: &GCodeModal) -> bool {
        matches!(
            (lhs, rhs),
            (GCodeModal::SetFeedrate(_), GCodeModal::SetFeedrate(_))
                | (GCodeModal::SetArcPlane(_), GCodeModal::SetArcPlane(_))
                | (
                    GCodeModal::SetCoordinateSystem(_),
                    GCodeModal::SetCoordinateSystem(_)
                )
                | (
                    GCodeModal::SetCoordinateMode(_),
                    GCodeModal::SetCoordinateMode(_)
                )
                | (GCodeModal::SetSpindle(_), GCodeModal::SetSpindle(_))
                | (GCodeModal::EndProgram, GCodeModal::EndProgram)
        )
    }
}
impl GCodeLine {
    fn is_valid(&self) -> bool {
        for i in 0..self.modals.len() {
            for j in 0..i {
                if GCodeModal::same_group(&self.modals[i], &self.modals[j]) {
                    return false;
                }
            }
        }
        true
    }
}
